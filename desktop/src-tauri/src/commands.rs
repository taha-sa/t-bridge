//! Arayüzün çağırdığı Tauri komutları.
use std::sync::Arc;

use kopru_core::*;
use serde::Serialize;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

use crate::{
    app::{Battery, Settings, Shared, lk},
    win,
};

type S<'a> = State<'a, Arc<Shared>>;

fn node<'a>(s: &'a S<'_>) -> Result<&'a Arc<Node>, String> {
    s.node().ok_or_else(|| crate::i18n::t("err.core"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    device_name: String,
    fingerprint: String,
    port: u16,
    state: ConnState,
    peer: Option<PeerInfo>,
    peers: Vec<PeerInfo>,
    transfers: Vec<TransferInfo>,
    notifications: Vec<NotificationData>,
    icons: std::collections::HashMap<String, String>,
    battery: Option<Battery>,
    now_playing: Option<NowPlaying>,
    settings: Settings,
    download_dir: String,
    error: Option<String>,
    firewall_ok: bool,
    version: String,
    update: crate::updater::UpdateState,
}

#[tauri::command]
pub fn snapshot(s: S<'_>) -> Snapshot {
    let n = s.node();
    // Kilitleri tek tek al-bırak: yapı ifadesindeki geçici kilitler ifade sonuna kadar yaşar,
    // aynı kilidi ikinci kez istemek ana iş parçacığını kilitler.
    let settings = lk(&s.settings).clone();
    let download_dir = crate::download_dir(&s.app, &settings).to_string_lossy().into();
    let notifications = lk(&s.notifications).iter().cloned().collect();
    let icons = lk(&s.icons).clone();
    let battery = *lk(&s.battery);
    let now_playing = lk(&s.now_playing).clone();
    Snapshot {
        device_name: crate::device_name(),
        fingerprint: n.map(|n| n.fingerprint()).unwrap_or_default(),
        port: n.map(|n| n.port()).unwrap_or(0),
        state: n.map(|n| n.state()).unwrap_or(ConnState::Disconnected),
        peer: n.and_then(|n| n.current_peer()),
        peers: n.map(|n| n.peers()).unwrap_or_default(),
        transfers: n.map(|n| n.transfers()).unwrap_or_default(),
        notifications,
        icons,
        battery,
        now_playing,
        settings,
        download_dir,
        error: crate::START_ERROR.get().cloned(),
        firewall_ok: s.firewall_ok.load(std::sync::atomic::Ordering::Relaxed),
        version: s.app.package_info().version.to_string(),
        update: crate::updater::state(&s.app),
    }
}

#[derive(Serialize)]
pub struct Pairing {
    code: String,
    svg: String,
    addrs: Vec<String>,
    name: String,
}

#[tauri::command]
pub fn pairing(s: S<'_>) -> Result<Pairing, String> {
    let info = node(&s)?.pairing_info();
    let svg = qrcode::QrCode::with_error_correction_level(info.uri.as_bytes(), qrcode::EcLevel::M)
        .map_err(|e| e.to_string())?
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(240, 240)
        .quiet_zone(true)
        .dark_color(qrcode::render::svg::Color("#000000"))
        .light_color(qrcode::render::svg::Color("#ffffff"))
        .build();
    Ok(Pairing { code: info.code, svg, addrs: info.addrs, name: info.name })
}

#[tauri::command]
pub fn unpair(s: S<'_>, id: String) -> Result<(), String> {
    node(&s)?.unpair(id.clone());
    // Eşleşme kalkınca "her zaman izin ver" de kalkar.
    revoke_screen(&s, &id);
    Ok(())
}

fn revoke_screen(s: &Shared, id: &str) {
    let changed = {
        let mut st = lk(&s.settings);
        let n = st.screen_allowed.len();
        st.screen_allowed.retain(|x| x != id);
        st.screen_allowed.len() != n
    };
    if changed {
        s.save_settings();
    }
}

/// Ayarlardan: bir cihazın "Her zaman izin ver" iznini geri al.
#[tauri::command]
pub fn revoke_screen_allowed(s: S<'_>, id: String) -> Result<(), String> {
    revoke_screen(&s, &id);
    Ok(())
}

#[tauri::command]
pub fn send_paths(s: S<'_>, paths: Vec<String>) -> Result<usize, String> {
    node(&s)?.send_paths(paths).map(|v| v.len()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pick_and_send(s: S<'_>, folder: bool) -> Result<usize, String> {
    let d = s.app.dialog().file().set_title(if folder { crate::i18n::t("dlg.sendfolder") } else { crate::i18n::t("dlg.sendfiles") });
    let paths: Vec<String> = if folder {
        d.blocking_pick_folder().into_iter().filter_map(|p| p.into_path().ok()).map(|p| p.to_string_lossy().into()).collect()
    } else {
        d.blocking_pick_files()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|p| p.into_path().ok())
            .map(|p| p.to_string_lossy().into())
            .collect()
    };
    if paths.is_empty() {
        return Ok(0);
    }
    node(&s)?.send_paths(paths).map(|v| v.len()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pause_transfer(s: S<'_>, id: u64, direction: Direction) -> Result<(), String> {
    node(&s)?.pause_transfer(id, direction).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resume_transfer(s: S<'_>, id: u64, direction: Direction) -> Result<(), String> {
    node(&s)?.resume_transfer(id, direction).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cancel_transfer(s: S<'_>, id: u64, direction: Direction) -> Result<(), String> {
    node(&s)?.cancel_transfer(id, direction);
    Ok(())
}

#[tauri::command]
pub fn clear_finished(s: S<'_>) -> Result<(), String> {
    node(&s)?.clear_finished();
    Ok(())
}

#[tauri::command]
pub fn reveal(path: String) {
    win::system::reveal(&path);
}

#[tauri::command]
pub fn open_downloads(s: S<'_>) {
    let dir = crate::download_dir(&s.app, &lk(&s.settings));
    let _ = std::fs::create_dir_all(&dir);
    win::system::reveal(&dir.to_string_lossy());
}

#[tauri::command]
pub fn dismiss_notification(s: S<'_>, key: String) -> Result<(), String> {
    lk(&s.notifications).retain(|n| n.key != key);
    node(&s)?.send_phone(PhoneCommand::DismissNotification { key });
    Ok(())
}

/// Telefondaki bildirime yanıt gönderir. Sonuç `notificationReplyResult` olayıyla asenkron gelir.
/// Telefondaki T-Bridge eski sürümse (yetenek duyurmaz) hata döner; hiçbir şey gönderilmez.
#[tauri::command]
pub fn reply_notification(s: S<'_>, key: String, text: String) -> Result<(), String> {
    let text = text.trim().to_string();
    if text.is_empty() || text.len() > 4000 {
        return Err(crate::i18n::t("err.reply.len"));
    }
    let n = node(&s)?;
    if !n.peer_supports_notif_reply() {
        return Err(crate::i18n::t("err.reply.old"));
    }
    if n.send_notification_reply(key, text) {
        Ok(())
    } else {
        Err(crate::i18n::t("err.reply.fail"))
    }
}

/// Açılır hapın "Durdur" düğmesi / Ayarlar: paylaşımı hemen kes.
#[tauri::command]
pub fn stop_screen_share(s: S<'_>) {
    crate::screen_share::stop(&s.app, "PC kullanıcısı durdurdu");
}

/// Hap: paylaşılan monitörün etiketi.
#[tauri::command]
pub fn pill_info(s: S<'_>) -> String {
    crate::screen_share::pill_label(&s.app)
}

/// Onay penceresi: bekleyen isteğin kimliği ve telefon adı (yoksa None -> pencere kendini kapatır).
#[tauri::command]
pub fn consent_info(s: S<'_>) -> Option<crate::screen_share::ConsentInfo> {
    crate::screen_share::consent_info(&s.app)
}

/// Onay penceresi kararı: `choice` = "allow" | "always" | "deny" (diğer her şey Reddet). Bayat kimlik yok sayılır.
#[tauri::command]
pub fn consent_result(s: S<'_>, id: u64, choice: String) {
    crate::screen_share::resolve_consent(&s.app, id, crate::screen_share::Choice::parse(&choice));
}

/// T-Suite uygulama kartları (katalog + kayıtlı uygulamalar).
#[tauri::command]
pub fn suite_status() -> Vec<crate::suite::SuiteCard> {
    crate::suite::hub().map(|h| h.cards()).unwrap_or_default()
}

/// Uygulamayı aç (kapalıysa başlat; çalışıyorsa öne getir).
#[tauri::command]
pub async fn suite_open(id: String) -> Result<(), String> {
    let h = crate::suite::hub().ok_or_else(|| crate::i18n::t("err.suite"))?.clone();
    let name = if h.cards().iter().any(|c| c.id == id && c.running) { "app.show" } else { "app.launch" };
    h.run(&id, name).await
}

/// Güncelle: `id` verilirse tek uygulama, verilmezse güncellemesi olan hepsi. Dönüş: başlatılan sayısı.
#[tauri::command]
pub async fn suite_update(id: Option<String>) -> Result<usize, String> {
    let h = crate::suite::hub().ok_or_else(|| crate::i18n::t("err.suite"))?.clone();
    Ok(h.update(id).await)
}

/// Çalışan uygulamalardan güncelleme sor.
#[tauri::command]
pub async fn suite_check() -> Result<(), String> {
    let h = crate::suite::hub().ok_or_else(|| crate::i18n::t("err.suite"))?.clone();
    h.check_updates().await;
    Ok(())
}

/// Telefon kamerası durumu (arayüz 2 sn'de bir yoklar).
#[tauri::command]
pub fn camera_status(s: S<'_>) -> win::camera::CameraStatus {
    let supported = win::camera::camera_allowed_now();
    let hub = s.camera.get();
    let phone = hub.and_then(|h| h.phone.lock().ok().map(|p| *p)).unwrap_or_default();
    win::camera::CameraStatus {
        supported,
        installed: supported && win::camera::is_installed_for_me(),
        update_needed: supported && win::camera::update_needed(),
        running: hub.is_some_and(|h| h.running.load(std::sync::atomic::Ordering::SeqCst)),
        phone_supports: s.node().is_some_and(|n| n.peer_supports_camera()),
        phone,
        in_use: hub.is_some_and(|h| h.in_use.load(std::sync::atomic::Ordering::SeqCst)),
        decoded_frames: hub.map_or(0, |h| h.decoded.load(std::sync::atomic::Ordering::Relaxed)),
    }
}

/// Kamerayı etkinleştir: gerekirse bir kerelik UAC ile DLL'i kaydeder, sonra sanal kamerayı başlatır.
#[tauri::command]
pub async fn camera_enable(s: S<'_>) -> Result<(), String> {
    if !win::camera::camera_allowed_now() {
        return Err(crate::i18n::t("err.win11"));
    }
    let sh = s.inner().clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        if !win::camera::is_installed_for_me() {
            let sid = win::camera::current_user_sid().ok_or_else(|| crate::i18n::t("err.cam.sid"))?;
            let code = match win::camera::run_elevated(&format!("--camera-register \"{sid}\"")) {
                Ok(c) => c,
                Err(e) if e == win::camera::UAC_CANCELLED => {
                    return Err(crate::i18n::t("err.cam.uac"));
                }
                Err(e) => return Err(e),
            };
            if code != 0 || !win::camera::is_installed_for_me() {
                return Err(crate::i18n::t("err.cam.reg"));
            }
        }
        sh.camera.get().ok_or_else(|| crate::i18n::t("err.cam.hub"))?.enable()?;
        lk(&sh.settings).camera_enabled = true;
        sh.save_settings();
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Sanal kamerayı durdur (kayıt kalır; bir sonraki "Etkinleştir" izin istemez).
#[tauri::command]
pub async fn camera_disable(s: S<'_>) -> Result<(), String> {
    let sh = s.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(h) = sh.camera.get() {
            h.disable();
        }
        lk(&sh.settings).camera_enabled = false;
        sh.save_settings();
    })
    .await
    .map_err(|e| e.to_string())
}

/// Kamerayı kaldır: sanal kamerayı durdurur, DLL kaydını ve dosyasını siler (UAC ister).
#[tauri::command]
pub async fn camera_uninstall(s: S<'_>) -> Result<(), String> {
    let sh = s.inner().clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        if let Some(h) = sh.camera.get() {
            h.disable();
        }
        lk(&sh.settings).camera_enabled = false;
        sh.save_settings();
        if win::camera::is_installed() {
            let code = win::camera::run_elevated("--camera-unregister")?;
            if code != 0 {
                return Err(crate::i18n::t("err.cam.unreg"));
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn clear_notifications(s: S<'_>) -> Result<(), String> {
    let keys: Vec<String> = lk(&s.notifications).drain(..).map(|n| n.key).collect();
    let n = node(&s)?;
    for key in keys {
        n.send_phone(PhoneCommand::DismissNotification { key });
    }
    Ok(())
}

#[tauri::command]
pub fn mute_app(s: S<'_>, package: String, muted: bool) {
    {
        let mut st = lk(&s.settings);
        st.muted_apps.retain(|p| p != &package);
        if muted {
            st.muted_apps.push(package.clone());
        }
    }
    if muted {
        lk(&s.notifications).retain(|n| n.package != package);
    }
    s.save_settings();
}

#[tauri::command]
pub fn phone_cmd(s: S<'_>, cmd: PhoneCommand) -> Result<bool, String> {
    Ok(node(&s)?.send_phone(cmd))
}

#[tauri::command]
pub fn set_settings(s: S<'_>, mut settings: Settings) -> Result<bool, String> {
    let restart_needed = {
        let old = lk(&s.settings);
        // Sürüm alanı yalnızca arka uçta yönetilir (arayüz eski değer yollasa da göç tekrarlanmasın).
        settings.settings_version = old.settings_version;
        // "Her zaman izin ver" listesi yalnızca onay penceresiyle değişir (arayüzdeki eski kopya ezmesin).
        settings.screen_allowed = old.screen_allowed.clone();
        settings.camera_enabled = old.camera_enabled; // yalnızca kamera komutlarıyla değişir
        old.download_dir != settings.download_dir
    };
    if !settings.notif_mirror {
        lk(&s.notifications).clear();
    }
    if settings.autostart != win::system::autostart_enabled() {
        win::system::set_autostart(settings.autostart);
    }
    crate::i18n::apply(&settings.language);
    crate::diag::set_auto(settings.auto_crash_report);
    *lk(&s.settings) = settings;
    s.save_settings();
    crate::refresh_tray_menu(&s.app);
    s.update_tray();
    Ok(restart_needed)
}

/// Ayarlar'daki "Dene" düğmesi: bildirim sesini bir kez çalar.
#[tauri::command]
pub fn test_notification_sound() {
    win::sound::play();
}

#[tauri::command]
pub async fn pick_download_dir(s: S<'_>) -> Result<Option<String>, String> {
    Ok(s.app
        .dialog()
        .file()
        .set_title(crate::i18n::t("dlg.downloaddir"))
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into()))
}

#[tauri::command]
pub fn restart_app(s: S<'_>) {
    s.app.restart();
}

/// Güvenlik duvarı iznini yeniden iste (kullanıcı ilk seferde reddettiyse).
#[tauri::command]
pub async fn fix_firewall(s: S<'_>) -> Result<bool, String> {
    let ok = tauri::async_runtime::spawn_blocking(win::firewall::add_rule_elevated).await.map_err(|e| e.to_string())?;
    s.firewall_ok.store(ok, std::sync::atomic::Ordering::Relaxed);
    Ok(ok)
}

/// Bağlantıyı varsayılan tarayıcıda aç (yalnızca https).
#[tauri::command]
pub fn open_url(url: String) {
    if url.starts_with("https://") {
        win::system::reveal_url(&url);
    }
}

/// Şimdi denetle (Ayarlar düğmesi). Bulunursa indirir; kurulum için `install_update` beklenir.
#[tauri::command]
pub async fn check_update(s: S<'_>) -> Result<bool, String> {
    crate::updater::check(s.inner().clone(), false).await
}

/// İndirilmiş güncellemeyi hemen kur (uygulama yeniden açılır).
#[tauri::command]
pub fn install_update(s: S<'_>) -> Result<(), String> {
    crate::updater::install_now(&s)
}

#[tauri::command]
pub fn ui_lang(s: S<'_>) -> String {
    crate::i18n::resolve(&lk(&s.settings).language).to_string()
}
