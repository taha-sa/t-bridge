//! Uzaktan ekran görüntüleme oturumu (PC tarafı): izin, ardışık düzen yaşam döngüsü, gösterge (tepsi + açılır hap), mutlak işaretçi.
use std::sync::Arc;

use kopru_core::{MonitorInfo, Node, ScreenCmd};
use std::time::Duration;

use tauri::{AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::app::{Shared, lk};
use crate::win::screen::{self, Params, Pipeline, Sink};

pub const PILL: &str = "pill";
pub const CONSENT: &str = "consent";
/// Yanıt gelmezse onay penceresi bu sürede kendiliğinden "Reddet" sayılır.
pub const CONSENT_TIMEOUT: Duration = Duration::from_secs(60);

/// Onay penceresindeki karar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Allow,
    Always,
    Deny,
}

impl Choice {
    /// Bilinmeyen/boş değer güvenli tarafta kalır: Reddet.
    pub fn parse(s: &str) -> Choice {
        match s {
            "allow" => Choice::Allow,
            "always" => Choice::Always,
            _ => Choice::Deny,
        }
    }
}

/// Karar bekleyen onay isteği (oturum kimliğine `id` ile bağlı).
#[derive(Debug, Clone)]
pub struct PendingConsent {
    pub id: u64,
    pub peer_id: String,
    pub peer_name: String,
    pub params: Params,
}

/// Kararın sonucu.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Paylaşımı başlat; `remember` doluysa o telefonu "her zaman izin ver" listesine ekle.
    Start { params: Params, remember: Option<String> },
    Reject,
    /// Bayat: istek sahibi telefon artık bağlı değil / başka cihaz bağlandı.
    Ignore,
}

pub fn outcome(p: &PendingConsent, choice: Choice, current_peer: Option<&str>) -> Outcome {
    if current_peer != Some(p.peer_id.as_str()) {
        return Outcome::Ignore;
    }
    match choice {
        Choice::Deny => Outcome::Reject,
        Choice::Allow => Outcome::Start { params: p.params, remember: None },
        Choice::Always => Outcome::Start { params: p.params, remember: Some(p.peer_id.clone()) },
    }
}

#[derive(Default)]
pub struct ScreenState {
    pub pipeline: Option<Pipeline>,
    /// Açık onay penceresi (en çok bir tane).
    pub consent: Option<PendingConsent>,
    /// Onay kimliği sayacı: her istek yeni kimlik alır, bayat sonuçlar kimlikten ayıklanır.
    pub consent_seq: u64,
    pub sharing: bool,
    pub monitor: u32,
    /// Hapın son konumu (fiziksel piksel); kapanırken kalıcı dosyaya yazılır.
    pub pill_pos: Option<(i32, i32)>,
    /// Başlatma oturum sayacı (bayat durum bildirimlerini ayıklar).
    pub session: u64,
    /// Hızlı hata bildiren son oturum (pipeline kaydedilmeden önce gelmiş olabilir).
    pub failed: u64,
}

impl ScreenState {
    /// Onay isteği aç: zaten açıksa None (aynı anda tek pencere).
    pub fn begin_consent(&mut self, peer_id: String, peer_name: String, params: Params) -> Option<u64> {
        if self.consent.is_some() {
            return None;
        }
        self.consent_seq += 1;
        let id = self.consent_seq;
        self.consent = Some(PendingConsent { id, peer_id, peer_name, params });
        Some(id)
    }
    /// Karar geldi: kimlik eşleşirse bekleyen isteği verir (bir kez); eşleşmezse/çift ise None (yok say).
    pub fn take_consent(&mut self, id: u64) -> Option<PendingConsent> {
        if self.consent.as_ref().map(|c| c.id) == Some(id) { self.consent.take() } else { None }
    }
    /// Stop / telefon koptu: bekleyen isteği iptal et (sonucu yok sayılır). Kapatılacak pencere kimliği.
    pub fn cancel_consent(&mut self) -> Option<u64> {
        self.consent.take().map(|c| c.id)
    }
    pub fn begin_session(&mut self) -> u64 {
        self.session += 1;
        self.session
    }
    /// Başlatma hatası: true ise çağıran `stop` yapmalı (pipeline zaten kayıtlı).
    pub fn report_failure(&mut self, id: u64) -> bool {
        if self.session != id {
            return false;
        }
        self.failed = id;
        self.pipeline.is_some()
    }
    /// Pipeline kaydedilebilir mi (hata gelmedi ve oturum bayatlamadı)?
    pub fn can_commit(&self, id: u64) -> bool {
        self.session == id && self.failed != id
    }
}

fn shared(app: &AppHandle) -> Arc<Shared> {
    app.state::<Arc<Shared>>().inner().clone()
}

fn monitor_infos() -> Vec<MonitorInfo> {
    crate::win::input::invalidate_monitors();
    screen::monitors()
        .into_iter()
        .map(|m| MonitorInfo { index: m.index, name: m.name, width: m.width, height: m.height, primary: m.primary })
        .collect()
}

fn send_info(sh: &Shared, node: &Node, active: bool, error: Option<String>) {
    let cur = lk(&sh.screen).monitor;
    node.send_screen_info(monitor_infos(), cur, active, error);
}

/// Bağlanınca telefona monitör listesini bildir (telefon seçici için).
pub fn on_connected(app: &AppHandle) {
    let sh = shared(app);
    let Some(node) = sh.node().cloned() else { return };
    if node.peer_supports_screen() && lk(&sh.settings).screen_share {
        send_info(&sh, &node, false, None);
    }
}

pub fn on_disconnected(app: &AppHandle) {
    cancel_consent(app);
    stop(app, "bağlantı koptu");
}

pub fn on_cmd(app: &AppHandle, cmd: ScreenCmd) {
    let sh = shared(app);
    let Some(node) = sh.node().cloned() else { return };
    match cmd {
        ScreenCmd::Start { monitor, max_w, max_h, fps, bitrate_kbps } => {
            let p = Params {
                monitor,
                max_w: max_w.clamp(320, 3840),
                max_h: max_h.clamp(240, 2160),
                fps: fps.clamp(5, 60),
                bitrate_kbps: bitrate_kbps.clamp(300, 20_000),
            };
            if !lk(&sh.settings).screen_share {
                send_info(&sh, &node, false, Some("Bu bilgisayarda uzaktan ekran görüntüleme kapalı".into()));
                return;
            }
            let Some(peer) = node.current_peer() else { return };
            if lk(&sh.settings).screen_allowed.contains(&peer.id) {
                start_pipeline(app, p);
                return;
            }
            // İlk kez: PC kullanıcısına temalı onay penceresiyle sor.
            let Some(id) = lk(&sh.screen).begin_consent(peer.id.clone(), peer.name.clone(), p) else { return };
            open_consent(app, id);
        }
        ScreenCmd::Stop => {
            cancel_consent(app);
            stop(app, "telefon durdurdu")
        }
        ScreenCmd::Keyframe => {
            if let Some(p) = lk(&sh.screen).pipeline.as_ref() {
                p.request_key();
            }
        }
        ScreenCmd::Config { max_w, max_h, fps, bitrate_kbps } => {
            if let Some(p) = lk(&sh.screen).pipeline.as_ref() {
                let cur = p.current();
                p.reconfigure(Params {
                    monitor: cur.monitor,
                    max_w: max_w.clamp(320, 3840),
                    max_h: max_h.clamp(240, 2160),
                    fps: fps.clamp(5, 60),
                    bitrate_kbps: bitrate_kbps.clamp(300, 20_000),
                });
            }
        }
        ScreenCmd::Pointer { x, y, monitor } => {
            // Yalnızca paylaşım sürerken (yetkisiz fare kontrolü yok).
            if lk(&sh.screen).sharing {
                crate::win::input::move_abs(monitor, x, y);
            }
        }
    }
}

// ---------------------------------------------------------------- onay penceresi

/// Onay penceresine gösterilen bilgi (`consent_info` komutu).
#[derive(serde::Serialize)]
pub struct ConsentInfo {
    pub id: u64,
    pub name: String,
    pub timeout_secs: u64,
}

pub fn consent_info(app: &AppHandle) -> Option<ConsentInfo> {
    lk(&shared(app).screen)
        .consent
        .as_ref()
        .map(|c| ConsentInfo { id: c.id, name: c.peer_name.clone(), timeout_secs: CONSENT_TIMEOUT.as_secs() })
}

fn close_consent_window(app: &AppHandle) {
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = app2.get_webview_window(CONSENT) {
            let _ = w.close();
        }
    });
}

/// Stop / telefon koptu: bekleyen onayı iptal et, pencereyi kapat; gecikmiş sonuç kimlik uyuşmadığından yok sayılır.
fn cancel_consent(app: &AppHandle) {
    if lk(&shared(app).screen).cancel_consent().is_some() {
        close_consent_window(app);
    }
}

/// Kararı işler (pencere düğmesi, zaman aşımı, pencere kapatma). Kimlik bayatsa hiçbir şey yapmaz.
pub fn resolve_consent(app: &AppHandle, id: u64, choice: Choice) {
    let sh = shared(app);
    let Some(pending) = lk(&sh.screen).take_consent(id) else { return };
    close_consent_window(app);
    let cur = sh.node().and_then(|n| n.current_peer()).map(|c| c.id);
    match outcome(&pending, choice, cur.as_deref()) {
        Outcome::Ignore => log::info!("ekran paylaşımı onayı bayat, yok sayıldı"),
        Outcome::Reject => {
            if let Some(n) = sh.node() {
                send_info(&sh, n, false, Some("Bilgisayar kullanıcısı reddetti".into()));
            }
        }
        Outcome::Start { params, remember } => {
            if let Some(pid) = remember {
                let mut s = lk(&sh.settings);
                if !s.screen_allowed.contains(&pid) {
                    s.screen_allowed.push(pid);
                }
                drop(s);
                sh.save_settings();
            }
            start_pipeline(app, params);
        }
    }
}

fn open_consent(app: &AppHandle, id: u64) {
    // Zaman aşımı: yanıt yoksa Reddet (kimlik eşleşmezse zararsız).
    let app_t = app.clone();
    let _ = std::thread::Builder::new().name("consent-timeout".into()).spawn(move || {
        std::thread::sleep(CONSENT_TIMEOUT);
        resolve_consent(&app_t, id, Choice::Deny);
    });
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(old) = app2.get_webview_window(CONSENT) {
            let _ = old.close();
        }
        let built = WebviewWindowBuilder::new(&app2, CONSENT, WebviewUrl::App("index.html#consent".into()))
            .title("T-Bridge · Ekran paylaşımı isteği")
            .inner_size(464.0, 270.0)
            .center()
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focused(true)
            .build();
        match built {
            Ok(w) => {
                // Pencere başka yolla kapanırsa (Alt+F4 vb.) karar Reddet.
                let app3 = app2.clone();
                w.on_window_event(move |e| {
                    if matches!(e, WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed) {
                        resolve_consent(&app3, id, Choice::Deny);
                    }
                });
            }
            Err(e) => {
                log::warn!("onay penceresi açılamadı: {e}");
                resolve_consent(&app2, id, Choice::Deny);
            }
        }
    });
}

fn start_pipeline(app: &AppHandle, p: Params) {
    let sh = shared(app);
    let Some(node) = sh.node().cloned() else { return };
    // Önceki oturum varsa kapat.
    let old = lk(&sh.screen).pipeline.take();
    if let Some(mut o) = old {
        o.stop();
    }
    node.end_screen_stream();
    let sid = {
        let mut st = lk(&sh.screen);
        st.monitor = p.monitor;
        st.begin_session()
    };

    let (n1, n2, n3, n4) = (node.clone(), node.clone(), node.clone(), node.clone());
    let app3 = app.clone();
    let sink = Sink {
        send: Box::new(move |pts, key, data| n1.send_screen_frame(pts, key, data)),
        feedback: Box::new(move |pts| {
            let f = n2.screen_feedback(pts);
            (f.want_key, f.backlog_ms)
        }),
        on_secure: Box::new(move |secure| {
            // Eski/yeteneksiz eşe gitmez (send_screen_notice yetenek kapısı).
            n4.send_screen_notice(secure);
        }),
        on_status: Box::new(move |r| {
            let sh = shared(&app3);
            match r {
                Ok(desc) => {
                    log::info!("ekran paylaşımı: {desc}");
                    send_info(&sh, &n3, true, None);
                }
                Err(e) => {
                    log::warn!("ekran paylaşımı başlatılamadı: {e}");
                    send_info(&sh, &n3, false, Some(format!("Ekran yakalanamadı: {e}")));
                    // Pipeline henüz kaydedilmediyse start_pipeline `failed` işaretini görüp kaydetmez.
                    if lk(&sh.screen).report_failure(sid) {
                        let app4 = app3.clone();
                        std::thread::spawn(move || stop(&app4, "hata"));
                    }
                }
            }
        }),
    };
    let pipeline = Pipeline::start(p, sink);
    let rejected = {
        let mut st = lk(&sh.screen);
        if st.can_commit(sid) {
            st.pipeline = Some(pipeline);
            st.sharing = true;
            None
        } else {
            Some(pipeline)
        }
    };
    if let Some(mut dead) = rejected {
        // Hızlı hata veya yarışta bayatlama: durumu temiz bırak.
        dead.stop();
        node.end_screen_stream();
        return;
    }
    set_indicator(app, true);
}

pub fn stop(app: &AppHandle, why: &str) {
    let sh = shared(app);
    let (pipeline, was) = {
        let mut st = lk(&sh.screen);
        let was = st.sharing;
        st.sharing = false;
        (st.pipeline.take(), was)
    };
    if !was && pipeline.is_none() {
        return;
    }
    log::info!("ekran paylaşımı durdu: {why}");
    if let Some(mut p) = pipeline {
        p.stop();
    }
    if let Some(n) = sh.node() {
        n.end_screen_stream();
        send_info(&sh, n, false, None);
    }
    set_indicator(app, false);
}

/// Hap konumu (fiziksel piksel) kalıcı dosyası.
fn pill_pos_path(sh: &Shared) -> std::path::PathBuf {
    sh.settings_path.with_file_name("pill-pos.json")
}

fn load_pill_pos(sh: &Shared) -> Option<(i32, i32)> {
    serde_json::from_slice(&std::fs::read(pill_pos_path(sh)).ok()?).ok()
}

fn save_pill_pos(sh: &Shared, pos: (i32, i32)) {
    if let Ok(b) = serde_json::to_vec(&pos) {
        let _ = std::fs::write(pill_pos_path(sh), b);
    }
}

/// Pencere (merkez noktası) monitörlerden birinin içindeyse konum geçerlidir (monitör çıkarıldıysa eski konum atılır).
/// `monitors` = (x, y, genişlik, yükseklik) fiziksel piksel.
pub fn pill_pos_visible(pos: (i32, i32), size: (i32, i32), monitors: &[(i32, i32, i32, i32)]) -> bool {
    let (cx, cy) = (pos.0 + size.0 / 2, pos.1 + size.1 / 2);
    monitors.iter().any(|&(x, y, w, h)| cx >= x && cx < x + w && cy >= y && cy < y + h)
}

/// Tepsi simgesi rengi + üstte açılır "Ekran paylaşılıyor · Durdur" hapı.
fn set_indicator(app: &AppHandle, on: bool) {
    let sh = shared(app);
    sh.update_tray();
    sh.set_tray_tint(on);
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        let sh = shared(&app2);
        if on {
            if app2.get_webview_window(PILL).is_some() {
                return;
            }
            let (w, h) = (300.0, 44.0);
            let x = app2
                .primary_monitor()
                .ok()
                .flatten()
                .map(|m| {
                    let sf = m.scale_factor();
                    m.position().x as f64 / sf + (m.size().width as f64 / sf - w) / 2.0
                })
                .unwrap_or(400.0);
            let built = WebviewWindowBuilder::new(&app2, PILL, WebviewUrl::App("index.html#pill".into()))
                .title("T-Bridge · Ekran paylaşılıyor")
                .inner_size(w, h)
                .position(x, 12.0)
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .resizable(false)
                .maximizable(false)
                .minimizable(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .focused(false)
                .build();
            if let Ok(win) = built {
                // Kayıtlı konum hâlâ bir monitörde görünüyorsa oraya taşı.
                if let Some(p) = load_pill_pos(&sh) {
                    let size = win.outer_size().map(|s| (s.width as i32, s.height as i32)).unwrap_or((300, 44));
                    let mons: Vec<_> = app2
                        .available_monitors()
                        .unwrap_or_default()
                        .iter()
                        .map(|m| (m.position().x, m.position().y, m.size().width as i32, m.size().height as i32))
                        .collect();
                    if pill_pos_visible(p, size, &mons) {
                        let _ = win.set_position(PhysicalPosition::new(p.0, p.1));
                    }
                }
                // Sürüklenince konumu bellekte tut; kapanırken diske yaz (her harekette yazma yok).
                let app3 = app2.clone();
                win.on_window_event(move |e| {
                    if let WindowEvent::Moved(p) = e {
                        lk(&shared(&app3).screen).pill_pos = Some((p.x, p.y));
                    }
                });
            }
        } else if let Some(w) = app2.get_webview_window(PILL) {
            if let Some(p) = lk(&sh.screen).pill_pos {
                save_pill_pos(&sh, p);
            }
            let _ = w.close();
        }
    });
}

/// Hap için paylaşılan monitör etiketi ("Monitör 2").
pub fn pill_label(app: &AppHandle) -> String {
    format!("Monitör {}", lk(&shared(app).screen).monitor + 1)
}

#[cfg(test)]
mod tests {
    use super::{Choice, Outcome, Params, ScreenState, outcome};

    fn params() -> Params {
        Params { monitor: 0, max_w: 1920, max_h: 1080, fps: 30, bitrate_kbps: 5000 }
    }

    #[test]
    fn fast_failure_before_commit_is_not_committed() {
        let mut st = ScreenState::default();
        let id = st.begin_session();
        assert!(!st.report_failure(id)); // pipeline yok -> stop thread gerekmez
        assert!(!st.can_commit(id)); // kaydedilmez, sharing=true olmaz
    }

    #[test]
    fn pill_position_visibility() {
        use super::pill_pos_visible;
        let mons = [(0, 0, 1920, 1080), (1920, 0, 1280, 1024)];
        assert!(pill_pos_visible((800, 12), (300, 44), &mons));
        assert!(pill_pos_visible((2000, 40), (300, 44), &mons)); // ikinci monitör
        assert!(!pill_pos_visible((4000, 40), (300, 44), &mons)); // monitör çıkarılmış
        assert!(!pill_pos_visible((10, 10), (300, 44), &[]));
    }

    #[test]
    fn single_consent_at_a_time() {
        let mut st = ScreenState::default();
        let id = st.begin_consent("p1".into(), "Tel".into(), params()).unwrap();
        assert!(st.begin_consent("p2".into(), "Baska".into(), params()).is_none());
        assert!(st.take_consent(id + 1).is_none()); // bayat/yanlış kimlik yok sayılır
        assert!(st.take_consent(id).is_some());
        assert!(st.take_consent(id).is_none()); // çift sonuç yok
        assert!(st.begin_consent("p2".into(), "Baska".into(), params()).unwrap() > id);
    }

    #[test]
    fn cancel_makes_late_result_stale() {
        let mut st = ScreenState::default();
        let id = st.begin_consent("p1".into(), "Tel".into(), params()).unwrap();
        assert_eq!(st.cancel_consent(), Some(id)); // Stop / telefon koptu
        assert!(st.take_consent(id).is_none()); // geç gelen sonuç yok sayılır
        assert_eq!(st.cancel_consent(), None);
    }

    #[test]
    fn timeout_is_deny_and_choice_parse_is_safe() {
        assert_eq!(Choice::parse("allow"), Choice::Allow);
        assert_eq!(Choice::parse("always"), Choice::Always);
        assert_eq!(Choice::parse(""), Choice::Deny);
        assert_eq!(Choice::parse("???"), Choice::Deny);
        let mut st = ScreenState::default();
        let id = st.begin_consent("p1".into(), "Tel".into(), params()).unwrap();
        let p = st.take_consent(id).unwrap();
        // Zaman aşımı yolu Choice::Deny çözer.
        assert_eq!(outcome(&p, Choice::Deny, Some("p1")), Outcome::Reject);
    }

    #[test]
    fn outcome_rules() {
        let mut st = ScreenState::default();
        let id = st.begin_consent("p1".into(), "Tel".into(), params()).unwrap();
        let p = st.take_consent(id).unwrap();
        assert_eq!(outcome(&p, Choice::Allow, Some("p1")), Outcome::Start { params: params(), remember: None });
        assert_eq!(outcome(&p, Choice::Always, Some("p1")), Outcome::Start { params: params(), remember: Some("p1".into()) });
        // Onay beklerken başka cihaz bağlandı / bağlantı koptu: bayat.
        assert_eq!(outcome(&p, Choice::Allow, Some("p2")), Outcome::Ignore);
        assert_eq!(outcome(&p, Choice::Always, None), Outcome::Ignore);
    }

    #[test]
    fn stale_session_failure_ignored() {
        let mut st = ScreenState::default();
        let old = st.begin_session();
        let new = st.begin_session();
        assert!(!st.report_failure(old));
        assert!(st.can_commit(new));
        assert!(!st.can_commit(old));
    }
}
