//! Uçtan uca: iki düğüm aynı makinede (127.0.0.1) eşleşir, pano, komut ve dosya aktarır.
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use kopru_core::*;

#[derive(Default)]
struct TestHost {
    events: Mutex<Vec<Event>>,
    cv: Condvar,
}

impl Host for TestHost {
    fn on_event(&self, event: Event) {
        self.events.lock().unwrap().push(event);
        self.cv.notify_all();
    }
    fn open_incoming(&self, _: String, _: String, _: u64) -> Result<OpenedFd, KopruError> {
        Err(KopruError::new("testte kullanılmaz"))
    }
    fn complete_incoming(&self, _: String, _: bool) -> Option<String> {
        None
    }
    fn open_outgoing(&self, _: String) -> Result<i32, KopruError> {
        Err(KopruError::new("testte kullanılmaz"))
    }
}

impl TestHost {
    fn wait<T>(&self, what: &str, secs: u64, mut f: impl FnMut(&Event) -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(secs);
        let mut seen = 0;
        let mut g = self.events.lock().unwrap();
        loop {
            while seen < g.len() {
                if let Some(v) = f(&g[seen]) {
                    return v;
                }
                seen += 1;
            }
            let left = deadline.checked_duration_since(Instant::now()).unwrap_or_else(|| panic!("zaman aşımı: {what}"));
            g = self.cv.wait_timeout(g, left).unwrap().0;
        }
    }

    fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
}

fn node(dir: &Path, name: &str, role: Role, platform: Platform, host: Arc<TestHost>) -> Arc<Node> {
    Node::new(
        NodeConfig {
            data_dir: dir.join("data").to_string_lossy().into(),
            device_name: name.into(),
            platform,
            role,
            port: 0,
            download_dir: Some(dir.join("indirilenler").to_string_lossy().into()),
        },
        host,
    )
    .unwrap()
}

fn loopback_uri(info: &PairingInfo, port: u16) -> String {
    let base = info.uri.split("&a=").next().unwrap();
    format!("{base}&a=127.0.0.1:{port}")
}

fn random_file(path: &Path, size: usize) -> blake3::Hash {
    let mut data = vec![0u8; size];
    // Hızlı, deterministik olmayan içerik.
    let mut x: u64 = rand_seed();
    for b in data.iter_mut() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *b = x as u8;
    }
    std::fs::write(path, &data).unwrap();
    blake3::hash(&data)
}

fn rand_seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64 | 1
}

fn hash_file(p: &Path) -> blake3::Hash {
    blake3::hash(&std::fs::read(p).unwrap())
}

fn incoming_done(h: &TestHost, secs: u64) -> (u64, PathBuf) {
    h.wait("gelen dosya tamamlandı", secs, |e| match e {
        Event::Transfer { info } if info.direction == Direction::Incoming && info.state == TransferState::Done => {
            Some((info.id, PathBuf::from(info.detail.clone().unwrap())))
        }
        _ => None,
    })
}

struct Pair {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    pc: Arc<Node>,
    pch: Arc<TestHost>,
    phone: Option<Arc<Node>>,
    phh: Arc<TestHost>,
}

fn paired() -> Pair {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let pch = Arc::new(TestHost::default());
    let phh = Arc::new(TestHost::default());
    let pc = node(&root.join("pc"), "Test PC", Role::Server, Platform::Desktop, pch.clone());
    let phone = node(&root.join("phone"), "Test Telefon", Role::Client, Platform::Android, phh.clone());
    let info = pc.pairing_info();
    assert_eq!(info.code.len(), 6);
    phone.pair_uri(loopback_uri(&info, pc.port())).unwrap();
    phh.wait("telefon eşleşti", 15, |e| matches!(e, Event::Paired { .. }).then_some(()));
    pch.wait("pc eşleşti", 15, |e| matches!(e, Event::Paired { .. }).then_some(()));
    phh.wait("telefon bağlı", 15, |e| matches!(e, Event::State { state: ConnState::Connected, .. }).then_some(()));
    assert_eq!(pc.peers().len(), 1);
    assert_eq!(phone.peers()[0].name, "Test PC");
    Pair { _tmp: tmp, root, pc, pch, phone: Some(phone), phh }
}

#[test]
fn pairing_clipboard_and_commands() {
    let p = paired();
    let phone = p.phone.as_ref().unwrap();

    // Pano telefon → PC
    assert!(phone.send_clipboard("merhaba ğüşıöç 🚀".into()));
    let got = p.pch.wait("pano PC'ye", 5, |e| match e {
        Event::ClipboardReceived { text } => Some(text.clone()),
        _ => None,
    });
    assert_eq!(got, "merhaba ğüşıöç 🚀");
    assert!(!phone.send_clipboard("merhaba ğüşıöç 🚀".into()), "aynı metin tekrar gitmemeli");

    // PC → telefon; telefon aldığı metni geri göndermemeli (döngü önleme).
    assert!(p.pc.send_clipboard("geri".into()));
    p.phh.wait("pano telefona", 5, |e| matches!(e, Event::ClipboardReceived { text } if text == "geri").then_some(()));
    assert!(!phone.send_clipboard("geri".into()));
    assert!(phone.send_clipboard("yeni".into()));

    // Komutlar ve fare datagramı
    assert!(phone.send_pc(PcCommand::Text { text: "İstanbul".into() }));
    p.pch.wait("pc komutu", 5, |e| matches!(e, Event::Pc { cmd: PcCommand::Text { text } } if text == "İstanbul").then_some(()));
    phone.send_pointer(12, -7);
    p.pch.wait("fare", 5, |e| matches!(e, Event::PointerMove { dx: 12, dy: -7 }).then_some(()));
    assert!(p.pc.send_phone(PhoneCommand::Ring { on: true }));
    p.phh.wait("çaldır", 5, |e| matches!(e, Event::Phone { cmd: PhoneCommand::Ring { on: true } }).then_some(()));
    assert!(phone.send_battery(57, true));
    p.pch.wait("pil", 5, |e| matches!(e, Event::Battery { level: 57, charging: true }).then_some(()));
    assert!(phone.send_notification(NotificationData {
        key: "k1".into(),
        package: "com.whatsapp".into(),
        app_name: "WhatsApp".into(),
        title: "Ali".into(),
        text: "Selam".into(),
        time: 1,
    }));
    p.pch.wait("bildirim", 5, |e| matches!(e, Event::Notification { data } if data.title == "Ali").then_some(()));
}

#[test]
fn wrong_code_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let pch = Arc::new(TestHost::default());
    let pc = node(&tmp.path().join("pc"), "PC", Role::Server, Platform::Desktop, pch);
    let info = pc.pairing_info();
    let bad = loopback_uri(&info, pc.port()).replace(&format!("c={}", info.code), "c=000000x");
    let hh = Arc::new(TestHost::default());
    let intruder = node(&tmp.path().join("x"), "Yabancı", Role::Client, Platform::Android, hh.clone());
    intruder.pair_uri(bad).unwrap();
    let reason = hh.wait("ret", 15, |e| match e {
        Event::PairFailed { reason } => Some(reason.clone()),
        _ => None,
    });
    assert!(reason.contains("reddedildi"), "{reason}");
    assert!(pc.peers().is_empty());
}

#[test]
fn wrong_fingerprint_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let pc = node(&tmp.path().join("pc"), "PC", Role::Server, Platform::Desktop, Arc::new(TestHost::default()));
    let info = pc.pairing_info();
    let uri = loopback_uri(&info, pc.port()).replace(&info.fingerprint, &"0".repeat(64));
    let hh = Arc::new(TestHost::default());
    let c = node(&tmp.path().join("x"), "C", Role::Client, Platform::Android, hh.clone());
    c.pair_uri(uri).unwrap();
    hh.wait("sahte sunucu reddi", 15, |e| matches!(e, Event::PairFailed { .. }).then_some(()));
    assert!(pc.peers().is_empty());
}

#[test]
fn file_and_folder_transfer_both_ways() {
    let p = paired();
    let phone = p.phone.as_ref().unwrap();
    // Telefon → PC: 20 MB
    let src = p.root.join("buyuk.bin");
    let h = random_file(&src, 20 * 1024 * 1024);
    phone.send_paths(vec![src.to_string_lossy().into()]).unwrap();
    let (_, dest) = incoming_done(&p.pch, 60);
    assert_eq!(hash_file(&dest), h);
    assert!(dest.ends_with("buyuk.bin"));
    p.phh.wait("gönderen tamam", 10, |e| matches!(e, Event::Transfer { info } if info.direction == Direction::Outgoing && info.state == TransferState::Done).then_some(()));

    // PC → telefon: klasör
    let dir = p.root.join("Proje");
    std::fs::create_dir_all(dir.join("alt")).unwrap();
    let h1 = random_file(&dir.join("a.txt"), 1000);
    let h2 = random_file(&dir.join("alt/b.txt"), 0);
    let ids = p.pc.send_paths(vec![dir.to_string_lossy().into()]).unwrap();
    assert_eq!(ids.len(), 2);
    let mut got = Vec::new();
    p.phh.wait("klasör", 30, |e| {
        if let Event::Transfer { info } = e {
            if info.direction == Direction::Incoming && info.state == TransferState::Done {
                got.push(PathBuf::from(info.detail.clone().unwrap()));
            }
        }
        (got.len() == 2).then_some(())
    });
    let dl = p.root.join("phone/indirilenler/Proje");
    assert_eq!(hash_file(&dl.join("a.txt")), h1);
    assert_eq!(hash_file(&dl.join("alt/b.txt")), h2);

    // Aynı ad tekrar → "a (1).txt"
    p.pch.clear();
    phone.send_paths(vec![src.to_string_lossy().into()]).unwrap();
    let (_, dest2) = incoming_done(&p.pch, 60);
    assert!(dest2.to_string_lossy().ends_with("buyuk (1).bin"), "{}", dest2.display());
}

#[test]
fn pause_resume_and_reconnect_resume() {
    let mut p = paired();
    let src = p.root.join("dev.bin");
    let size = 300 * 1024 * 1024;
    let h = random_file(&src, size);
    let phone = p.phone.clone().unwrap();
    let ids = phone.send_paths(vec![src.to_string_lossy().into()]).unwrap();
    let id = ids[0];

    // Alıcı %10'u geçince alıcı tarafından duraklat.
    p.pch.wait("ilerleme", 60, |e| matches!(e, Event::Transfer { info } if info.direction == Direction::Incoming && info.done > size as u64 / 10).then_some(()));
    p.pc.pause_transfer(id, Direction::Incoming).unwrap();
    p.phh.wait("gönderen duraklatıldı", 10, |e| matches!(e, Event::Transfer { info } if info.id == id && info.state == TransferState::Paused).then_some(()));
    std::thread::sleep(Duration::from_millis(500));
    let part_len = std::fs::read_dir(p.root.join("pc/indirilenler/.kopru-part")).unwrap().next().unwrap().unwrap().metadata().unwrap().len();
    assert!(part_len > 0 && part_len < size as u64, "yarım: {part_len}");

    // Alıcı devam ettirir: yarım dosyadan sürmeli.
    p.pch.clear();
    p.pc.resume_transfer(id, Direction::Incoming).unwrap();
    let resumed_from = p.pch.wait("devam teklifi", 10, |e| match e {
        Event::Transfer { info } if info.id == id && info.state == TransferState::Active => Some(info.done),
        _ => None,
    });
    assert!(resumed_from >= part_len.min(1), "kaldığı yerden devam etmeli: {resumed_from}");

    // Ortada telefonu tamamen kapat (süreç ölümü benzetimi) ve yeniden aç.
    p.pch.wait("ilerleme 2", 60, |e| matches!(e, Event::Transfer { info } if info.direction == Direction::Incoming && info.done > size as u64 / 2).then_some(()));
    p.phone = None;
    drop(phone);
    p.pch.wait("kopma algılandı", 70, |e| matches!(e, Event::State { state: ConnState::Disconnected, .. }).then_some(()));
    p.pch.clear();
    let phh2 = Arc::new(TestHost::default());
    let phone2 = node(&p.root.join("phone"), "Test Telefon", Role::Client, Platform::Android, phh2.clone());
    // Kuyruk kalıcı: aynı id ile yeniden teklif edilir ve %50'nin üstünden sürer.
    let again = p.pch.wait("yeniden teklif", 30, |e| match e {
        Event::Transfer { info } if info.id == id && info.state == TransferState::Active => Some(info.done),
        _ => None,
    });
    assert!(again > size as u64 / 2, "kopmadan sonra kaldığı yerden: {again}");
    let (rid, dest) = incoming_done(&p.pch, 120);
    assert_eq!(rid, id);
    assert_eq!(hash_file(&dest), h);
    phh2.wait("gönderen tamam", 10, |e| matches!(e, Event::Transfer { info } if info.id == id && info.state == TransferState::Done).then_some(()));
    assert!(std::fs::read_dir(p.root.join("pc/indirilenler/.kopru-part")).unwrap().next().is_none(), "yarım dosya kalmamalı");
    drop(phone2);
}

#[test]
fn cancel_removes_partial_and_queue() {
    let p = paired();
    let phone = p.phone.as_ref().unwrap();
    let src = p.root.join("iptal.bin");
    let size = 200 * 1024 * 1024;
    random_file(&src, size);
    let id = phone.send_paths(vec![src.to_string_lossy().into()]).unwrap()[0];
    p.pch.wait("ilerleme", 60, |e| matches!(e, Event::Transfer { info } if info.direction == Direction::Incoming && info.done > 0).then_some(()));
    phone.cancel_transfer(id, Direction::Outgoing);
    p.pch.wait("alıcı iptal", 10, |e| matches!(e, Event::Transfer { info } if info.id == id && info.state == TransferState::Cancelled).then_some(()));
    std::thread::sleep(Duration::from_millis(500));
    let left = std::fs::read_dir(p.root.join("pc/indirilenler/.kopru-part")).map(|d| d.count()).unwrap_or(0);
    assert_eq!(left, 0, "yarım dosya silinmeli");
    let q = std::fs::read_to_string(p.root.join("phone/data/queue.json")).unwrap();
    assert_eq!(q.trim(), "[]");
}

/// Ölü (yanıt vermeyen) adres önde, canlı adres arkada: bağlantı zaman aşımını beklememeli.
#[test]
fn dead_address_does_not_delay_connect() {
    let tmp = tempfile::tempdir().unwrap();
    let pc = node(&tmp.path().join("pc"), "PC", Role::Server, Platform::Desktop, Arc::new(TestHost::default()));
    // Hiç yanıt vermeyen UDP soketi: ICMP dönmez, bağlantı denemesi ancak zaman aşımıyla biter.
    let blackhole = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let dead = blackhole.local_addr().unwrap();
    let info = pc.pairing_info();
    let base = info.uri.split("&a=").next().unwrap();
    let uri = format!("{base}&a={dead},127.0.0.1:{}", pc.port());
    let hh = Arc::new(TestHost::default());
    let phone = node(&tmp.path().join("phone"), "Telefon", Role::Client, Platform::Android, hh.clone());
    let t0 = Instant::now();
    phone.pair_uri(uri).unwrap();
    hh.wait("telefon bağlı", 20, |e| matches!(e, Event::State { state: ConnState::Connected, .. }).then_some(()));
    let took = t0.elapsed();
    // Sıralı eski davranış en az 5 sn (adres başına zaman aşımı) harcardı.
    assert!(took < Duration::from_secs(4), "ölü adres bağlantıyı geciktirdi: {took:?}");
    println!("ölü+canlı adres bağlanma süresi: {took:?}");
    drop(blackhole);
}

/// Tarama yalnızca tetiklenince, sınırlı bir pencerede çalışır; pencere bitince kapanır ve durum
/// sabit `Disconnected` kalır (sürekli Connecting/Disconnected dönüşü yok).
#[test]
fn scan_window_is_bounded_and_state_stays_disconnected() {
    let tmp = tempfile::tempdir().unwrap();
    let hh = Arc::new(TestHost::default());
    let phone = node(&tmp.path().join("phone"), "Telefon", Role::Client, Platform::Android, hh.clone());
    // Tetikleyici yokken tarama açılmaz.
    std::thread::sleep(Duration::from_millis(300));
    assert!(!phone.is_scanning());
    assert_eq!(phone.state(), ConnState::Disconnected);

    phone.set_scan_window(Duration::from_millis(600));
    phone.rescan();
    hh.wait("tarama açıldı", 10, |e| matches!(e, Event::Scanning { on: true }).then_some(()));
    assert!(phone.is_scanning());
    hh.wait("tarama kapandı", 10, |e| matches!(e, Event::Scanning { on: false }).then_some(()));
    assert!(!phone.is_scanning());

    // Pencere bittikten sonra bir süre daha: yeni tarama açılmamalı, durum değişmemeli.
    std::thread::sleep(Duration::from_millis(1500));
    assert!(!phone.is_scanning());
    assert_eq!(phone.state(), ConnState::Disconnected);
    let evs = hh.events.lock().unwrap();
    let scans_on = evs.iter().filter(|e| matches!(e, Event::Scanning { on: true })).count();
    let states = evs.iter().filter(|e| matches!(e, Event::State { .. })).count();
    assert_eq!(scans_on, 1, "tek pencere açılmalı");
    assert_eq!(states, 0, "eşleşmiş bilgisayar yokken durum olayı üretilmemeli");
}

/// Eşleşmiş bilgisayar sonradan açılırsa, taramadan bağımsız sessiz denemelerle yine bağlanır;
/// sessiz denemeler `Connecting` göstermez.
#[test]
fn silent_retry_reconnects_without_scanning_or_connecting_state() {
    let mut p = paired();
    let phone = p.phone.take().unwrap();
    let phone_dir = p.root.join("phone");
    let port = p.pc.port();
    // Telefonu kapat; PC aynı portta çevrimdışı kalamaz (aynı süreç), bu yüzden PC'yi de kapatıp
    // aynı veri dizini/kimlikle yeniden aç: eşleşme kalıcı, telefon sessizce yeniden bağlanmalı.
    drop(phone);
    p.pch.wait("kopma", 70, |e| matches!(e, Event::State { state: ConnState::Disconnected, .. }).then_some(()));
    let _ = port;
    let phh2 = Arc::new(TestHost::default());
    let phone2 = node(&phone_dir, "Test Telefon", Role::Client, Platform::Android, phh2.clone());
    // Tarama tetiklenmedi: yalnızca kayıtlı adrese doğrudan deneme.
    phh2.wait("sessiz yeniden bağlanma", 20, |e| matches!(e, Event::State { state: ConnState::Connected, .. }).then_some(()));
    assert!(!phone2.is_scanning());
    let evs = phh2.events.lock().unwrap();
    assert!(!evs.iter().any(|e| matches!(e, Event::State { state: ConnState::Connecting, .. })), "sessiz deneme Connecting göstermemeli");
    assert!(!evs.iter().any(|e| matches!(e, Event::Scanning { .. })));
}
