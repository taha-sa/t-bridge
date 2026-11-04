//! Masaüstü uygulama durumu, ayarlar ve çekirdek olaylarının işlenmesi.
//! Pencere kapalıyken de durum burada tutulur (pencere yeniden açılınca `snapshot` ile alınır).
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard, OnceLock},
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
    pub notif_toast: bool,
    pub transfer_toast: bool,
    pub low_battery_warn: bool,
    pub autostart: bool,
    pub download_dir: String,
    pub muted_apps: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            clipboard_sync: true,
            notif_toast: true,
            transfer_toast: true,
            low_battery_warn: true,
            autostart: true,
            download_dir: String::new(),
            muted_apps: Vec::new(),
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
    pub node: OnceLock<Arc<Node>>,
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
            (ConnState::Connected, Some(p)) => format!("Köprü — {} bağlı", p.name),
            (ConnState::Connecting, _) => "Köprü — bağlanıyor…".to_string(),
            _ => "Köprü — telefon bağlı değil".to_string(),
        };
        if state == ConnState::Connected
            && let Some(b) = *lk(&self.battery) {
                tip.push_str(&format!("\nPil: %{}{}", b.level, if b.charging { " ⚡" } else { "" }));
            }
        let _ = tray.set_tooltip(Some(tip));
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
            Event::ClipboardReceived { text } => {
                if lk(&self.settings).clipboard_sync {
                    win::clipboard::set_text(text);
                }
            }
            Event::State { state, .. } => {
                if *state != ConnState::Connected {
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
                let (muted, toast) = {
                    let s = lk(&self.settings);
                    (s.muted_apps.contains(&data.package), s.notif_toast)
                };
                if muted {
                    return;
                }
                {
                    let mut n = lk(&self.notifications);
                    n.retain(|x| x.key != data.key);
                    n.push_front(data.clone());
                    n.truncate(MAX_NOTIFS);
                }
                if toast && !self.window_visible() {
                    let title = if data.title.is_empty() { data.app_name.clone() } else { format!("{} · {}", data.app_name, data.title) };
                    self.toast(&title, &data.text);
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
