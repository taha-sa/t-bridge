mod app;
mod commands;
mod win;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};

use kopru_core::{Node, NodeConfig, Platform, Role};
use tauri::{
    AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder,
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
        .join("Köprü")
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
        .title("Köprü")
        .inner_size(960.0, 660.0)
        .min_inner_size(720.0, 520.0)
        .center()
        .build()
    {
        log::error!("pencere açılamadı: {e}");
    }
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Köprü'yü aç", true, None::<&str>)?;
    let ring = MenuItem::with_id(app, "ring", "Telefonu çaldır", true, None::<&str>)?;
    let send = MenuItem::with_id(app, "send", "Dosya gönder…", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Çıkış", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &send, &ring, &sep, &quit])?;
    TrayIconBuilder::with_id("tray")
        .icon(app.default_window_icon().cloned().expect("uygulama ikonu yok"))
        .tooltip("Köprü")
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

fn start_node(shared: &Arc<Shared>, data_dir: PathBuf) {
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
    let hidden = std::env::args().any(|a| a == "--hidden");
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            commands::snapshot,
            commands::pairing,
            commands::unpair,
            commands::send_paths,
            commands::pick_and_send,
            commands::pause_transfer,
            commands::resume_transfer,
            commands::cancel_transfer,
            commands::clear_finished,
            commands::reveal,
            commands::open_downloads,
            commands::dismiss_notification,
            commands::clear_notifications,
            commands::mute_app,
            commands::phone_cmd,
            commands::set_settings,
            commands::pick_download_dir,
            commands::restart_app,
        ])
        .setup(move |app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let settings_path = data_dir.join("settings.json");
            let first_run = !settings_path.exists();
            let settings: Settings = std::fs::read(&settings_path)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default();
            let shared = Arc::new(Shared {
                app: app.handle().clone(),
                settings_path,
                settings: Mutex::new(settings),
                notifications: Default::default(),
                icons: Default::default(),
                battery: Default::default(),
                now_playing: Default::default(),
                low_warned: Default::default(),
                node: OnceLock::new(),
            });
            if first_run {
                shared.save_settings();
                win::system::set_autostart(true);
            }
            app.manage(shared.clone());
            setup_tray(app)?;
            start_node(&shared, data_dir);
            if !hidden {
                show_main(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Tauri başlatılamadı");

    app.run(|_app, event| {
        // Son pencere kapanınca uygulama tepside kalır; yalnızca "Çıkış" kapatır.
        if let RunEvent::ExitRequested { code: None, api, .. } = event {
            api.prevent_exit();
        }
    });
}
