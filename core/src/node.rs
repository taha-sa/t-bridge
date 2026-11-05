//! Düğüm: bağlantı yaşam döngüsü, eşleştirme, mesaj yönlendirme ve dosya aktarımı.
use std::{
    collections::{BTreeMap, HashMap},
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard, Weak,
        atomic::{AtomicU8, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};
use quinn::{Connection, Endpoint, RecvStream, SendStream};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
    sync::{Notify, mpsc, oneshot},
    time::timeout,
};

use crate::{
    discovery::{self, Advertiser, Browser},
    files,
    host::Host,
    proto::{self, Datagram, Msg, PROTO_VERSION},
    store::{Identity, OutItem, Store, load_json, save_json},
    tls,
    types::*,
};

const CHUNK: usize = 256 * 1024;
const PROGRESS_EVERY: Duration = Duration::from_millis(250);
const PAIR_CODE_TTL: Duration = Duration::from_secs(300);

const RUN: u8 = 0;
const PAUSE: u8 = 1;
const CANCEL: u8 = 2;

/// Akış sıfırlama kodları.
const RESET_PAUSE: u32 = 1;
const RESET_CANCEL: u32 = 2;

pub(crate) fn lk<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) struct Session {
    pub peer: PeerInfo,
    pub conn: Connection,
    tx: mpsc::UnboundedSender<Msg>,
}

impl Session {
    fn send(&self, m: Msg) {
        let _ = self.tx.send(m);
    }
}

/// Alıcı tarafında kalıcı küçük durum: tamamlanmış ve iptal edilmiş aktarım anahtarları.
#[derive(Default, Serialize, Deserialize)]
struct IncPersist {
    completed: Vec<(String, String)>,
    cancelled: Vec<String>,
}

struct IncOpen {
    file: std::fs::File,
    key: String,
    name: String,
    size: u64,
    offset: u64,
}

enum Accept {
    Offset(u64),
    Cancelled,
    AlreadyDone(Option<String>),
    Failed(Option<String>),
}

/// Akış karşı tarafça durduruldu/sıfırlandı: kalıcı hata değil, kuyrukta kalır.
#[derive(Debug, thiserror::Error)]
#[error("akış kesildi: {0}")]
struct Interrupted(String);

fn intr<E: std::fmt::Display>(e: E) -> anyhow::Error {
    Interrupted(e.to_string()).into()
}

type ResultTx = oneshot::Sender<(bool, Option<String>)>;

pub(crate) struct Inner {
    pub cfg: NodeConfig,
    host: Arc<dyn Host>,
    store: Store,
    ident: Identity,
    fp: String,
    rt: tokio::runtime::Handle,
    peers: Mutex<Vec<PeerInfo>>,
    session: Mutex<Option<Arc<Session>>>,
    state: Mutex<ConnState>,
    pair_code: Mutex<Option<(String, Instant)>>,
    pending_pair: Mutex<Option<(ServerInfo, String)>>,
    discovered: Mutex<Vec<ServerInfo>>,
    browser: Mutex<Option<Browser>>,
    advertiser: Mutex<Option<Advertiser>>,
    endpoint: Mutex<Option<Endpoint>>,
    nudge: Notify,
    last_clip: Mutex<Option<blake3::Hash>>,
    // Aktarımlar
    queue: Mutex<Vec<OutItem>>,
    out_notify: Notify,
    active_out: Mutex<Option<(u64, Arc<AtomicU8>)>>,
    pending_accept: Mutex<HashMap<u64, oneshot::Sender<Accept>>>,
    pending_result: Mutex<HashMap<u64, ResultTx>>,
    inc_open: Mutex<HashMap<u64, IncOpen>>,
    active_in: Mutex<HashMap<u64, Arc<AtomicU8>>>,
    inc_persist: Mutex<IncPersist>,
    transfers: Mutex<BTreeMap<(u8, u64), TransferInfo>>,
    port: Mutex<u16>,
}

fn dir_key(d: Direction) -> u8 {
    match d {
        Direction::Outgoing => 0,
        Direction::Incoming => 1,
    }
}

impl Inner {
    pub fn new(cfg: NodeConfig, host: Arc<dyn Host>, rt: tokio::runtime::Handle) -> Result<Arc<Self>> {
        let store = Store::new(&cfg.data_dir)?;
        let ident = Identity::load_or_create(store.dir())?;
        let fp = ident.fingerprint();
        let peers = store.peers();
        let queue = store.queue();
        let inc_persist: IncPersist = load_json(&store.dir().join("incoming.json"));
        let mut transfers = BTreeMap::new();
        for it in &queue {
            if it.cancelled {
                continue;
            }
            transfers.insert(
                (0, it.id),
                TransferInfo {
                    id: it.id,
                    direction: Direction::Outgoing,
                    name: it.src.name.clone(),
                    size: it.src.size,
                    done: 0,
                    state: if it.paused { TransferState::Paused } else { TransferState::Queued },
                    speed: 0,
                    detail: None,
                },
            );
        }
        Ok(Arc::new(Self {
            host,
            store,
            ident,
            fp,
            rt,
            peers: Mutex::new(peers),
            session: Mutex::new(None),
            state: Mutex::new(ConnState::Disconnected),
            pair_code: Mutex::new(None),
            pending_pair: Mutex::new(None),
            discovered: Mutex::new(Vec::new()),
            browser: Mutex::new(None),
            advertiser: Mutex::new(None),
            endpoint: Mutex::new(None),
            nudge: Notify::new(),
            last_clip: Mutex::new(None),
            queue: Mutex::new(queue),
            out_notify: Notify::new(),
            active_out: Mutex::new(None),
            pending_accept: Mutex::new(HashMap::new()),
            pending_result: Mutex::new(HashMap::new()),
            inc_open: Mutex::new(HashMap::new()),
            active_in: Mutex::new(HashMap::new()),
            inc_persist: Mutex::new(inc_persist),
            transfers: Mutex::new(transfers),
            port: Mutex::new(0),
            cfg,
        }))
    }

    pub fn device_id(&self) -> &str {
        &self.ident.id
    }

    pub fn fingerprint(&self) -> &str {
        &self.fp
    }

    pub fn port(&self) -> u16 {
        *lk(&self.port)
    }

    fn emit(&self, e: Event) {
        self.host.on_event(e);
    }

    pub fn session(&self) -> Option<Arc<Session>> {
        lk(&self.session).clone()
    }

    pub fn state(&self) -> ConnState {
        *lk(&self.state)
    }

    pub fn current_peer(&self) -> Option<PeerInfo> {
        self.session().map(|s| s.peer.clone())
    }

    fn set_state(&self, st: ConnState) {
        let peer = self.current_peer();
        {
            let mut g = lk(&self.state);
            if *g == st && st != ConnState::Connected {
                return;
            }
            *g = st;
        }
        self.emit(Event::State { state: st, peer });
    }

    pub fn peers(&self) -> Vec<PeerInfo> {
        lk(&self.peers).clone()
    }

    fn upsert_peer(&self, p: PeerInfo) {
        let list = {
            let mut g = lk(&self.peers);
            g.retain(|x| x.fingerprint != p.fingerprint && x.id != p.id);
            g.insert(0, p);
            g.clone()
        };
        if let Err(e) = self.store.save_peers(&list) {
            log::warn!("eşler kaydedilemedi: {e:#}");
        }
    }

    pub fn unpair(&self, id: &str) {
        let list = {
            let mut g = lk(&self.peers);
            g.retain(|x| x.id != id);
            g.clone()
        };
        let _ = self.store.save_peers(&list);
        if let Some(s) = self.session()
            && s.peer.id == id {
                s.conn.close(0u32.into(), b"unpaired");
            }
    }

    fn save_queue(&self) {
        let q = lk(&self.queue).clone();
        if let Err(e) = self.store.save_queue(&q) {
            log::warn!("kuyruk kaydedilemedi: {e:#}");
        }
    }

    fn save_inc(&self) {
        let g = lk(&self.inc_persist);
        let _ = save_json(&self.store.dir().join("incoming.json"), &*g);
    }

    // ---------- Aktarım kaydı (UI) ----------

    pub fn transfers(&self) -> Vec<TransferInfo> {
        lk(&self.transfers).values().cloned().collect()
    }

    fn put_transfer(&self, t: TransferInfo) {
        lk(&self.transfers).insert((dir_key(t.direction), t.id), t.clone());
        self.emit(Event::Transfer { info: t });
    }

    fn patch_transfer(&self, dir: Direction, id: u64, f: impl FnOnce(&mut TransferInfo)) {
        let t = {
            let mut g = lk(&self.transfers);
            let Some(t) = g.get_mut(&(dir_key(dir), id)) else { return };
            f(t);
            t.clone()
        };
        self.emit(Event::Transfer { info: t });
    }

    pub fn clear_finished(&self) {
        let removed: Vec<(u64, Direction)> = {
            let mut g = lk(&self.transfers);
            let ids: Vec<_> = g
                .iter()
                .filter(|(_, t)| matches!(t.state, TransferState::Done | TransferState::Failed | TransferState::Cancelled))
                .map(|(k, t)| (*k, t.direction))
                .collect();
            for (k, _) in &ids {
                g.remove(k);
            }
            ids.into_iter().map(|((_, id), d)| (id, d)).collect()
        };
        for (id, direction) in removed {
            self.emit(Event::TransferRemoved { id, direction });
        }
    }

    // ---------- Başlatma ----------

    pub fn start(self: &Arc<Self>) -> Result<()> {
        match self.cfg.role {
            Role::Server => self.start_server(),
            Role::Client => self.start_client(),
        }
    }

    fn start_server(self: &Arc<Self>) -> Result<()> {
        let sc = tls::server_config(&self.ident)?;
        let _guard = self.rt.enter();
        let ep = match Endpoint::server(sc.clone(), SocketAddr::from(([0, 0, 0, 0], self.cfg.port))) {
            Ok(ep) => ep,
            Err(e) if self.cfg.port != 0 => {
                log::warn!("port {} kullanılamadı ({e}); otomatik port", self.cfg.port);
                Endpoint::server(sc, SocketAddr::from(([0, 0, 0, 0], 0)))?
            }
            Err(e) => return Err(e.into()),
        };
        let port = ep.local_addr()?.port();
        *lk(&self.port) = port;
        *lk(&self.endpoint) = Some(ep.clone());
        match Advertiser::start(&self.ident.id, &self.cfg.device_name, &self.fp, port) {
            Ok(a) => *lk(&self.advertiser) = Some(a),
            Err(e) => log::warn!("mDNS ilanı başlatılamadı: {e:#}"),
        }
        let me = self.clone();
        self.rt.spawn(async move {
            while let Some(inc) = ep.accept().await {
                let me = me.clone();
                tokio::spawn(async move {
                    if let Err(e) = me.handle_incoming(inc).await {
                        log::info!("gelen bağlantı reddedildi/kapandı: {e:#}");
                    }
                });
            }
        });
        Ok(())
    }

    async fn handle_incoming(self: Arc<Self>, inc: quinn::Incoming) -> Result<()> {
        let conn = timeout(Duration::from_secs(10), inc).await.context("el sıkışma zaman aşımı")??;
        let fp = tls::peer_fingerprint(&conn).ok_or_else(|| anyhow!("istemci sertifikası yok"))?;
        let (mut send, mut recv) = timeout(Duration::from_secs(10), conn.accept_bi()).await.context("hello zaman aşımı")??;
        let hello = timeout(Duration::from_secs(10), proto::read_msg(&mut recv)).await??;
        let Some(Msg::Hello { version, id, name, platform, pair_code }) = hello else {
            bail!("beklenmeyen ilk mesaj");
        };
        let reject = |reason: &str| -> Result<Vec<u8>> { proto::encode(&Msg::Reject { reason: reason.into() }) };
        if version != PROTO_VERSION {
            send.write_all(&reject("sürüm uyumsuz")?).await?;
            let _ = send.finish();
            tokio::time::sleep(Duration::from_millis(200)).await;
            bail!("sürüm uyumsuz: {version}");
        }
        let known = lk(&self.peers).iter().any(|p| p.fingerprint == fp);
        let mut paired_now = false;
        if !known {
            let ok = {
                let g = lk(&self.pair_code);
                match (&*g, &pair_code) {
                    (Some((code, exp)), Some(given)) => Instant::now() < *exp && constant_eq(code, given),
                    _ => false,
                }
            };
            if !ok {
                let why = if pair_code.is_some() { "eşleştirme kodu hatalı veya süresi dolmuş" } else { "eşleşmemiş cihaz" };
                send.write_all(&reject(why)?).await?;
                let _ = send.finish();
                tokio::time::sleep(Duration::from_millis(200)).await;
                conn.close(1u32.into(), b"rejected");
                bail!("{why}");
            }
            paired_now = true;
            *lk(&self.pair_code) = None;
        }
        let peer = PeerInfo { id, name, platform, fingerprint: fp, addrs: vec![conn.remote_address().to_string()] };
        self.upsert_peer(peer.clone());
        send.write_all(&proto::encode(&Msg::Welcome {
            id: self.ident.id.clone(),
            name: self.cfg.device_name.clone(),
            platform: self.cfg.platform,
        })?)
        .await?;
        if paired_now {
            self.emit(Event::Paired { peer: peer.clone() });
        }
        self.start_session(peer, conn, send, recv);
        Ok(())
    }

    fn start_client(self: &Arc<Self>) -> Result<()> {
        let _guard = self.rt.enter();
        let ep = Endpoint::client(SocketAddr::from(([0, 0, 0, 0], 0)))?;
        *lk(&self.endpoint) = Some(ep.clone());
        let me = self.clone();
        self.rt.spawn(async move { me.client_loop(ep).await });
        Ok(())
    }

    fn set_browsing(self: &Arc<Self>, on: bool) {
        let mut g = lk(&self.browser);
        if on && g.is_none() {
            let weak: Weak<Inner> = Arc::downgrade(self);
            match Browser::start(move |list| {
                if let Some(me) = weak.upgrade() {
                    me.on_discovered(list);
                }
            }) {
                Ok(b) => *g = Some(b),
                Err(e) => log::warn!("mDNS arama başlatılamadı: {e:#}"),
            }
        } else if !on {
            *g = None;
        }
    }

    fn on_discovered(&self, mut list: Vec<ServerInfo>) {
        let peers = self.peers();
        for s in &mut list {
            s.paired = peers.iter().any(|p| p.fingerprint == s.fingerprint);
        }
        list.sort_by(|a, b| b.paired.cmp(&a.paired).then(a.name.cmp(&b.name)));
        *lk(&self.discovered) = list.clone();
        self.emit(Event::Discovered { servers: list });
        self.nudge.notify_one();
    }

    pub fn discovered(&self) -> Vec<ServerInfo> {
        lk(&self.discovered).clone()
    }

    async fn client_loop(self: Arc<Self>, ep: Endpoint) {
        let mut backoff = Duration::from_secs(1);
        loop {
            // Bekleyen eşleştirme isteği öncelikli.
            let pending = lk(&self.pending_pair).take();
            if let Some((srv, code)) = pending {
                self.set_state(ConnState::Connecting);
                match self.connect(&ep, &srv.addrs, &srv.fingerprint, Some(code)).await {
                    Ok((peer, conn, send, recv)) => {
                        self.upsert_peer(peer.clone());
                        self.emit(Event::Paired { peer: peer.clone() });
                        self.run_client_session(peer, conn, send, recv).await;
                        backoff = Duration::from_secs(1);
                        continue;
                    }
                    Err(e) => {
                        self.emit(Event::PairFailed { reason: format!("{e:#}") });
                        self.set_state(ConnState::Disconnected);
                    }
                }
            }
            let peers: Vec<PeerInfo> = self.peers().into_iter().filter(|p| p.platform == Platform::Desktop).collect();
            self.set_browsing(true);
            let mut connected = false;
            if !peers.is_empty() {
                self.set_state(ConnState::Connecting);
                let found = self.discovered();
                for p in &peers {
                    let mut addrs: Vec<String> = found
                        .iter()
                        .filter(|s| s.fingerprint == p.fingerprint)
                        .flat_map(|s| s.addrs.clone())
                        .collect();
                    for a in &p.addrs {
                        if !addrs.contains(a) {
                            addrs.push(a.clone());
                        }
                    }
                    if addrs.is_empty() {
                        continue;
                    }
                    match self.connect(&ep, &addrs, &p.fingerprint, None).await {
                        Ok((peer, conn, send, recv)) => {
                            self.upsert_peer(peer.clone());
                            self.run_client_session(peer, conn, send, recv).await;
                            connected = true;
                            backoff = Duration::from_secs(1);
                            break;
                        }
                        Err(e) => log::info!("{} bağlanamadı: {e:#}", p.name),
                    }
                }
                if !connected {
                    self.set_state(ConnState::Disconnected);
                }
            }
            if connected {
                continue;
            }
            // Bekle: geri çekilme süresi veya dürtme (ağ değişti, mDNS buldu, eşleştirme istendi).
            let _ = timeout(backoff, self.nudge.notified()).await;
            backoff = (backoff * 2).min(Duration::from_secs(30));
        }
    }

    async fn run_client_session(self: &Arc<Self>, peer: PeerInfo, conn: Connection, send: SendStream, recv: RecvStream) {
        self.set_browsing(false);
        let s = self.start_session(peer, conn, send, recv);
        s.conn.closed().await;
        // Kısa bekleme: temizlik görevinin durumu güncellemesi için.
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    async fn connect(
        &self,
        ep: &Endpoint,
        addrs: &[String],
        server_fp: &str,
        pair_code: Option<String>,
    ) -> Result<(PeerInfo, Connection, SendStream, RecvStream)> {
        let cc = tls::client_config(&self.ident, server_fp)?;
        let mut last_err = anyhow!("adres yok");
        for a in addrs {
            let Ok(addr) = a.parse::<SocketAddr>() else { continue };
            let attempt = async {
                let conn = ep.connect_with(cc.clone(), addr, "kopru")?.await?;
                let (mut send, mut recv) = conn.open_bi().await?;
                send.write_all(&proto::encode(&Msg::Hello {
                    version: PROTO_VERSION,
                    id: self.ident.id.clone(),
                    name: self.cfg.device_name.clone(),
                    platform: self.cfg.platform,
                    pair_code: pair_code.clone(),
                })?)
                .await?;
                match proto::read_msg(&mut recv).await? {
                    Some(Msg::Welcome { id, name, platform }) => {
                        let mut addrs = vec![a.clone()];
                        if let Some(old) = lk(&self.peers).iter().find(|p| p.fingerprint == server_fp) {
                            for x in &old.addrs {
                                if !addrs.contains(x) && addrs.len() < 4 {
                                    addrs.push(x.clone());
                                }
                            }
                        }
                        let peer = PeerInfo { id, name, platform, fingerprint: server_fp.to_string(), addrs };
                        Ok::<_, anyhow::Error>(Ok((peer, conn, send, recv)))
                    }
                    Some(Msg::Reject { reason }) => Ok(Err(reason)),
                    other => bail!("beklenmeyen yanıt: {other:?}"),
                }
            };
            match timeout(Duration::from_secs(5), attempt).await {
                Ok(Ok(Ok(v))) => return Ok(v),
                Ok(Ok(Err(reason))) => bail!("reddedildi: {reason}"),
                Ok(Err(e)) => last_err = e,
                Err(_) => last_err = anyhow!("{a}: zaman aşımı"),
            }
        }
        Err(last_err)
    }

    // ---------- Oturum ----------

    fn start_session(self: &Arc<Self>, peer: PeerInfo, conn: Connection, mut send: SendStream, mut recv: RecvStream) -> Arc<Session> {
        let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();
        let s = Arc::new(Session { peer, conn: conn.clone(), tx });
        if let Some(old) = lk(&self.session).replace(s.clone()) {
            old.conn.close(0u32.into(), b"replaced");
        }
        self.set_state(ConnState::Connected);
        log::info!("bağlandı: {} ({})", s.peer.name, conn.remote_address());

        // Yazıcı: kontrol akışına sıralı yazım.
        let c = conn.clone();
        self.rt.spawn(async move {
            while let Some(m) = rx.recv().await {
                let Ok(b) = proto::encode(&m) else { continue };
                if send.write_all(&b).await.is_err() {
                    c.close(2u32.into(), b"write");
                    break;
                }
            }
        });
        // Okuyucu.
        let me = self.clone();
        let s2 = s.clone();
        self.rt.spawn(async move {
            loop {
                match proto::read_msg(&mut recv).await {
                    Ok(Some(m)) => me.dispatch(&s2, m).await,
                    Ok(None) => break,
                    Err(e) => {
                        log::info!("kontrol akışı kapandı: {e:#}");
                        break;
                    }
                }
            }
            s2.conn.close(0u32.into(), b"eof");
        });
        // Datagramlar (fare).
        let me = self.clone();
        let c = conn.clone();
        self.rt.spawn(async move {
            while let Ok(b) = c.read_datagram().await {
                if let Some(Datagram::Move { dx, dy }) = proto::decode_datagram(&b) {
                    me.emit(Event::PointerMove { dx: dx as i32, dy: dy as i32 });
                }
            }
        });
        // Gelen dosya akışları.
        let me = self.clone();
        let s3 = s.clone();
        self.rt.spawn(async move {
            while let Ok(r) = s3.conn.accept_uni().await {
                let me = me.clone();
                let s4 = s3.clone();
                tokio::spawn(async move {
                    if let Err(e) = me.receive_stream(&s4, r).await {
                        log::info!("gelen akış: {e:#}");
                    }
                });
            }
        });
        // Giden kuyruk işçisi.
        let me = self.clone();
        let s5 = s.clone();
        self.rt.spawn(async move { me.out_worker(s5).await });
        // Temizlik.
        let me = self.clone();
        let s6 = s.clone();
        self.rt.spawn(async move {
            let reason = s6.conn.closed().await;
            log::info!("bağlantı kapandı: {reason}");
            me.on_session_closed(&s6);
        });
        s
    }

    fn on_session_closed(self: &Arc<Self>, s: &Arc<Session>) {
        let is_current = {
            let mut g = lk(&self.session);
            if g.as_ref().is_some_and(|c| Arc::ptr_eq(c, s)) {
                *g = None;
                true
            } else {
                false
            }
        };
        // Yerine yeni oturum geçtiyse (hızlı yeniden bağlanma) ortak tablolar yeni oturumundur: dokunma.
        if !is_current {
            return;
        }
        lk(&self.pending_accept).clear();
        lk(&self.pending_result).clear();
        lk(&self.inc_open).clear();
        // Yarım kalan aktarımlar "sırada" görünür; bağlanınca devam eder.
        let ids: Vec<(Direction, u64)> = lk(&self.transfers)
            .values()
            .filter(|t| t.state == TransferState::Active)
            .map(|t| (t.direction, t.id))
            .collect();
        for (d, id) in ids {
            self.patch_transfer(d, id, |t| {
                t.state = TransferState::Queued;
                t.speed = 0;
            });
        }
        self.set_state(ConnState::Disconnected);
        self.nudge.notify_one();
    }

    async fn dispatch(self: &Arc<Self>, s: &Arc<Session>, m: Msg) {
        match m {
            Msg::Clip { text } => {
                *lk(&self.last_clip) = Some(blake3::hash(text.as_bytes()));
                self.emit(Event::ClipboardReceived { text });
            }
            Msg::Battery { level, charging } => self.emit(Event::Battery { level, charging }),
            Msg::Notif(data) => self.emit(Event::Notification { data }),
            Msg::NotifRemoved { key } => self.emit(Event::NotificationRemoved { key }),
            Msg::AppIcon { package, png } => self.emit(Event::AppIcon { package, png }),
            Msg::NowPlaying(info) => self.emit(Event::NowPlaying { info }),
            Msg::Pc(cmd) => self.emit(Event::Pc { cmd }),
            Msg::Phone(cmd) => self.emit(Event::Phone { cmd }),
            Msg::FileOffer { id, name, size } => {
                if let Err(e) = self.on_offer(s, id, name, size).await {
                    log::warn!("teklif işlenemedi: {e:#}");
                    s.send(Msg::FileResult { id, ok: false, detail: Some(format!("{e:#}")) });
                }
            }
            Msg::FileAccept { id, offset } => {
                if let Some(tx) = lk(&self.pending_accept).remove(&id) {
                    let _ = tx.send(Accept::Offset(offset));
                }
            }
            Msg::FileResult { id, ok, detail } => {
                if let Some(tx) = lk(&self.pending_accept).remove(&id) {
                    // Teklife doğrudan sonuçla yanıt: zaten alınmış (ok) veya hedef açılamadı.
                    let _ = tx.send(if ok { Accept::AlreadyDone(detail) } else { Accept::Failed(detail) });
                } else if let Some(tx) = lk(&self.pending_result).remove(&id) {
                    let _ = tx.send((ok, detail));
                }
            }
            Msg::FilePause { id } => {
                if self.is_outgoing(id) {
                    self.pause_out(id, false);
                } else {
                    self.patch_transfer(Direction::Incoming, id, |t| {
                        t.state = TransferState::Paused;
                        t.speed = 0;
                    });
                }
            }
            Msg::FileResume { id } => {
                if self.is_outgoing(id) {
                    self.resume_out(id);
                }
            }
            Msg::FileCancel { id } => {
                if self.is_outgoing(id) {
                    self.cancel_out(id, false);
                } else {
                    self.cancel_in_local(s, id);
                }
            }
            Msg::Hello { .. } | Msg::Welcome { .. } | Msg::Reject { .. } => {}
        }
    }

    // ---------- Pano ----------

    pub fn send_clipboard(&self, text: String) -> bool {
        if text.is_empty() || text.len() > proto::MAX_CLIP {
            return false;
        }
        // Bağlı değilken kaydetme: bağlandıktan sonra aynı metin yeniden kopyalanınca gidebilsin.
        let Some(s) = self.session() else { return false };
        let h = blake3::hash(text.as_bytes());
        {
            let mut g = lk(&self.last_clip);
            if *g == Some(h) {
                return false;
            }
            *g = Some(h);
        }
        s.send(Msg::Clip { text });
        true
    }

    pub fn send_msg(&self, m: Msg) -> bool {
        match self.session() {
            Some(s) => {
                s.send(m);
                true
            }
            None => false,
        }
    }

    pub fn send_pointer(&self, dx: i32, dy: i32) {
        if let Some(s) = self.session() {
            let d = Datagram::Move { dx: dx.clamp(-32768, 32767) as i16, dy: dy.clamp(-32768, 32767) as i16 };
            let _ = s.conn.send_datagram(proto::encode_datagram(&d).into());
        }
    }

    // ---------- Eşleştirme ----------

    pub fn pairing_info(&self) -> PairingInfo {
        let code = format!("{:06}", rand::random_range(0..1_000_000u32));
        *lk(&self.pair_code) = Some((code.clone(), Instant::now() + PAIR_CODE_TTL));
        let port = self.port();
        let addrs: Vec<String> = discovery::local_ipv4s().into_iter().map(|ip| format!("{ip}:{port}")).collect();
        let uri = format!(
            "kopru://pair?i={}&n={}&f={}&c={}&a={}",
            self.ident.id,
            pct_encode(&self.cfg.device_name),
            self.fp,
            code,
            addrs.join(",")
        );
        PairingInfo { uri, code, addrs, fingerprint: self.fp.clone(), name: self.cfg.device_name.clone() }
    }

    pub fn pair_uri(&self, uri: &str) -> Result<()> {
        let q = uri.strip_prefix("kopru://pair?").ok_or_else(|| anyhow!("geçersiz QR"))?;
        let mut map = HashMap::new();
        for kv in q.split('&') {
            if let Some((k, v)) = kv.split_once('=') {
                map.insert(k, pct_decode(v));
            }
        }
        let get = |k: &str| map.get(k).cloned().ok_or_else(|| anyhow!("QR'da '{k}' yok"));
        let srv = ServerInfo {
            id: get("i")?,
            name: get("n")?,
            fingerprint: get("f")?,
            addrs: get("a")?.split(',').filter(|s| !s.is_empty()).map(String::from).collect(),
            paired: false,
        };
        let code = get("c")?;
        *lk(&self.pending_pair) = Some((srv, code));
        self.nudge.notify_one();
        Ok(())
    }

    pub fn pair_code(&self, server_id: &str, code: &str) -> Result<()> {
        let srv = self
            .discovered()
            .into_iter()
            .find(|s| s.id == server_id)
            .ok_or_else(|| anyhow!("bilgisayar bulunamadı"))?;
        *lk(&self.pending_pair) = Some((srv, code.trim().to_string()));
        self.nudge.notify_one();
        Ok(())
    }

    pub fn nudge(&self) {
        self.nudge.notify_one();
    }

    // ---------- Giden dosyalar ----------

    fn default_peer(&self) -> Option<String> {
        self.session().map(|s| s.peer.id.clone()).or_else(|| lk(&self.peers).first().map(|p| p.id.clone()))
    }

    pub fn send_files(&self, files: Vec<FileSource>) -> Result<Vec<u64>> {
        let peer = self.default_peer().ok_or_else(|| anyhow!("eşleşmiş cihaz yok"))?;
        let mut ids = Vec::new();
        for src in files {
            // JS sayıları 2^53 altında tam kesin: arayüzde id kaybı olmasın.
            let id = rand::random::<u64>() & ((1u64 << 53) - 1);
            ids.push(id);
            self.put_transfer(TransferInfo {
                id,
                direction: Direction::Outgoing,
                name: src.name.clone(),
                size: src.size,
                done: 0,
                state: TransferState::Queued,
                speed: 0,
                detail: None,
            });
            lk(&self.queue).push(OutItem { id, peer: peer.clone(), src, paused: false, cancelled: false });
        }
        self.save_queue();
        self.out_notify.notify_one();
        Ok(ids)
    }

    fn is_outgoing(&self, id: u64) -> bool {
        lk(&self.queue).iter().any(|i| i.id == id)
    }

    fn pause_out(&self, id: u64, tell_peer: bool) {
        let mut found = false;
        for it in lk(&self.queue).iter_mut().filter(|i| i.id == id) {
            it.paused = true;
            found = true;
        }
        if !found {
            return;
        }
        self.save_queue();
        if let Some((aid, ctl)) = &*lk(&self.active_out)
            && *aid == id {
                ctl.store(PAUSE, Ordering::SeqCst);
            }
        if let Some(tx) = lk(&self.pending_accept).remove(&id) {
            let _ = tx.send(Accept::Cancelled);
        }
        self.patch_transfer(Direction::Outgoing, id, |t| {
            t.state = TransferState::Paused;
            t.speed = 0;
        });
        if tell_peer {
            self.send_msg(Msg::FilePause { id });
        }
    }

    fn resume_out(&self, id: u64) {
        let mut found = false;
        for it in lk(&self.queue).iter_mut().filter(|i| i.id == id && !i.cancelled) {
            it.paused = false;
            found = true;
        }
        if found {
            self.save_queue();
            self.patch_transfer(Direction::Outgoing, id, |t| t.state = TransferState::Queued);
            self.out_notify.notify_one();
        }
    }

    fn cancel_out(&self, id: u64, tell_peer: bool) {
        let connected = self.session().is_some();
        {
            let mut q = lk(&self.queue);
            if tell_peer && !connected {
                // Çevrimdışı: karşı tarafa bağlanınca bildirilecek.
                for it in q.iter_mut().filter(|i| i.id == id) {
                    it.cancelled = true;
                }
            } else {
                q.retain(|i| i.id != id);
            }
        }
        self.save_queue();
        if let Some((aid, ctl)) = &*lk(&self.active_out)
            && *aid == id {
                ctl.store(CANCEL, Ordering::SeqCst);
            }
        if let Some(tx) = lk(&self.pending_accept).remove(&id) {
            let _ = tx.send(Accept::Cancelled);
        }
        self.patch_transfer(Direction::Outgoing, id, |t| {
            t.state = TransferState::Cancelled;
            t.speed = 0;
        });
        if tell_peer && connected {
            self.send_msg(Msg::FileCancel { id });
        }
    }

    pub fn pause_transfer(&self, id: u64, dir: Direction) -> Result<()> {
        match dir {
            Direction::Outgoing => self.pause_out(id, true),
            Direction::Incoming => {
                if !self.send_msg(Msg::FilePause { id }) {
                    bail!("bağlı değil");
                }
                if let Some(ctl) = lk(&self.active_in).get(&id) {
                    ctl.store(PAUSE, Ordering::SeqCst);
                }
                self.patch_transfer(Direction::Incoming, id, |t| {
                    t.state = TransferState::Paused;
                    t.speed = 0;
                });
            }
        }
        Ok(())
    }

    pub fn resume_transfer(&self, id: u64, dir: Direction) -> Result<()> {
        match dir {
            Direction::Outgoing => self.resume_out(id),
            Direction::Incoming => {
                if !self.send_msg(Msg::FileResume { id }) {
                    bail!("bağlı değil");
                }
                self.patch_transfer(Direction::Incoming, id, |t| t.state = TransferState::Queued);
            }
        }
        Ok(())
    }

    pub fn cancel_transfer(&self, id: u64, dir: Direction) {
        match dir {
            Direction::Outgoing => self.cancel_out(id, true),
            Direction::Incoming => {
                let peer = self.current_peer().map(|p| p.id).or_else(|| self.default_peer());
                if let Some(s) = self.session() {
                    s.send(Msg::FileCancel { id });
                    self.cancel_in_local(&s, id);
                } else if let Some(peer) = peer {
                    let key = inc_key(&peer, id);
                    lk(&self.inc_persist).cancelled.push(key.clone());
                    self.save_inc();
                    self.discard_partial(&key, "");
                    self.patch_transfer(Direction::Incoming, id, |t| t.state = TransferState::Cancelled);
                }
            }
        }
    }

    async fn out_worker(self: Arc<Self>, s: Arc<Session>) {
        // Çevrimdışıyken iptal edilenleri bildir.
        let cancelled: Vec<u64> = {
            let mut q = lk(&self.queue);
            let ids = q.iter().filter(|i| i.peer == s.peer.id && i.cancelled).map(|i| i.id).collect();
            q.retain(|i| !(i.peer == s.peer.id && i.cancelled));
            ids
        };
        if !cancelled.is_empty() {
            self.save_queue();
            for id in cancelled {
                s.send(Msg::FileCancel { id });
            }
        }
        loop {
            if s.conn.close_reason().is_some() {
                return;
            }
            let next = lk(&self.queue).iter().find(|i| i.peer == s.peer.id && !i.paused && !i.cancelled).cloned();
            let Some(item) = next else {
                tokio::select! {
                    _ = self.out_notify.notified() => continue,
                    _ = s.conn.closed() => return,
                }
            };
            let id = item.id;
            match self.send_one(&s, &item).await {
                Ok(()) => {}
                Err(e) => {
                    if s.conn.close_reason().is_some() {
                        return;
                    }
                    lk(&self.pending_result).remove(&id);
                    if e.downcast_ref::<Interrupted>().is_some() {
                        // Duraklatma/iptal veya geçici kesinti: öğe kuyrukta kalır.
                        let still = lk(&self.queue).iter().any(|i| i.id == id && !i.paused && !i.cancelled);
                        if still {
                            self.patch_transfer(Direction::Outgoing, id, |t| {
                                t.state = TransferState::Queued;
                                t.speed = 0;
                            });
                            tokio::select! {
                                _ = tokio::time::sleep(Duration::from_secs(1)) => {}
                                _ = s.conn.closed() => return,
                            }
                        }
                        continue;
                    }
                    // Kalıcı hata (dosya okunamıyor vb.): kuyruktan çıkar.
                    log::warn!("gönderim hatası ({}): {e:#}", item.src.name);
                    lk(&self.queue).retain(|i| i.id != id);
                    self.save_queue();
                    self.patch_transfer(Direction::Outgoing, id, |t| {
                        t.state = TransferState::Failed;
                        t.speed = 0;
                        t.detail = Some(format!("{e:#}"));
                    });
                }
            }
        }
    }

    async fn open_outgoing(&self, src: &FileSource) -> Result<tokio::fs::File> {
        if self.cfg.download_dir.is_some() || cfg!(not(unix)) {
            Ok(tokio::fs::File::open(&src.source).await.with_context(|| src.source.clone())?)
        } else {
            let host = self.host.clone();
            let source = src.source.clone();
            let fd = tokio::task::spawn_blocking(move || host.open_outgoing(source)).await??;
            Ok(tokio::fs::File::from_std(files::file_from_fd(fd)?))
        }
    }

    async fn send_one(&self, s: &Arc<Session>, item: &OutItem) -> Result<()> {
        let ctl = Arc::new(AtomicU8::new(RUN));
        *lk(&self.active_out) = Some((item.id, ctl.clone()));
        let r = self.send_one_inner(s, item, &ctl).await;
        // Yalnızca kendi kaydını temizle: eski oturumun işçisi yenisinin denetimini silmesin.
        let mut g = lk(&self.active_out);
        if g.as_ref().is_some_and(|(_, c)| Arc::ptr_eq(c, &ctl)) {
            *g = None;
        }
        r
    }

    async fn send_one_inner(&self, s: &Arc<Session>, item: &OutItem, ctl: &AtomicU8) -> Result<()> {
        let id = item.id;
        let size = item.src.size;
        let (atx, arx) = oneshot::channel();
        lk(&self.pending_accept).insert(id, atx);
        self.patch_transfer(Direction::Outgoing, id, |t| {
            t.state = TransferState::Active;
            t.speed = 0;
            t.detail = None;
        });
        s.send(Msg::FileOffer { id, name: item.src.name.clone(), size });
        let accept = tokio::select! {
            r = arx => r.map_err(|_| anyhow!("teklif yanıtsız kaldı"))?,
            _ = s.conn.closed() => bail!("bağlantı koptu"),
            _ = tokio::time::sleep(Duration::from_secs(60)) => {
                lk(&self.pending_accept).remove(&id);
                bail!("teklif zaman aşımı");
            }
        };
        let offset = match accept {
            Accept::Offset(o) => o.min(size),
            Accept::Cancelled => return Ok(()), // durum zaten güncellendi (pause/cancel)
            Accept::AlreadyDone(detail) => {
                self.finish_out(id, true, detail);
                return Ok(());
            }
            Accept::Failed(detail) => bail!("alıcı reddetti: {}", detail.unwrap_or_default()),
        };

        let mut file = self.open_outgoing(&item.src).await?;
        let mut hasher = blake3::Hasher::new();
        let mut buf = vec![0u8; CHUNK];
        // Devam: önceki kısmın hash'ini yerelde hesapla (ağdan geçmez).
        let mut pos = 0u64;
        while pos < offset {
            let want = ((offset - pos) as usize).min(CHUNK);
            let n = file.read(&mut buf[..want]).await?;
            if n == 0 {
                bail!("dosya kısalmış");
            }
            hasher.update(&buf[..n]);
            pos += n as u64;
        }

        let (rtx, rrx) = oneshot::channel();
        lk(&self.pending_result).insert(id, rtx);
        let mut stream = s.conn.open_uni().await.map_err(intr)?;
        stream.write_all(&id.to_le_bytes()).await.map_err(intr)?;
        let mut done = offset;
        let mut tick = Instant::now();
        let mut tick_bytes = done;
        self.patch_transfer(Direction::Outgoing, id, |t| t.done = done);
        loop {
            match ctl.load(Ordering::SeqCst) {
                RUN => {}
                PAUSE => {
                    let _ = stream.reset(RESET_PAUSE.into());
                    lk(&self.pending_result).remove(&id);
                    return Ok(());
                }
                _ => {
                    let _ = stream.reset(RESET_CANCEL.into());
                    lk(&self.pending_result).remove(&id);
                    return Ok(());
                }
            }
            let n = file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            if done + n as u64 > size {
                let _ = stream.reset(RESET_CANCEL.into());
                bail!("dosya gönderim sırasında büyüdü");
            }
            hasher.update(&buf[..n]);
            stream.write_all(&buf[..n]).await.map_err(intr)?;
            done += n as u64;
            let el = tick.elapsed();
            if el >= PROGRESS_EVERY {
                let speed = ((done - tick_bytes) as f64 / el.as_secs_f64()) as u64;
                tick = Instant::now();
                tick_bytes = done;
                self.patch_transfer(Direction::Outgoing, id, |t| {
                    t.done = done;
                    t.speed = speed;
                });
            }
        }
        if done != size {
            let _ = stream.reset(RESET_CANCEL.into());
            bail!("dosya boyutu değişti ({done} / {size})");
        }
        stream.write_all(hasher.finalize().as_bytes()).await.map_err(intr)?;
        stream.finish().map_err(intr)?;
        self.patch_transfer(Direction::Outgoing, id, |t| {
            t.done = done;
            t.speed = 0;
        });
        let (ok, detail) = tokio::select! {
            r = rrx => r.map_err(|_| anyhow!("sonuç gelmedi"))?,
            _ = s.conn.closed() => bail!("bağlantı koptu"),
        };
        self.finish_out(id, ok, detail);
        Ok(())
    }

    fn finish_out(&self, id: u64, ok: bool, detail: Option<String>) {
        lk(&self.queue).retain(|i| i.id != id);
        self.save_queue();
        self.patch_transfer(Direction::Outgoing, id, |t| {
            t.state = if ok { TransferState::Done } else { TransferState::Failed };
            if ok {
                t.done = t.size;
            }
            t.speed = 0;
            t.detail = detail;
        });
    }

    // ---------- Gelen dosyalar ----------

    fn open_target(&self, key: &str, name: &str, size: u64) -> Result<(std::fs::File, u64)> {
        match &self.cfg.download_dir {
            Some(dir) => files::open_part(&PathBuf::from(dir), key, size),
            None => {
                let o = self.host.open_incoming(key.to_string(), name.to_string(), size)?;
                let f = files::file_from_fd(o.fd)?;
                let mut existing = o.existing;
                if existing > size {
                    f.set_len(0)?;
                    existing = 0;
                }
                Ok((f, existing))
            }
        }
    }

    fn finish_target(&self, key: &str, name: &str, ok: bool) -> Result<Option<String>> {
        match &self.cfg.download_dir {
            Some(dir) => files::finish_part(&PathBuf::from(dir), key, name, ok),
            None => Ok(self.host.complete_incoming(key.to_string(), ok)),
        }
    }

    fn discard_partial(&self, key: &str, name: &str) {
        if let Err(e) = self.finish_target(key, name, false) {
            log::warn!("yarım dosya silinemedi: {e:#}");
        }
    }

    async fn on_offer(self: &Arc<Self>, s: &Arc<Session>, id: u64, name: String, size: u64) -> Result<()> {
        let key = inc_key(&s.peer.id, id);
        {
            let mut p = lk(&self.inc_persist);
            if let Some((_, path)) = p.completed.iter().find(|(k, _)| *k == key) {
                s.send(Msg::FileResult { id, ok: true, detail: Some(path.clone()) });
                return Ok(());
            }
            if let Some(i) = p.cancelled.iter().position(|k| *k == key) {
                p.cancelled.remove(i);
                drop(p);
                self.save_inc();
                s.send(Msg::FileCancel { id });
                return Ok(());
            }
        }
        let me = self.clone();
        let (k2, n2) = (key.clone(), name.clone());
        let (file, existing) = tokio::task::spawn_blocking(move || me.open_target(&k2, &n2, size)).await??;
        lk(&self.inc_open).insert(id, IncOpen { file, key, name: name.clone(), size, offset: existing });
        self.put_transfer(TransferInfo {
            id,
            direction: Direction::Incoming,
            name,
            size,
            done: existing,
            state: TransferState::Active,
            speed: 0,
            detail: None,
        });
        s.send(Msg::FileAccept { id, offset: existing });
        Ok(())
    }

    fn cancel_in_local(&self, _s: &Arc<Session>, id: u64) {
        if let Some(ctl) = lk(&self.active_in).get(&id) {
            ctl.store(CANCEL, Ordering::SeqCst);
        }
        // Akış etkin değilse yarım dosyayı hemen sil.
        let open = lk(&self.inc_open).remove(&id);
        let active = lk(&self.active_in).contains_key(&id);
        if !active {
            let (key, name) = match open {
                Some(o) => (o.key, o.name),
                None => (inc_key(&_s.peer.id, id), String::new()),
            };
            self.discard_partial(&key, &name);
        }
        self.patch_transfer(Direction::Incoming, id, |t| {
            t.state = TransferState::Cancelled;
            t.speed = 0;
        });
    }

    async fn receive_stream(self: &Arc<Self>, s: &Arc<Session>, mut recv: RecvStream) -> Result<()> {
        let mut idb = [0u8; 8];
        recv.read_exact(&mut idb).await?;
        let id = u64::from_le_bytes(idb);
        let Some(open) = lk(&self.inc_open).remove(&id) else {
            let _ = recv.stop(RESET_CANCEL.into());
            bail!("bilinmeyen aktarım {id}");
        };
        let IncOpen { file, key, name, size, offset } = open;
        let ctl = Arc::new(AtomicU8::new(RUN));
        lk(&self.active_in).insert(id, ctl.clone());
        // Dosya tanıtıcısı receive_body içinde kapanır (Windows'ta yeniden adlandırma için şart).
        let r = self.receive_body(&mut recv, file, size, offset, id, &ctl).await;
        lk(&self.active_in).remove(&id);
        match r {
            Ok(true) => {
                let me = self.clone();
                let (k, n) = (key.clone(), name.clone());
                let path = tokio::task::spawn_blocking(move || me.finish_target(&k, &n, true)).await??;
                {
                    let mut p = lk(&self.inc_persist);
                    p.completed.push((key.clone(), path.clone().unwrap_or_default()));
                    let len = p.completed.len();
                    if len > 500 {
                        p.completed.drain(..len - 500);
                    }
                }
                self.save_inc();
                s.send(Msg::FileResult { id, ok: true, detail: path.clone() });
                self.patch_transfer(Direction::Incoming, id, |t| {
                    t.state = TransferState::Done;
                    t.done = t.size;
                    t.speed = 0;
                    t.detail = path;
                });
            }
            Ok(false) => {
                // Hash uyuşmadı: sil, gönderene bildir.
                self.discard_partial(&key, &name);
                s.send(Msg::FileResult { id, ok: false, detail: Some("doğrulama hatası".into()) });
                self.patch_transfer(Direction::Incoming, id, |t| {
                    t.state = TransferState::Failed;
                    t.speed = 0;
                    t.detail = Some("doğrulama hatası".into());
                });
            }
            Err(e) => {
                if ctl.load(Ordering::SeqCst) == CANCEL {
                    self.discard_partial(&key, &name);
                    let _ = recv.stop(RESET_CANCEL.into());
                } else {
                    // Duraklatma/kopma: yarım dosya korunur, devam edilebilir.
                    self.patch_transfer(Direction::Incoming, id, |t| {
                        if t.state == TransferState::Active {
                            t.state = TransferState::Queued;
                        }
                        t.speed = 0;
                    });
                }
                return Err(e);
            }
        }
        Ok(())
    }

    /// Ok(true): tamam ve doğrulandı; Ok(false): hash uyuşmadı; Err: kesildi.
    async fn receive_body(&self, recv: &mut RecvStream, file: std::fs::File, size: u64, offset: u64, id: u64, ctl: &AtomicU8) -> Result<bool> {
        let mut file = tokio::fs::File::from_std(file);
        let mut hasher = blake3::Hasher::new();
        let mut buf = vec![0u8; CHUNK];
        file.seek(std::io::SeekFrom::Start(0)).await?;
        let mut pos = 0u64;
        while pos < offset {
            let want = ((offset - pos) as usize).min(CHUNK);
            let n = file.read(&mut buf[..want]).await?;
            if n == 0 {
                bail!("yarım dosya kısalmış");
            }
            hasher.update(&buf[..n]);
            pos += n as u64;
        }
        file.set_len(offset).await?;
        file.seek(std::io::SeekFrom::Start(offset)).await?;
        let mut done = offset;
        let mut tick = Instant::now();
        let mut tick_bytes = done;
        while done < size {
            if ctl.load(Ordering::SeqCst) != RUN {
                file.flush().await?;
                bail!("durduruldu");
            }
            let want = ((size - done) as usize).min(CHUNK);
            let n = match recv.read(&mut buf[..want]).await {
                Ok(Some(n)) => n,
                Ok(None) => {
                    file.flush().await?;
                    bail!("akış erken bitti");
                }
                Err(e) => {
                    file.flush().await?;
                    return Err(e.into());
                }
            };
            hasher.update(&buf[..n]);
            file.write_all(&buf[..n]).await?;
            done += n as u64;
            let el = tick.elapsed();
            if el >= PROGRESS_EVERY {
                let speed = ((done - tick_bytes) as f64 / el.as_secs_f64()) as u64;
                tick = Instant::now();
                tick_bytes = done;
                self.patch_transfer(Direction::Incoming, id, |t| {
                    t.done = done;
                    t.speed = speed;
                    t.state = TransferState::Active;
                });
            }
        }
        let mut trailer = [0u8; 32];
        recv.read_exact(&mut trailer).await?;
        file.flush().await?;
        file.sync_all().await?;
        drop(file);
        Ok(hasher.finalize().as_bytes() == &trailer)
    }

    pub fn shutdown(&self) {
        if let Some(s) = lk(&self.session).take() {
            s.conn.close(0u32.into(), b"shutdown");
        }
        *lk(&self.browser) = None;
        *lk(&self.advertiser) = None;
        if let Some(ep) = lk(&self.endpoint).take() {
            ep.close(0u32.into(), b"shutdown");
        }
    }
}

fn inc_key(peer: &str, id: u64) -> String {
    format!("{peer}-{id:016x}")
}

fn constant_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.trim().as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn pct_encode(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

fn pct_decode(s: &str) -> String {
    // Bayt düzeyinde çözülür: "%" ardından çok baytlı karakter gelse de dilimleme paniği olmaz.
    fn hex(c: u8) -> Option<u8> {
        (c as char).to_digit(16).map(|d| d as u8)
    }
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h << 4 | l);
                i += 3;
                continue;
            }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pct() {
        let n = "Taha'nın PC'si & ğ";
        assert_eq!(pct_decode(&pct_encode(n)), n);
        assert!(constant_eq("123456", " 123456 "));
        assert!(!constant_eq("123456", "123457"));
        // Bozuk/kötü niyetli girdi panik yaratmamalı.
        assert_eq!(pct_decode("%ğ1%"), "%ğ1%");
        assert_eq!(pct_decode("a%2"), "a%2");
        assert_eq!(pct_decode("%41"), "A");
    }
}
