//! T-Suite merkezi (hub): T-Bridge, `%LOCALAPPDATA%\T-Suite\apps` kaydındaki diğer uygulamaları izler, T-Link ile bağlanır,
//! beyaz listedeki olayları (download.*, ram.*, disk.*, scan.*, app.update.*) kısıtlayarak telefona iletir ve telefondan gelen
//! genel komutları (SuiteCmd) ilgili uygulamaya yönlendirir. Kendisi de "t-bridge" kimliğiyle T-Link sunucusudur.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kopru_core::{SuiteApp, ConnState};
use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};
use tlink::{Envelope, client::ReconnectingClient, registry, server::Server};
use tokio::sync::mpsc;

use crate::app::Shared;

/// Varsayılan (güvenli) iletme önekleri: manifestte ne `forwardPrefixes` ne de `events` önekleri varsa kullanılır.
pub const WHITELIST: &[&str] = &["download.", "ram.", "disk.", "scan.", "app.update."];
const CATALOG: &[(&str, &str)] = &[("t-ramcash", "T-RamCash"), ("t-downloader", "T-Downloader"), ("t-desktree", "T-DeskTree")];
const PROGRESS_MIN_GAP: Duration = Duration::from_millis(500); // ≤ 2 Hz / öğe

#[cfg(test)]
fn whitelisted(name: &str) -> bool {
    WHITELIST.iter().any(|p| name.starts_with(p))
}

/// Uygulamanın telefona iletilecek olay önekleri: manifest (`forwardPrefixes` ya da `events` önekleri) + `app.update.`
/// (güncelleme akışı için her zaman). Boş önek ("hepsi") ve boş sonuç → varsayılan beyaz liste. Saf mantık (birim testli).
pub fn forward_prefixes(m: &registry::Manifest) -> Vec<String> {
    let mut v: Vec<String> = m.effective_forward_prefixes().into_iter().filter(|p| !p.is_empty() && p.len() <= 64).collect();
    if v.is_empty() {
        return WHITELIST.iter().map(|s| s.to_string()).collect();
    }
    if !v.iter().any(|p| "app.update.".starts_with(p.as_str())) {
        v.push("app.update.".into());
    }
    v
}

pub fn prefix_match(prefixes: &[String], name: &str) -> bool {
    prefixes.iter().any(|p| name.starts_with(p.as_str()))
}

/// İlerleme olayları için kısıtlayıcı: aynı (uygulama, olay, öğe) en çok 2 Hz. Saf mantık (birim testli).
#[derive(Default)]
pub struct Throttle {
    last: HashMap<String, Instant>,
}

impl Throttle {
    pub fn allow(&mut self, app: &str, name: &str, item: &str, now: Instant) -> bool {
        if !name.ends_with(".progress") {
            return true;
        }
        let key = format!("{app}|{name}|{item}");
        match self.last.get(&key) {
            Some(t) if now.duration_since(*t) < PROGRESS_MIN_GAP => false,
            _ => {
                self.last.insert(key, now);
                if self.last.len() > 2000 {
                    self.last.retain(|_, t| now.duration_since(*t) < Duration::from_secs(30));
                }
                true
            }
        }
    }
}

struct Entry {
    m: registry::Manifest,
    running: bool,
    update: Option<String>,
    client: Option<ReconnectingClient>,
    prefixes: Vec<String>,
}

pub struct Hub {
    app: AppHandle,
    apps: Mutex<HashMap<String, Entry>>,
    server: Server,
    dir: std::path::PathBuf,
    last_sent: Mutex<Vec<SuiteApp>>,
    throttle: Mutex<Throttle>,
    ev_tx: mpsc::UnboundedSender<(String, Envelope)>,
    auto: Mutex<AutoState>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuiteCard {
    pub id: String,
    pub name: String,
    pub version: String,
    pub installed: bool,
    pub running: bool,
    pub update_available: Option<String>,
    pub releases_url: String,
}

fn shared(app: &AppHandle) -> Arc<Shared> {
    app.state::<Arc<Shared>>().inner().clone()
}

/// Kapalıyken başlatma argümanları: manifest `launchArgs`; yoksa bilinen uygulama bayrakları (arka planda/tepside açılır).
fn launch_args(m: &registry::Manifest) -> Vec<String> {
    if m.launch_args.is_none() {
        match m.id.as_str() {
            "t-downloader" | "t-desktree" => return vec!["--background".into()],
            "t-ramcash" => return vec!["--tray".into()],
            _ => {}
        }
    }
    m.effective_launch_args()
}

/// Otomatik başlatma adayı (saf veri; karar `AutoState::pick` içinde).
#[derive(Debug, Clone)]
pub struct AutoCand {
    pub id: String,
    /// Var olan çalıştırılabilir dosya; kurulu değilse `None`.
    pub exe: Option<std::path::PathBuf>,
    pub running: bool,
    pub args: Vec<String>,
}

const AUTO_BASE_DELAY: Duration = Duration::from_secs(30);
const AUTO_MAX_DELAY: Duration = Duration::from_secs(300);
const AUTO_MAX_TRIES: u32 = 5;

/// Otomatik başlatma durumu: uygulama başına deneme sayısı ve son deneme anı (hız sınırı).
#[derive(Default)]
pub struct AutoState {
    tries: HashMap<String, (u32, Instant)>,
}

impl AutoState {
    /// Yeni telefon oturumu: sayaçlar sıfırlanır.
    pub fn reset(&mut self) {
        self.tries.clear();
    }

    /// Şimdi başlatılacak adayları seçer ve denemeyi kaydeder. Kurulu + kapalı + argümanı bilinen olanlar;
    /// önceki denemeden beri üstel bekleme (30 sn, 60 sn, ... en çok 5 dk) geçmiş ve en çok 5 deneme.
    pub fn pick(&mut self, cands: &[AutoCand], now: Instant) -> Vec<AutoCand> {
        let mut out = Vec::new();
        for c in cands {
            if c.running {
                self.tries.remove(&c.id);
                continue;
            }
            // Argümansız başlatma pencere açıp odağı çalar: bilinmeyen uygulamalar otomatik açılmaz.
            if c.exe.is_none() || c.args.is_empty() {
                continue;
            }
            if let Some((n, at)) = self.tries.get(&c.id) {
                let wait = AUTO_BASE_DELAY.saturating_mul(1u32 << (*n - 1).min(8)).min(AUTO_MAX_DELAY);
                if *n >= AUTO_MAX_TRIES || now.duration_since(*at) < wait {
                    continue;
                }
            }
            let n = self.tries.get(&c.id).map_or(0, |t| t.0);
            self.tries.insert(c.id.clone(), (n + 1, now));
            out.push(c.clone());
        }
        out
    }

    /// Deneme hakkı tükendi mi (telefonda elle "Aç" yedeği gösterilir).
    pub fn failed(&self, id: &str) -> bool {
        self.tries.get(id).is_some_and(|t| t.0 >= AUTO_MAX_TRIES)
    }
}

/// Yükleyicinin düzeni: `%LOCALAPPDATA%\Programs\T-Suite\<Ad>\<Ad>.exe` (manifest yoksa yedek yol).
fn installer_exe(display: &str) -> Option<std::path::PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from)?;
    let p = base.join("Programs").join("T-Suite").join(display).join(format!("{display}.exe"));
    p.exists().then_some(p)
}

impl Hub {
    fn snapshot(&self) -> Vec<SuiteApp> {
        let g = self.apps.lock().unwrap();
        let mut v: Vec<SuiteApp> = g
            .values()
            .map(|e| SuiteApp { id: e.m.id.clone(), name: e.m.name.clone(), version: e.m.version.clone(), running: e.running, update_available: e.update.clone() })
            .collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    pub fn cards(&self) -> Vec<SuiteCard> {
        let g = self.apps.lock().unwrap();
        let mut out: Vec<SuiteCard> = Vec::new();
        for (id, name) in CATALOG {
            let e = g.get(*id);
            out.push(SuiteCard {
                id: (*id).into(),
                name: e.map(|e| e.m.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| (*name).into()),
                version: e.map(|e| e.m.version.clone()).unwrap_or_default(),
                installed: e.is_some(),
                running: e.is_some_and(|e| e.running),
                update_available: e.and_then(|e| e.update.clone()),
                releases_url: e
                    .and_then(|e| e.m.releases_repo.as_ref())
                    .map(|r| format!("https://github.com/{r}/releases"))
                    .unwrap_or_else(|| format!("https://github.com/taha-sa/{id}-releases/releases")),
            });
        }
        // Kataloğda olmayan (gelecekteki) uygulamalar otomatik görünür
        for e in g.values() {
            if !CATALOG.iter().any(|(id, _)| *id == e.m.id) {
                out.push(SuiteCard {
                    id: e.m.id.clone(),
                    name: if e.m.name.is_empty() { e.m.id.clone() } else { e.m.name.clone() },
                    version: e.m.version.clone(),
                    installed: true,
                    running: e.running,
                    update_available: e.update.clone(),
                    releases_url: e.m.releases_repo.as_ref().map(|r| format!("https://github.com/{r}/releases")).unwrap_or_default(),
                });
            }
        }
        out
    }

    /// `refresh` senkron dosya G/Ç yapar: async bağlamda çalışma zamanı iş parçacığını bloklamasın.
    async fn refresh_async(self: &Arc<Self>) {
        let h = self.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || h.refresh()).await;
    }

    /// Kayıt klasörünü tarar, istemcileri açar/kapatır; liste değiştiyse telefona yollar.
    fn refresh(self: &Arc<Self>) {
        let all = registry::read_all(&self.dir);
        {
            let mut g = self.apps.lock().unwrap();
            let ids: Vec<String> = all.iter().map(|m| m.id.clone()).collect();
            g.retain(|id, _| ids.contains(id));
            for m in all {
                if m.id == "t-bridge" {
                    continue;
                }
                let running = registry::pid_alive(m.pid);
                let id = m.id.clone();
                let pf = forward_prefixes(&m);
                let e = g.entry(id.clone()).or_insert_with(|| Entry { m: m.clone(), running, update: None, client: None, prefixes: pf.clone() });
                e.m = m;
                e.running = running;
                if e.prefixes != pf {
                    e.prefixes = pf; // manifest önekleri değişti: istemci yeni aboneliklerle yeniden açılır
                    e.client = None;
                }
                if running && e.client.is_none() {
                    e.client = Some(ReconnectingClient::spawn(&id, e.prefixes.clone(), self.ev_tx.clone(), Duration::from_secs(2)));
                } else if !running {
                    e.client = None;
                    e.update = e.update.take().filter(|_| false);
                }
            }
        }
        let snap = self.snapshot();
        let mut last = self.last_sent.lock().unwrap();
        if *last != snap {
            *last = snap.clone();
            if let Some(n) = shared(&self.app).node() {
                n.send_suite_apps(snap);
            }
        }
    }

    fn on_event(&self, app: String, e: Envelope) {
        let Some(name) = e.name.clone() else { return };
        let allowed = self.apps.lock().unwrap().get(&app).is_some_and(|en| prefix_match(&en.prefixes, &name));
        if !allowed {
            return;
        }
        let data = e.data.clone().unwrap_or(Value::Null);
        let item = data.get("id").and_then(Value::as_str).unwrap_or("");
        if !self.throttle.lock().unwrap().allow(&app, &name, item, Instant::now()) {
            return;
        }
        if name == "app.update.available"
            && let Some(v) = data.get("version").and_then(Value::as_str)
            && let Some(en) = self.apps.lock().unwrap().get_mut(&app)
        {
            en.update = Some(v.to_string());
        }
        if let Some(n) = shared(&self.app).node() {
            n.send_suite_event(app, name, data.to_string());
        }
    }

    // ---------------------------------------------------------------- komutlar

    fn auto_candidates(&self) -> Vec<AutoCand> {
        let g = self.apps.lock().unwrap();
        let mut out: Vec<AutoCand> = Vec::new();
        for (id, name) in CATALOG {
            let (exe, running, args) = match g.get(*id) {
                Some(e) => (
                    Some(std::path::PathBuf::from(&e.m.exe_path)).filter(|p| p.exists()).or_else(|| installer_exe(name)),
                    e.running,
                    launch_args(&e.m),
                ),
                None => (installer_exe(name), false, launch_args(&registry::Manifest { id: (*id).into(), ..Default::default() })),
            };
            out.push(AutoCand { id: (*id).into(), exe, running, args });
        }
        for e in g.values().filter(|e| e.m.id != "t-bridge" && !CATALOG.iter().any(|(id, _)| *id == e.m.id)) {
            out.push(AutoCand {
                id: e.m.id.clone(),
                exe: Some(std::path::PathBuf::from(&e.m.exe_path)).filter(|p| p.exists()),
                running: e.running,
                args: launch_args(&e.m),
            });
        }
        out
    }

    /// Ekosistemi otomatik başlat: telefon bağlıysa ve ayar açıksa, kurulu ama kapalı uygulamaları arka planda açar.
    /// Odak çalmaz (uygulamanın kendi `--tray`/`--background` bayrağı); hız sınırlı; hatalar günlüğe yazılır.
    pub fn auto_start(self: &Arc<Self>) {
        let sh = shared(&self.app);
        if !crate::app::lk(&sh.settings).auto_ecosystem {
            return;
        }
        if !sh.node().is_some_and(|n| n.state() == ConnState::Connected) {
            return;
        }
        let picks = self.auto.lock().unwrap().pick(&self.auto_candidates(), Instant::now());
        for c in picks {
            let Some(exe) = c.exe else { continue };
            let mut cmd = std::process::Command::new(&exe);
            cmd.args(&c.args);
            if let Some(d) = exe.parent() {
                cmd.current_dir(d);
            }
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x0000_0008 | 0x0000_0200); // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
            }
            match cmd.spawn() {
                Ok(_) => log::info!("ekosistem: {} arka planda başlatıldı", c.id),
                Err(e) => log::warn!("ekosistem: {} başlatılamadı: {e}", c.id),
            }
        }
    }

    fn launch(&self, id: &str, extra: &[String]) -> Result<(), String> {
        let exe = {
            let g = self.apps.lock().unwrap();
            g.get(id).map(|e| e.m.exe_path.clone()).ok_or_else(|| "uygulama kurulu değil".to_string())?
        };
        if !std::path::Path::new(&exe).exists() {
            return Err("uygulama dosyası bulunamadı".into());
        }
        let mut c = std::process::Command::new(&exe);
        c.args(extra);
        if let Some(d) = std::path::Path::new(&exe).parent() {
            c.current_dir(d);
        }
        c.spawn().map(|_| ()).map_err(|e| e.to_string())
    }

    async fn app_cmd(self: &Arc<Self>, id: &str, name: &str, data: Option<Value>) -> Result<Value, String> {
        if id == "t-bridge" {
            return match name {
                "suite.update" => Ok(json!({ "started": Box::pin(self.update(None)).await })),
                "suite.check" => {
                    Box::pin(self.check_updates()).await;
                    Ok(json!({}))
                }
                _ => self.local_cmd(name, data),
            };
        }
        if name == "app.launch" {
            self.refresh_async().await;
            return self.launch(id, &[]).map(|_| json!({ "launched": true }));
        }
        let connected = |h: &Hub| h.apps.lock().unwrap().get(id).and_then(|e| e.client.as_ref().map(|c| c.is_connected())).unwrap_or(false);
        if !connected(self) {
            self.refresh_async().await;
            let args = self.apps.lock().unwrap().get(id).map(|e| launch_args(&e.m)).unwrap_or_default();
            self.launch(id, &args)?;
            let deadline = Instant::now() + Duration::from_secs(12);
            loop {
                self.refresh_async().await;
                if connected(self) {
                    break;
                }
                if Instant::now() > deadline {
                    return Err("uygulama açılamadı veya yanıt vermedi".into());
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
        // Komut: istemciyi kilit dışında kullanmak için kimliğe göre yeniden bulunur.
        let c = {
            let g = self.apps.lock().unwrap();
            g.get(id).and_then(|e| e.client.as_ref()).and_then(|c| c.handle())
        }
        .ok_or("bağlı değil")?;
        c.cmd(name, data, Duration::from_secs(8)).await
    }

    fn local_cmd(&self, name: &str, data: Option<Value>) -> Result<Value, String> {
        let sh = shared(&self.app);
        match name {
            "file.present" => {
                // Telefon, sunum dosyasını göndermeden önce adı bildirir; dosya bitince PC tam ekran açar (on_incoming_done).
                let name = data.as_ref().and_then(|d| d.get("name")).and_then(Value::as_str).ok_or("name gerekli")?;
                if !is_presentable_name(name) {
                    return Err("not_allowed".into());
                }
                PENDING_PRESENT.lock().unwrap().add(name, Instant::now());
                Ok(json!({}))
            }
            "file.send" => {
                let path = data.as_ref().and_then(|d| d.get("path")).and_then(Value::as_str).ok_or("path gerekli")?;
                let open = data.as_ref().and_then(|d| d.get("open")).and_then(Value::as_bool).unwrap_or(false);
                if !std::path::Path::new(path).is_file() {
                    return Err("dosya bulunamadı".into());
                }
                let n = sh.node().ok_or("çekirdek yok")?;
                if n.state() != ConnState::Connected {
                    return Err("telefon bağlı değil".into());
                }
                if open {
                    // Telefona "bu dosya bitince aç" bilgisi (suite olayı; eski telefon yok sayar). Dosyadan ÖNCE gider.
                    let name = std::path::Path::new(path).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
                    n.send_suite_event("t-bridge".into(), "file.open".into(), json!({ "name": name }).to_string());
                }
                let ids = n.send_paths(vec![path.to_string()]).map_err(|e| e.to_string())?;
                Ok(json!({ "ids": ids }))
            }
            "phone.status" => {
                let n = sh.node();
                let connected = n.is_some_and(|n| n.state() == ConnState::Connected);
                let name = n.and_then(|n| n.current_peer()).map(|p| p.name).unwrap_or_default();
                Ok(json!({ "connected": connected, "name": name }))
            }
            "app.show" => {
                crate::show_main(&self.app);
                Ok(json!({}))
            }
            "app.version" => Ok(json!({ "version": self.app.package_info().version.to_string(), "protocol": 1 })),
            _ => Err("unknown_command".into()),
        }
    }

    /// UI için: uygulamaya basit komut (adı verilen; veri yok), sonucu hata metnine çevirir.
    pub async fn run(self: &Arc<Self>, id: &str, name: &str) -> Result<(), String> {
        self.app_cmd(id, name, None).await.map(|_| ())
    }

    /// "Hepsini güncelle" / tek uygulama: çalışana app.update.install, kapalıya `--update` ile başlat.
    pub async fn update(self: &Arc<Self>, only: Option<String>) -> usize {
        let targets: Vec<(String, bool)> = {
            let g = self.apps.lock().unwrap();
            g.values()
                .filter(|e| e.update.is_some() && only.as_ref().is_none_or(|o| *o == e.m.id))
                .map(|e| (e.m.id.clone(), e.running && e.client.as_ref().is_some_and(|c| c.is_connected())))
                .collect()
        };
        let mut n = 0;
        for (id, live) in targets {
            let ok = if live { self.app_cmd(&id, "app.update.install", None).await.is_ok() } else { self.launch(&id, &["--update".to_string()]).is_ok() };
            if ok {
                n += 1;
            }
        }
        n
    }

    /// Çalışan uygulamalardan güncelleme sor (app.update.check).
    pub async fn check_updates(self: &Arc<Self>) {
        let ids: Vec<String> = {
            let g = self.apps.lock().unwrap();
            g.values().filter(|e| e.running && e.m.commands.iter().any(|c| c == "app.update.check")).map(|e| e.m.id.clone()).collect()
        };
        for id in ids {
            if let Ok(r) = self.app_cmd(&id, "app.update.check", None).await {
                let v = if r.get("available").and_then(Value::as_bool) == Some(true) { r.get("version").and_then(Value::as_str).map(String::from) } else { None };
                if let Some(e) = self.apps.lock().unwrap().get_mut(&id) {
                    e.update = v;
                }
            }
        }
        self.refresh_async().await;
    }
}

static HUB: std::sync::OnceLock<Arc<Hub>> = std::sync::OnceLock::new();

pub fn hub() -> Option<&'static Arc<Hub>> {
    HUB.get()
}

/// Node başladıktan sonra çağrılır: sunucuyu açar, kaydı yazar, izleme döngüsünü başlatır.
pub fn start(app: &AppHandle) {
    let version = app.package_info().version.to_string();
    let server = Server::new("t-bridge", &version, &["phone.connected", "phone.disconnected"], &["file.send", "phone.status", "app.show", "app.version"]);
    let (tx, mut rx) = mpsc::unbounded_channel::<(String, Envelope)>();
    let hub = Arc::new(Hub {
        app: app.clone(),
        apps: Mutex::new(HashMap::new()),
        server: server.clone(),
        dir: registry::default_dir(),
        last_sent: Mutex::new(Vec::new()),
        throttle: Mutex::new(Throttle::default()),
        ev_tx: tx,
        auto: Mutex::new(AutoState::default()),
    });
    for cmd in ["file.send", "phone.status", "app.show", "app.version"] {
        let h = hub.clone();
        server.on_command(cmd, move |d| {
            let h = h.clone();
            async move { h.local_cmd(cmd, d) }
        });
    }
    let _ = HUB.set(hub.clone());
    let h2 = hub.clone();
    tauri::async_runtime::spawn(async move {
        let _ = server.start();
        let m = registry::Manifest {
            id: "t-bridge".into(),
            name: "T-Bridge".into(),
            version: h2.app.package_info().version.to_string(),
            exe_path: std::env::current_exe().map(|p| p.to_string_lossy().into()).unwrap_or_default(),
            pipe_name: "TSuite.t-bridge".into(),
            events: vec!["phone.connected".into(), "phone.disconnected".into()],
            commands: vec!["file.send".into(), "phone.status".into(), "app.show".into(), "app.version".into()],
            releases_repo: Some("taha-sa/t-bridge-releases".into()),
            icon: None,
            pid: std::process::id() as i32,
            forward_prefixes: None,
            launch_args: None,
        };
        let _ = registry::write(&h2.dir, &m);
        let h3 = h2.clone();
        tauri::async_runtime::spawn(async move {
            while let Some((app, e)) = rx.recv().await {
                h3.on_event(app, e);
            }
        });
        loop {
            h2.refresh_async().await;
            let h4 = h2.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || h4.auto_start()).await;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}

pub fn shutdown() {
    if let Some(h) = HUB.get() {
        registry::remove(&h.dir, "t-bridge");
    }
}

/// Telefon bağlantı durumu değişti: olayı yayınla, listeyi gönder.
pub fn on_phone_state(connected: bool) {
    let Some(h) = HUB.get() else { return };
    h.server.publish(if connected { "phone.connected" } else { "phone.disconnected" }, Some(json!({})));
    if connected {
        *h.last_sent.lock().unwrap() = Vec::new(); // yeni oturum: listeyi baştan gönder
        h.auto.lock().unwrap().reset();
        let h = h.clone();
        tauri::async_runtime::spawn_blocking(move || {
            h.refresh();
            h.auto_start();
        });
    }
}

/// Telefondan gelmesi beklenen sunum dosyaları (ad -> bildirim anı). Saf mantık, birim testli.
#[derive(Default)]
pub struct PendingNames {
    items: Vec<(String, Instant)>,
}

impl PendingNames {
    const TTL: Duration = Duration::from_secs(15 * 60);

    pub fn add(&mut self, name: &str, now: Instant) {
        self.items.retain(|(_, t)| now.duration_since(*t) < Self::TTL);
        self.items.push((name.to_string(), now));
    }

    /// Ad (dizin kısmı yok sayılır, büyük/küçük harf duyarsız) bekleniyorsa kaydı düşer ve true döner.
    pub fn take(&mut self, name: &str, now: Instant) -> bool {
        let base = name.rsplit(['/', '\\']).next().unwrap_or(name).to_lowercase();
        self.items.retain(|(_, t)| now.duration_since(*t) < Self::TTL);
        match self.items.iter().position(|(n, _)| n.to_lowercase() == base) {
            Some(i) => {
                self.items.remove(i);
                true
            }
            None => false,
        }
    }
}

static PENDING_PRESENT: Mutex<PendingNames> = Mutex::new(PendingNames { items: Vec::new() });

/// Telefondan açılabilecek (sunum/belge) uzantılar. Yürütülebilir/betik/kısayol türleri asla burada olmaz.
pub const PRESENT_EXTS: &[&str] = &["pdf", "ppt", "pptx", "pps", "ppsx", "odp"];

/// Ad (dizin kısmı yok sayılır) izin listesindeki bir uzantıyla bitiyor mu?
pub fn is_presentable_name(name: &str) -> bool {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    // Alternatif veri akışı ("a.pdf:evil.exe") ve boş ad reddedilir.
    if base.is_empty() || base.contains(':') {
        return false;
    }
    match std::path::Path::new(base).extension().and_then(|e| e.to_str()) {
        Some(e) => PRESENT_EXTS.contains(&e.to_lowercase().as_str()),
        None => false,
    }
}

/// Açılacak yolu doğrular: uzantı izin listesinde, dosya gerçek (symlink/reparse değil) ve kanonik yolu `recv_dir` içinde olmalı.
/// Başarıda kanonik yolu döndürür.
pub fn validate_present_path(path: &str, recv_dir: &std::path::Path) -> Result<std::path::PathBuf, String> {
    if !is_presentable_name(path) {
        return Err("uzantı izinli değil".into());
    }
    let p = std::path::Path::new(path);
    let meta = std::fs::symlink_metadata(p).map_err(|e| e.to_string())?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err("dosya normal değil".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err("reparse noktası reddedildi".into());
        }
    }
    let canon = std::fs::canonicalize(p).map_err(|e| e.to_string())?;
    let dir = std::fs::canonicalize(recv_dir).map_err(|e| e.to_string())?;
    if !canon.starts_with(&dir) || canon == dir {
        return Err("dosya alınan dosyalar klasörü dışında".into());
    }
    Ok(canon)
}

/// Telefona sunum sonucu (`file.presented` suite olayı; eski telefon yok sayar): Viewer'a opened/failed iletilir.
fn notify_present_result(name: &str, ok: bool) {
    let Some(h) = HUB.get() else { return };
    if let Some(n) = shared(&h.app).node() {
        n.send_suite_event("t-bridge".into(), "file.presented".into(), json!({ "name": name, "ok": ok }).to_string());
    }
}

/// Telefondan bir dosya tamamlandı: sunum için beklenen dosyaysa PC'de tam ekran aç.
pub fn on_incoming_done(name: &str, path: Option<&str>, recv_dir: &std::path::Path) {
    let Some(path) = path else { return };
    if !PENDING_PRESENT.lock().unwrap().take(name, Instant::now()) {
        return;
    }
    let name = name.to_string();
    let Ok(safe) = validate_present_path(path, recv_dir) else {
        notify_present_result(&name, false);
        return;
    };
    let path = safe.to_string_lossy().into_owned();
    std::thread::spawn(move || {
        let ok = crate::win::system::present_file(&path);
        notify_present_result(&name, ok);
    });
}

/// Telefon kaynaklı komut izin listesi. `file.send` (ve listede olmayan her şey) telefondan çağrılamaz.
pub fn phone_cmd_allowed(app_id: &str, name: &str) -> bool {
    if matches!(name, "file.present" | "suite.update" | "suite.check") {
        return app_id == "t-bridge";
    }
    matches!(
        name,
        "download.add"
            | "download.pause"
            | "download.resume"
            | "download.list"
            | "ram.clean"
            | "ram.status"
            | "disk.status"
            | "scan.start"
            | "app.show"
            | "app.launch"
            | "app.update.check"
            | "app.update.install"
    )
}

/// Telefondan SuiteCmd geldi.
pub fn on_phone_cmd(app: &AppHandle, req: u64, app_id: String, name: String, json_str: String) {
    let Some(h) = HUB.get().cloned() else { return };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let data: Option<Value> = serde_json::from_str(&json_str).ok().filter(|v: &Value| !v.is_null());
        let r = if phone_cmd_allowed(&app_id, &name) { h.app_cmd(&app_id, &name, data).await } else { Err("not_allowed".to_string()) };
        if let Some(n) = shared(&app).node() {
            match r {
                Ok(v) => n.send_suite_reply(req, true, v.to_string(), None),
                Err(e) => n.send_suite_reply(req, false, String::new(), Some(e)),
            };
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn present_allowlist_extensions() {
        for ok in ["a.pdf", "X.PPTX", "d/e.ppt", "c\\s.ppsx", "m.pps", "o.odp"] {
            assert!(is_presentable_name(ok), "{ok}");
        }
        for bad in ["a.exe", "a.bat", "a.lnk", "a.js", "a.msi", "a", "a.pdf.exe", "a.pdf:evil.exe", "a.pptm", ""] {
            assert!(!is_presentable_name(bad), "{bad}");
        }
    }

    #[test]
    fn present_path_must_be_inside_recv_dir() {
        let base = std::env::temp_dir().join(format!("kopru_pres_{}", std::process::id()));
        let recv = base.join("recv");
        std::fs::create_dir_all(&recv).unwrap();
        let ok = recv.join("s.pdf");
        std::fs::write(&ok, b"x").unwrap();
        let outside = base.join("o.pdf");
        std::fs::write(&outside, b"x").unwrap();
        let exe = recv.join("x.exe");
        std::fs::write(&exe, b"x").unwrap();
        assert!(validate_present_path(ok.to_str().unwrap(), &recv).is_ok());
        assert!(validate_present_path(outside.to_str().unwrap(), &recv).is_err());
        assert!(validate_present_path(exe.to_str().unwrap(), &recv).is_err());
        // ".." ile kaçış
        let esc = recv.join("..").join("o.pdf");
        assert!(validate_present_path(esc.to_str().unwrap(), &recv).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn phone_allowlist() {
        assert!(phone_cmd_allowed("t-downloader", "download.add"));
        assert!(phone_cmd_allowed("t-ramcash", "ram.clean"));
        assert!(phone_cmd_allowed("t-bridge", "file.present"));
        assert!(phone_cmd_allowed("t-bridge", "suite.update"));
        assert!(!phone_cmd_allowed("t-bridge", "file.send"));
        assert!(!phone_cmd_allowed("t-downloader", "file.send"));
        assert!(!phone_cmd_allowed("t-ramcash", "file.present"));
        assert!(!phone_cmd_allowed("t-bridge", "phone.status"));
        assert!(!phone_cmd_allowed("t-x", "anything"));
    }

    #[test]
    fn pending_present_matches_once_by_basename() {
        let mut p = PendingNames::default();
        let t0 = Instant::now();
        p.add("Sunum.PDF", t0);
        assert!(!p.take("baska.pdf", t0));
        assert!(p.take("klasor/sunum.pdf", t0));
        assert!(!p.take("sunum.pdf", t0), "bir kez tüketilir");
    }

    #[test]
    fn whitelist_blocks_unknown_prefixes() {
        for ok in ["download.progress", "ram.status", "disk.low", "scan.finished", "app.update.available"] {
            assert!(whitelisted(ok), "{ok}");
        }
        for no in ["clipboard.text", "app.show", "secret.x", "phone.connected"] {
            assert!(!whitelisted(no), "{no}");
        }
    }

    #[test]
    fn forward_prefixes_from_manifest() {
        let mut m = registry::Manifest { id: "t-fake".into(), events: vec!["fake.progress".into(), "fake.done".into()], ..Default::default() };
        let p = forward_prefixes(&m);
        assert!(prefix_match(&p, "fake.progress") && prefix_match(&p, "app.update.available"));
        assert!(!prefix_match(&p, "download.progress"), "events dışı önek geçmez");
        m.forward_prefixes = Some(vec!["other.".into()]);
        let p = forward_prefixes(&m);
        assert!(prefix_match(&p, "other.x") && !prefix_match(&p, "fake.progress"));
        // manifestte hiçbir şey yoksa varsayılan beyaz liste (mevcut uygulamalar değişmeden çalışır)
        let p = forward_prefixes(&registry::Manifest { id: "t-old".into(), ..Default::default() });
        assert!(prefix_match(&p, "download.progress") && prefix_match(&p, "ram.status") && !prefix_match(&p, "clipboard.text"));
        // boş önek "hepsi" anlamına gelemez
        let p = forward_prefixes(&registry::Manifest { forward_prefixes: Some(vec!["".into()]), ..Default::default() });
        assert!(!prefix_match(&p, "secret.x"));
    }

    #[test]
    fn launch_args_from_manifest() {
        let mut m = registry::Manifest { id: "t-fake".into(), ..Default::default() };
        assert!(launch_args(&m).is_empty());
        m.launch_args = Some(vec!["--tray".into()]);
        assert_eq!(launch_args(&m), vec!["--tray".to_string()]);
        assert_eq!(launch_args(&registry::Manifest { id: "t-downloader".into(), ..Default::default() }), vec!["--background".to_string()]);
        assert_eq!(launch_args(&registry::Manifest { id: "t-ramcash".into(), ..Default::default() }), vec!["--tray".to_string()]);
        assert_eq!(launch_args(&registry::Manifest { id: "t-desktree".into(), ..Default::default() }), vec!["--background".to_string()]);
    }

    fn cand(id: &str, installed: bool, running: bool, args: &[&str]) -> AutoCand {
        AutoCand {
            id: id.into(),
            exe: installed.then(|| std::path::PathBuf::from("x.exe")),
            running,
            args: args.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn auto_start_picks_only_installed_stopped_with_args() {
        let mut st = AutoState::default();
        let now = Instant::now();
        let got = st.pick(&[cand("a", true, false, &["--tray"]), cand("b", false, false, &["--x"]), cand("c", true, true, &["--x"]), cand("d", true, false, &[])], now);
        assert_eq!(got.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), vec!["a"]);
    }

    #[test]
    fn auto_start_never_twice_and_rate_limited() {
        let mut st = AutoState::default();
        let t0 = Instant::now();
        let a = [cand("a", true, false, &["--tray"])];
        assert_eq!(st.pick(&a, t0).len(), 1);
        assert!(st.pick(&a, t0 + Duration::from_secs(5)).is_empty());
        assert_eq!(st.pick(&a, t0 + Duration::from_secs(31)).len(), 1);
        assert!(st.pick(&a, t0 + Duration::from_secs(31 + 59)).is_empty());
        assert_eq!(st.pick(&a, t0 + Duration::from_secs(31 + 61)).len(), 1);
    }

    #[test]
    fn auto_start_gives_up_after_max_tries_until_reset() {
        let mut st = AutoState::default();
        let mut t = Instant::now();
        let a = [cand("a", true, false, &["--tray"])];
        let mut n = 0;
        for _ in 0..20 {
            n += st.pick(&a, t).len();
            t += Duration::from_secs(400);
        }
        assert_eq!(n, AUTO_MAX_TRIES as usize);
        assert!(st.failed("a"));
        st.reset();
        assert!(!st.failed("a"));
        assert_eq!(st.pick(&a, t).len(), 1);
    }

    #[test]
    fn auto_start_running_clears_counter() {
        let mut st = AutoState::default();
        let t = Instant::now();
        assert_eq!(st.pick(&[cand("a", true, false, &["--tray"])], t).len(), 1);
        assert!(st.pick(&[cand("a", true, true, &["--tray"])], t).is_empty());
        assert_eq!(st.pick(&[cand("a", true, false, &["--tray"])], t).len(), 1);
    }

    #[test]
    fn progress_throttled_to_2hz_per_item_others_pass() {
        let mut t = Throttle::default();
        let t0 = Instant::now();
        assert!(t.allow("d", "download.progress", "1", t0));
        assert!(!t.allow("d", "download.progress", "1", t0 + Duration::from_millis(100)));
        assert!(!t.allow("d", "download.progress", "1", t0 + Duration::from_millis(499)));
        assert!(t.allow("d", "download.progress", "1", t0 + Duration::from_millis(500)));
        assert!(t.allow("d", "download.progress", "2", t0 + Duration::from_millis(100)), "başka öğe bağımsız");
        assert!(t.allow("d", "download.completed", "1", t0), "ilerleme olmayanlar kısıtlanmaz");
        assert!(t.allow("d", "download.completed", "1", t0));
    }
}
