mod app;
mod commands;
mod diag;
mod i18n;
mod screen_share;
mod suite;
mod updater;
mod win;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};

use kopru_core::{Node, NodeConfig, Platform, Role};
use tauri::{
    AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use app::{DesktopHost, Settings, Shared, lk};

pub const MAIN: &str = "main";
pub const PORT: u16 = 47800;
pub static START_ERROR: OnceLock<String> = OnceLock::new();

pub fn device_name() -> String {
    std::env::var("COMPUTERNAME").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "Bilgisayar".into())
}

pub fn download_dir(app: &AppHandle, s: &Settings) -> PathBuf {
    if !s.download_dir.is_empty() {
        return PathBuf::from(&s.download_dir);
    }
    app.path()
        .download_dir()
        .unwrap_or_else(|_| PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default()).join("Downloads"))
        .join("T-Bridge")
}

/// Pencere yoksa oluşturur, varsa öne getirir. Kapatılınca WebView tamamen yok edilir.
pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    if let Err(e) = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::default())
        .title("T-Bridge")
        .inner_size(1040.0, 700.0)
        .min_inner_size(820.0, 560.0)
        // Kendi başlık çubuğumuz (arayüzde); Windows 11 gölge ve yuvarlak köşe korunur.
        .decorations(false)
        .shadow(true)
        .background_color(tauri::window::Color(10, 10, 11, 255))
        .center()
        .build()
    {
        log::error!("pencere açılamadı: {e}");
    }
}

fn build_tray_menu<M: Manager<tauri::Wry>>(app: &M) -> tauri::Result<Menu<tauri::Wry>> {
    let open = MenuItem::with_id(app, "open", i18n::t("tray.open"), true, None::<&str>)?;
    let ring = MenuItem::with_id(app, "ring", i18n::t("tray.ring"), true, None::<&str>)?;
    let send = MenuItem::with_id(app, "send", i18n::t("tray.send"), true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", i18n::t("tray.quit"), true, None::<&str>)?;
    Menu::with_items(app, &[&open, &send, &ring, &sep, &quit])
}

/// Dil değişince tepsi menüsünü yeni dille yeniden kurar.
pub fn refresh_tray_menu(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id("tray"), build_tray_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let menu = build_tray_menu(app)?;
    TrayIconBuilder::with_id("tray")
        .icon(app.default_window_icon().cloned().expect("uygulama ikonu yok"))
        .tooltip("T-Bridge")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "open" => show_main(app),
            "ring" => {
                if let Some(n) = app.state::<Arc<Shared>>().node() {
                    n.send_phone(kopru_core::PhoneCommand::Ring { on: true });
                }
            }
            "send" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let s = app.state::<Arc<Shared>>();
                    let _ = commands::pick_and_send(s, false).await;
                });
            }
            "quit" => {
                if let Some(n) = app.state::<Arc<Shared>>().node() {
                    n.shutdown();
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Faz 4c: Windows 11 kapısı + kamera merkezi. `camera` yeteneği YALNIZCA kapı açıksa duyurulur.
struct DesktopPhoneLink(Arc<Shared>);
impl win::camera::PhoneLink for DesktopPhoneLink {
    fn request_keyframe(&self) {
        if let Some(n) = self.0.node() {
            n.send_cam_keyframe();
        }
    }
    fn request_format(&self, w: u32, h: u32) {
        if let Some(n) = self.0.node() {
            let kbps = if h >= 1000 { 6000 } else { 3000 };
            n.send_cam_start(w, h, 30, kbps);
        }
    }
    fn request_stop(&self) {
        if let Some(n) = self.0.node() {
            n.send_cam_stop();
        }
    }
}

fn init_camera(shared: &Arc<Shared>) {
    let allowed = win::camera::camera_allowed_now();
    kopru_core::camera::set_camera_cap(allowed);
    if !allowed {
        return;
    }
    let hub = win::camera::Hub::start(Arc::new(DesktopPhoneLink(shared.clone())));
    let _ = shared.camera.set(hub);
    if lk(&shared.settings).camera_enabled && win::camera::is_installed() {
        let sh = shared.clone();
        std::thread::spawn(move || {
            if let Some(h) = sh.camera.get()
                && let Err(e) = h.enable()
            {
                log::warn!("sanal kamera başlatılamadı: {e}");
            }
        });
    }
}

fn start_node(shared: &Arc<Shared>, data_dir: PathBuf) {
    init_camera(shared);
    let dl = download_dir(&shared.app, &lk(&shared.settings));
    let _ = std::fs::create_dir_all(&dl);
    let cfg = NodeConfig {
        data_dir: data_dir.join("core").to_string_lossy().into(),
        device_name: device_name(),
        platform: Platform::Desktop,
        role: Role::Server,
        port: PORT,
        download_dir: Some(dl.to_string_lossy().into()),
    };
    match Node::new(cfg, Arc::new(DesktopHost(shared.clone()))) {
        Ok(n) => {
            let _ = shared.node.set(n.clone());
            suite::start(&shared.app);
            // Yerel pano değişince telefona gönder.
            let sh = shared.clone();
            win::clipboard::spawn_listener(move || {
                if !lk(&sh.settings).clipboard_sync {
                    return;
                }
                if let Some(text) = win::clipboard::get_text() {
                    n.send_clipboard(text);
                }
            });
        }
        Err(e) => {
            log::error!("çekirdek başlatılamadı: {e}");
            let _ = START_ERROR.set(e.to_string());
        }
    }
    shared.update_tray();
}

pub fn run() {
    // Yükseltilmiş yardımcı modları (UAC ile yeniden başlatılan kamera kayıt/silme adımı): arayüz açmadan çık.
    if let Some(code) = win::camera::maybe_run_helper() {
        std::process::exit(code);
    }
    let hidden = std::env::args().any(|a| a == "--hidden");
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::snapshot,
            diag::crash_pending,
            diag::crash_dismiss,
            diag::report_manual,
            commands::pairing,
            commands::unpair,
            commands::revoke_screen_allowed,
            commands::send_paths,
            commands::pick_and_send,
            commands::pause_transfer,
            commands::resume_transfer,
            commands::cancel_transfer,
            commands::clear_finished,
            commands::reveal,
            commands::open_downloads,
            commands::dismiss_notification,
            commands::stop_screen_share,
            commands::pill_info,
            commands::consent_info,
            commands::consent_result,
            commands::suite_status,
            commands::suite_open,
            commands::suite_update,
            commands::suite_check,
            commands::camera_status,
            commands::camera_enable,
            commands::camera_disable,
            commands::camera_uninstall,
            commands::reply_notification,
            commands::clear_notifications,
            commands::mute_app,
            commands::phone_cmd,
            commands::set_settings,
            commands::ui_lang,
            commands::test_notification_sound,
            commands::pick_download_dir,
            commands::restart_app,
            commands::fix_firewall,
            commands::open_url,
            commands::check_update,
            commands::install_update,
        ])
        .setup(move |app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let settings_path = data_dir.join("settings.json");
            let first_run = !settings_path.exists();
            let mut settings: Settings = std::fs::read(&settings_path)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default();
            // Eski sürümden kalan ayarları bir kez yükselt (ör. Windows bildirimi varsayılanı kapalı).
            let migrated = settings.migrate();
            i18n::apply(&settings.language);
            diag::init(&app.package_info().version.to_string(), settings.auto_crash_report);
            let shared = Arc::new(Shared {
                app: app.handle().clone(),
                settings_path,
                settings: Mutex::new(settings),
                notifications: Default::default(),
                icons: Default::default(),
                battery: Default::default(),
                now_playing: Default::default(),
                low_warned: Default::default(),
                quiet_until: Default::default(),
                firewall_ok: std::sync::atomic::AtomicBool::new(true),
                updater: updater::Updater::new(),
                node: OnceLock::new(),
                screen: Default::default(),
                camera: OnceLock::new(),
            });
            if first_run || migrated {
                shared.save_settings();
            }
            // Her açılışta kaydı güncel exe yoluna yaz (güncelleme/taşıma sonrası eski yol kalmasın).
            win::system::remove_legacy_autostart();
            if lk(&shared.settings).autostart {
                win::system::set_autostart(true);
            }
            app.manage(shared.clone());
            setup_tray(app)?;
            start_node(&shared, data_dir);
            updater::spawn(shared.clone());
            // Güvenlik duvarı: kural yoksa bir kez onay iste (arka planda; açılışı bekletmez).
            let sh = shared.clone();
            std::thread::spawn(move || {
                let mut ok = win::firewall::rule_exists();
                if !ok && !win::firewall::is_dev_build() {
                    ok = win::firewall::add_rule_elevated();
                }
                sh.firewall_ok.store(ok, std::sync::atomic::Ordering::Relaxed);
                let _ = sh.app.emit("kopru", serde_json::json!({ "type": "firewall", "ok": ok }));
            });
            if !hidden {
                show_main(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Tauri başlatılamadı");

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            if let Some(h) = app.state::<Arc<Shared>>().camera.get() {
                h.disable(); // sanal kamera T-Bridge kapanınca kalkar
            }
            suite::shutdown();
        }
        // Son pencere kapanınca uygulama tepside kalır; yalnızca "Çıkış" kapatır.
        if let RunEvent::ExitRequested { code: None, api, .. } = event {
            api.prevent_exit();
        }
    });
}
