//! Masaüstü uygulama durumu, ayarlar ve çekirdek olaylarının işlenmesi.
//! Pencere kapalıyken de durum burada tutulur (pencere yeniden açılınca `snapshot` ile alınır).
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard, OnceLock},
    time::{Duration, Instant},
};

use base64::Engine;
use kopru_core::*;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::win;

pub const MAX_NOTIFS: usize = 200;

pub fn lk<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub clipboard_sync: bool,
    /// Telefon bildirimlerini göster/sakla (kapalıysa özellik tamamen durur).
    pub notif_mirror: bool,
    /// Yeni bildirimde kısa ses.
    pub notif_sound: bool,
    /// Windows'un sağ altında uyarı (varsayılan kapalı).
    pub notif_toast: bool,
    pub transfer_toast: bool,
    pub low_battery_warn: bool,
    pub autostart: bool,
    pub download_dir: String,
    pub muted_apps: Vec<String>,
    /// "dark" | "system" | "light"
    pub theme: String,
    pub auto_update: bool,
    /// Telefonun bu bilgisayarın ekranını görmesine izin ver (kapalıysa özellik tamamen devre dışı).
    pub screen_share: bool,
    /// "Her zaman izin ver" denen telefon kimlikleri (ilk seferde onay sorulur).
    pub screen_allowed: Vec<String>,
    /// Telefon kamerası etkin (sanal kamera uygulama açılışında başlatılır). Yalnızca Windows 11.
    pub camera_enabled: bool,
    /// Ayar göçleri için sürüm. Dosyada yoksa 0 sayılır (eski kurulum); yeni kurulum doğrudan güncel sürümle başlar.
    #[serde(default = "legacy_version")]
    pub settings_version: u32,
}

/// Güncel ayar şeması sürümü (1: Windows bildirimi varsayılanı kapalı).
pub const SETTINGS_VERSION: u32 = 1;

fn legacy_version() -> u32 {
    0
}

impl Settings {
    /// Eski dosyaları bir kez yükseltir. Değişiklik olduysa `true` (kaydedilmeli).
    pub fn migrate(&mut self) -> bool {
        if self.settings_version >= SETTINGS_VERSION {
            return false;
        }
        if self.settings_version < 1 {
            self.notif_toast = false;
        }
        self.settings_version = SETTINGS_VERSION;
        true
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            clipboard_sync: true,
            notif_mirror: true,
            notif_sound: true,
            notif_toast: false,
            transfer_toast: true,
            low_battery_warn: true,
            autostart: true,
            download_dir: String::new(),
            muted_apps: Vec::new(),
            theme: "dark".into(),
            auto_update: true,
            screen_share: true,
            screen_allowed: Vec::new(),
            camera_enabled: false,
            settings_version: SETTINGS_VERSION,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Battery {
    pub level: u8,
    pub charging: bool,
}

pub struct Shared {
    pub app: AppHandle,
    pub settings_path: PathBuf,
    pub settings: Mutex<Settings>,
    pub notifications: Mutex<VecDeque<NotificationData>>,
    pub icons: Mutex<HashMap<String, String>>,
    pub battery: Mutex<Option<Battery>>,
    pub now_playing: Mutex<Option<NowPlaying>>,
    pub low_warned: Mutex<bool>,
    /// Bağlanınca telefon tüm etkin bildirimleri yeniden yollar: bu süre içinde Windows bildirimi gösterilmez.
    pub quiet_until: Mutex<Option<Instant>>,
    /// Güvenlik duvarı kuralı var mı (arayüzde uyarı için).
    pub firewall_ok: std::sync::atomic::AtomicBool,
    pub updater: crate::updater::Updater,
    pub node: OnceLock<Arc<Node>>,
    /// Uzaktan ekran görüntüleme oturumu.
    pub screen: Mutex<crate::screen_share::ScreenState>,
    /// Telefon kamerası merkezi (yalnızca Windows 11'de oluşturulur).
    pub camera: OnceLock<crate::win::camera::Hub>,
}

impl Shared {
    pub fn node(&self) -> Option<&Arc<Node>> {
        self.node.get()
    }

    pub fn save_settings(&self) {
        let s = lk(&self.settings).clone();
        if let Ok(b) = serde_json::to_vec_pretty(&s) {
            let tmp = self.settings_path.with_extension("tmp");
            if std::fs::write(&tmp, b).is_ok() {
                let _ = std::fs::rename(&tmp, &self.settings_path);
            }
        }
    }

    fn window_visible(&self) -> bool {
        self.app
            .get_webview_window(crate::MAIN)
            .map(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false))
            .unwrap_or(false)
    }

    fn toast(&self, title: &str, body: &str) {
        let _ = self.app.notification().builder().title(title).body(body).show();
    }

    pub fn update_tray(&self) {
        let Some(tray) = self.app.tray_by_id("tray") else { return };
        let node = self.node();
        let state = node.map(|n| n.state()).unwrap_or(ConnState::Disconnected);
        let peer = node.and_then(|n| n.current_peer());
        let mut tip = match (state, peer) {
            (ConnState::Connected, Some(p)) => format!("T-Bridge — {} bağlı", p.name),
            (ConnState::Connecting, _) => "T-Bridge — bağlanıyor…".to_string(),
            _ => "T-Bridge — telefon bağlı değil".to_string(),
        };
        if state == ConnState::Connected
            && let Some(b) = *lk(&self.battery) {
                tip.push_str(&format!("\nPil: %{}{}", b.level, if b.charging { " ⚡" } else { "" }));
            }
        let _ = tray.set_tooltip(Some(tip));
    }

    /// Ekran paylaşılırken tepsi simgesine altın nokta ekler; bitince özgün simgeye döner.
    pub fn set_tray_tint(&self, on: bool) {
        let Some(tray) = self.app.tray_by_id("tray") else { return };
        let Some(icon) = self.app.default_window_icon() else { return };
        let (w, h) = (icon.width(), icon.height());
        let mut rgba = icon.rgba().to_vec();
        if on {
            let r = (w.min(h) as f32 * 0.22).max(3.0);
            let (cx, cy) = (w as f32 - r - 1.0, h as f32 - r - 1.0);
            for y in 0..h {
                for x in 0..w {
                    let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                    let d = (dx * dx + dy * dy).sqrt();
                    if d <= r + 1.0 {
                        let i = ((y * w + x) * 4) as usize;
                        let (c, a): ([u8; 3], u8) = if d <= r - 1.0 { ([0xE3, 0xA9, 0x2B], 255) } else if d <= r { ([0x0A, 0x0A, 0x0B], 255) } else { ([0x0A, 0x0A, 0x0B], 160) };
                        rgba[i] = c[0];
                        rgba[i + 1] = c[1];
                        rgba[i + 2] = c[2];
                        rgba[i + 3] = a;
                    }
                }
            }
        }
        let _ = tray.set_icon(Some(tauri::image::Image::new_owned(rgba, w, h)));
    }

    /// Çekirdekten gelen olay. Kumanda olayları doğrudan işlenir (arayüze gitmez).
    pub fn handle(&self, ev: Event) {
        match &ev {
            Event::PointerMove { dx, dy } => {
                win::input::move_rel(*dx, *dy);
                return;
            }
            Event::Pc { cmd } => {
                win::input::exec(cmd);
                return;
            }
            Event::CamFrame { pts_us, key, data } => {
                if let Some(h) = self.camera.get() {
                    h.push_frame((*pts_us, *key, data.clone()));
                }
                return; // video kareleri arayüze gönderilmez
            }
            Event::CamState { active, w, h, fps, facing, mirror, rotation } => {
                if let Some(hub) = self.camera.get() {
                    if let Ok(mut p) = hub.phone.lock() {
                        *p = crate::win::camera::PhoneCam { active: *active, w: *w, h: *h, fps: *fps, facing: *facing, mirror: *mirror, rotation: *rotation };
                    }
                }
            }
            Event::SuiteCmd { req, app, name, json } => {
                crate::suite::on_phone_cmd(&self.app, *req, app.clone(), name.clone(), json.clone());
                return;
            }
            Event::Screen { cmd } => {
                crate::screen_share::on_cmd(&self.app, cmd.clone());
                return;
            }
            Event::ClipboardReceived { text } => {
                if lk(&self.settings).clipboard_sync {
                    win::clipboard::set_text(text);
                }
            }
            Event::State { state, .. } => {
                crate::suite::on_phone_state(*state == ConnState::Connected);
                if *state == ConnState::Connected {
                    crate::screen_share::on_connected(&self.app);
                } else {
                    crate::screen_share::on_disconnected(&self.app);
                }
                if *state == ConnState::Connected {
                    // Telefon güncel listeyi baştan gönderecek: bağlantı yokken kalkanlar burada kalmasın.
                    lk(&self.notifications).clear();
                    *lk(&self.quiet_until) = Some(Instant::now() + Duration::from_secs(5));
                } else {
                    *lk(&self.battery) = None;
                    *lk(&self.now_playing) = None;
                }
                self.update_tray();
            }
            Event::Battery { level, charging } => {
                *lk(&self.battery) = Some(Battery { level: *level, charging: *charging });
                let warn = {
                    let mut w = lk(&self.low_warned);
                    if *charging || *level > 20 {
                        *w = false;
                        false
                    } else if *level <= 15 && !*w {
                        *w = true;
                        true
                    } else {
                        false
                    }
                };
                if warn && lk(&self.settings).low_battery_warn {
                    self.toast("Telefonun şarjı azaldı", &format!("Pil %{}", level));
                }
                self.update_tray();
            }
            Event::Notification { data } => {
                let (muted, toast, sound, mirror) = {
                    let s = lk(&self.settings);
                    (s.muted_apps.contains(&data.package), s.notif_toast, s.notif_sound, s.notif_mirror)
                };
                // Özellik kapalı: saklama, arayüze iletme, ses/toast yok.
                if muted || !mirror {
                    return;
                }
                let same = {
                    let mut n = lk(&self.notifications);
                    let same = n.iter().any(|x| x.key == data.key && x.title == data.title && x.text == data.text);
                    n.retain(|x| x.key != data.key);
                    n.push_front(data.clone());
                    n.truncate(MAX_NOTIFS);
                    same
                };
                let quiet = lk(&self.quiet_until).is_some_and(|t| Instant::now() < t);
                if !same && !quiet {
                    if sound {
                        win::sound::play_throttled();
                    }
                    if toast && !self.window_visible() {
                        let title = if data.title.is_empty() { data.app_name.clone() } else { format!("{} · {}", data.app_name, data.title) };
                        self.toast(&title, &data.text);
                    }
                }
            }
            Event::NotificationRemoved { key } => {
                lk(&self.notifications).retain(|x| &x.key != key);
            }
            Event::AppIcon { package, png } => {
                let url = format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png));
                lk(&self.icons).insert(package.clone(), url.clone());
                let _ = self.app.emit("kopru", serde_json::json!({ "type": "appIconUrl", "package": package, "url": url }));
                return;
            }
            Event::NowPlaying { info } => {
                *lk(&self.now_playing) = info.clone();
            }
            Event::Transfer { info }
                if info.direction == Direction::Incoming
                    && info.state == TransferState::Done
                    && {
                        crate::suite::on_incoming_done(&info.name, info.detail.as_deref(), &crate::download_dir(&self.app, &lk(&self.settings)));
                        true
                    }
                    && lk(&self.settings).transfer_toast
                    && !self.window_visible()
                => {
                    let name = info.name.rsplit('/').next().unwrap_or(&info.name);
                    self.toast("Dosya alındı", name);
                }
            _ => {}
        }
        let _ = self.app.emit("kopru", &ev);
    }
}

/// Çekirdeğin Host arayüzü (masaüstü): dosyalar doğrudan diskte, fd gerekmez.
pub struct DesktopHost(pub Arc<Shared>);

impl Host for DesktopHost {
    fn on_event(&self, event: Event) {
        self.0.handle(event);
    }
    fn open_incoming(&self, _: String, _: String, _: u64) -> Result<OpenedFd, KopruError> {
        Err(KopruError::new("masaüstünde kullanılmaz"))
    }
    fn complete_incoming(&self, _: String, _: bool) -> Option<String> {
        None
    }
    fn open_outgoing(&self, _: String) -> Result<i32, KopruError> {
        Err(KopruError::new("masaüstünde kullanılmaz"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eski_dosya_bir_kez_goc_eder() {
        // Eski sürüm: notifToast true, sürüm alanı yok.
        let mut s: Settings = serde_json::from_str(r#"{"notifToast":true,"clipboardSync":false}"#).unwrap();
        assert_eq!(s.settings_version, 0);
        assert!(s.notif_toast);
        assert!(s.migrate());
        assert!(!s.notif_toast);
        assert!(!s.clipboard_sync);
        assert!(s.notif_sound && s.notif_mirror);
        assert_eq!(s.settings_version, SETTINGS_VERSION);
        assert!(!s.migrate());
    }

    #[test]
    fn kullanicinin_tercihi_korunur() {
        // Göçten sonra kullanıcı toast'ı açıp kaydeder; yeniden yüklenince dokunulmaz.
        let mut s = Settings::default();
        assert!(!s.notif_toast);
        s.notif_toast = true;
        let json = serde_json::to_string(&s).unwrap();
        let mut back: Settings = serde_json::from_str(&json).unwrap();
        assert!(!back.migrate());
        assert!(back.notif_toast);
    }

    #[test]
    fn bos_dosya_eski_sayilir_varsayilan_yeni() {
        let mut s: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(s.settings_version, 0);
        assert!(s.migrate());
        assert!(Settings::default().settings_version == SETTINGS_VERSION && !Settings::default().notif_toast);
    }
}
