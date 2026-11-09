//! T-Bridge çekirdeği: aynı Wi-Fi üzerinde PC ↔ telefon bağlantısı.
//!
//! - QUIC (quinn) + karşılıklı TLS, sertifika parmak izi pinleme
//! - mDNS ile bulma, QR/6 haneli kod ile eşleştirme
//! - metin pano senkronu, kaldığı yerden devam eden dosya aktarımı + kalıcı kuyruk
//! - bildirim, pil, medya ve uzaktan kumanda mesajları
uniffi::setup_scaffolding!();

pub mod discovery;
pub mod files;
pub mod host;
pub mod keys;
pub mod node;
pub mod camera;
pub mod proto;
pub mod screen;
pub mod store;
pub mod tls;
pub mod types;

use std::sync::{Arc, Mutex};

pub use host::{Host, KopruError};
pub use types::*;

use node::{Inner, lk};
use proto::Msg;

/// Android log çıktısını logcat'e yönlendirir (bir kez çağrılır).
#[uniffi::export]
pub fn init_logging() {
    #[cfg(target_os = "android")]
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("kopru"),
    );
}

/// Bu cihaz `camera` yeteneğini duyursun mu (Android: true; masaüstü yalnızca Windows 11'de). Node oluşturulmadan ÖNCE çağrılır.
#[uniffi::export]
pub fn set_camera_capability(on: bool) {
    camera::set_camera_cap(on);
}

/// Ekran paylaşımı saati (µs, süreç içi monoton). Karelerin `pts_us` değerleri PC'de bu saatten alınır.
#[uniffi::export]
pub fn screen_now_us() -> u64 {
    screen::now_us()
}

#[derive(uniffi::Object)]
pub struct Node {
    inner: Arc<Inner>,
    rt: Mutex<Option<tokio::runtime::Runtime>>,
}

impl Drop for Node {
    fn drop(&mut self) {
        let ep = self.inner.shutdown_endpoint();
        if let Some(rt) = lk(&self.rt).take() {
            // Çalışma zamanını hemen söndürmek CONNECTION_CLOSE paketini yarıda keser; karşı taraf
            // kopuşu ancak boşta-zaman aşımında (60 sn) görürdü. Kısa süre gönderimin bitmesini bekle.
            if let (Some(ep), Err(_)) = (ep, tokio::runtime::Handle::try_current()) {
                rt.block_on(async {
                    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), ep.wait_idle()).await;
                });
            }
            rt.shutdown_background();
        }
    }
}

/// UniFFI'ye açılmayan yardımcılar (test ve masaüstü).
impl Node {
    /// Tarama penceresi açık mı.
    pub fn is_scanning(&self) -> bool {
        self.inner.is_scanning()
    }

    /// PC: kodlayıcıya kare vermeden önce ayrıntılı geri bildirim (ack tabanlı gecikme, uçuştaki bayt, telefon istatistiği).
    pub fn screen_feedback_ex(&self, now_us: u64) -> ScreenFeedbackEx {
        self.inner.screen_feedback_ex(now_us)
    }

    /// Tarama penceresi süresini değiştirir (varsayılan 25 sn; testler için).
    pub fn set_scan_window(&self, d: std::time::Duration) {
        self.inner.set_scan_window(d)
    }
}

#[uniffi::export]
impl Node {
    /// Düğümü oluşturur ve başlatır (sunucu dinlemeye, istemci bağlanmaya başlar).
    #[uniffi::constructor]
    pub fn new(config: NodeConfig, host: Arc<dyn Host>) -> Result<Arc<Self>, KopruError> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("kopru-rt")
            .enable_all()
            .build()
            .map_err(|e| KopruError::new(format!("runtime: {e}")))?;
        let inner = Inner::new(config, host, rt.handle().clone())?;
        inner.start()?;
        Ok(Arc::new(Self { inner, rt: Mutex::new(Some(rt)) }))
    }

    pub fn device_id(&self) -> String {
        self.inner.device_id().to_string()
    }

    pub fn fingerprint(&self) -> String {
        self.inner.fingerprint().to_string()
    }

    /// Sunucunun gerçek dinleme portu.
    pub fn port(&self) -> u16 {
        self.inner.port()
    }

    pub fn state(&self) -> ConnState {
        self.inner.state()
    }

    pub fn current_peer(&self) -> Option<PeerInfo> {
        self.inner.current_peer()
    }

    pub fn peers(&self) -> Vec<PeerInfo> {
        self.inner.peers()
    }

    pub fn unpair(&self, id: String) {
        self.inner.unpair(&id)
    }

    /// Sunucu: yeni eşleştirme kodu üretir (5 dk geçerli) ve QR içeriğini döndürür.
    pub fn pairing_info(&self) -> PairingInfo {
        self.inner.pairing_info()
    }

    pub fn discovered(&self) -> Vec<ServerInfo> {
        self.inner.discovered()
    }

    /// İstemci: QR içeriğiyle eşleş.
    pub fn pair_uri(&self, uri: String) -> Result<(), KopruError> {
        Ok(self.inner.pair_uri(&uri)?)
    }

    /// İstemci: bulunan bilgisayarla 6 haneli kodla eşleş.
    pub fn pair_code(&self, server_id: String, code: String) -> Result<(), KopruError> {
        Ok(self.inner.pair_code(&server_id, &code)?)
    }

    /// Ağ değişti: hemen yeniden bağlanmayı dene.
    pub fn nudge(&self) {
        self.inner.nudge()
    }

    /// Ağ taramasını baştan başlatır.
    pub fn rescan(&self) {
        self.inner.rescan()
    }

    /// Yerel panodaki metni gönderir. Aynı metin tekrar gönderilmez (döngü önleme).
    pub fn send_clipboard(&self, text: String) -> bool {
        self.inner.send_clipboard(text)
    }

    pub fn send_files(&self, files: Vec<FileSource>) -> Result<Vec<u64>, KopruError> {
        Ok(self.inner.send_files(files)?)
    }

    /// Masaüstü: dosya/klasör yollarını gönderir (klasörler özyinelemeli).
    pub fn send_paths(&self, paths: Vec<String>) -> Result<Vec<u64>, KopruError> {
        let files = files::expand_paths(&paths)?;
        Ok(self.inner.send_files(files)?)
    }

    pub fn transfers(&self) -> Vec<TransferInfo> {
        self.inner.transfers()
    }

    pub fn clear_finished(&self) {
        self.inner.clear_finished()
    }

    pub fn pause_transfer(&self, id: u64, direction: Direction) -> Result<(), KopruError> {
        Ok(self.inner.pause_transfer(id, direction)?)
    }

    pub fn resume_transfer(&self, id: u64, direction: Direction) -> Result<(), KopruError> {
        Ok(self.inner.resume_transfer(id, direction)?)
    }

    pub fn cancel_transfer(&self, id: u64, direction: Direction) {
        self.inner.cancel_transfer(id, direction)
    }

    pub fn send_notification(&self, data: NotificationData) -> bool {
        self.inner.send_msg(Msg::Notif(data))
    }

    /// Eşin `notif-reply` yeteneği var mı (eski sürüm eş: false).
    pub fn peer_supports_notif_reply(&self) -> bool {
        self.inner.session().is_some_and(|s| s.has_cap(proto::CAP_NOTIF_REPLY))
    }

    /// Telefon -> PC: bildirimin yanıtlanabilirliği. Eş yeteneği duyurmadıysa gönderilmez (false).
    pub fn send_notification_ext(&self, key: String, can_reply: bool, choices: Vec<String>) -> bool {
        self.peer_supports_notif_reply() && self.inner.send_msg(Msg::NotifExt { key, can_reply, choices })
    }

    /// PC -> telefon: yanıt. Eş yeteneği duyurmadıysa gönderilmez (false).
    pub fn send_notification_reply(&self, key: String, text: String) -> bool {
        self.peer_supports_notif_reply() && self.inner.send_msg(Msg::NotifReply { key, text })
    }

    /// Telefon -> PC: yanıt sonucu.
    pub fn send_notification_reply_result(&self, key: String, ok: bool, error: Option<String>) -> bool {
        self.peer_supports_notif_reply() && self.inner.send_msg(Msg::NotifReplyResult { key, ok, error })
    }

    /// Eş `suite` yeteneğini duyurdu mu.
    pub fn peer_supports_suite(&self) -> bool {
        self.inner.session().is_some_and(|s| s.has_cap(proto::CAP_SUITE))
    }

    /// PC -> telefon: uygulama listesi.
    pub fn send_suite_apps(&self, apps: Vec<SuiteApp>) -> bool {
        self.peer_supports_suite() && self.inner.send_msg(Msg::SuiteApps { apps })
    }

    /// PC -> telefon: uygulama olayı.
    pub fn send_suite_event(&self, app: String, name: String, json: String) -> bool {
        self.peer_supports_suite() && self.inner.send_msg(Msg::SuiteEvent { app, name, json })
    }

    /// Telefon -> PC: uygulamaya komut.
    pub fn send_suite_cmd(&self, req: u64, app: String, name: String, json: String) -> bool {
        self.peer_supports_suite() && self.inner.send_msg(Msg::SuiteCmd { req, app, name, json })
    }

    /// PC -> telefon: komut yanıtı.
    pub fn send_suite_reply(&self, req: u64, ok: bool, json: String, error: Option<String>) -> bool {
        self.peer_supports_suite() && self.inner.send_msg(Msg::SuiteReply { req, ok, json, error })
    }

    /// Eş `camera` yeteneğini duyurdu mu (masaüstü: yalnızca Windows 11).
    pub fn peer_supports_camera(&self) -> bool {
        self.inner.session().is_some_and(|s| s.has_cap(proto::CAP_CAMERA))
    }

    /// PC -> telefon: kamera biçimi iste.
    pub fn send_cam_start(&self, w: u32, h: u32, fps: u32, bitrate_kbps: u32) -> bool {
        self.peer_supports_camera() && self.inner.send_msg(Msg::CamStart { w, h, fps, bitrate_kbps })
    }

    pub fn send_cam_stop(&self) -> bool {
        self.peer_supports_camera() && self.inner.send_msg(Msg::CamStop)
    }

    pub fn send_cam_keyframe(&self) -> bool {
        self.peer_supports_camera() && self.inner.send_msg(Msg::CamKeyframe)
    }

    /// Telefon -> PC: kamera durumu. rotation: karenin dik görünmesi için saat yönünde uygulanacak derece (0/90/180/270).
    #[allow(clippy::too_many_arguments)] // UniFFI ile Kotlin'e açık imza; değişmemeli
    pub fn send_cam_state(&self, active: bool, w: u32, h: u32, fps: u32, facing: u8, mirror: bool, rotation: u16) -> bool {
        self.peer_supports_camera() && self.inner.send_msg(Msg::CamState { active, w, h, fps, facing, mirror, rotation })
    }

    /// Telefon: kodlanmış kamera karesi gönder (geri basınç kapısı). false = atıldı.
    pub fn send_cam_frame(&self, pts_us: u64, key: bool, data: Vec<u8>) -> bool {
        self.inner.send_cam_frame(pts_us, key, data)
    }

    /// Telefon: (anahtar kare isteniyor mu, gönderim kuyruğu gecikmesi ms).
    pub fn cam_feedback(&self, newest_pts_us: u64) -> ScreenFeedback {
        let (want_key, backlog_ms) = self.inner.cam_feedback(newest_pts_us);
        ScreenFeedback { want_key, backlog_ms }
    }

    pub fn end_cam_stream(&self) {
        self.inner.end_cam_stream()
    }

    /// Eş `screen-tune` (ack, istatistik, saat senkronu, datagram işaretçi) duyuruyor mu.
    pub fn peer_supports_screen_tune(&self) -> bool {
        self.inner.session().is_some_and(|s| s.has_cap(proto::CAP_SCREEN_TUNE))
    }

    /// Eş ekran paylaşımı yeteneğini duyurdu mu.
    pub fn peer_supports_screen(&self) -> bool {
        self.inner.session().is_some_and(|s| s.has_cap(proto::CAP_SCREEN))
    }

    pub fn send_screen_start(&self, monitor: u32, max_w: u32, max_h: u32, fps: u32, bitrate_kbps: u32) -> bool {
        self.peer_supports_screen() && self.inner.send_msg(Msg::ScreenStart { monitor, max_w, max_h, fps, bitrate_kbps })
    }

    pub fn send_screen_stop(&self) -> bool {
        self.peer_supports_screen() && self.inner.send_msg(Msg::ScreenStop)
    }

    pub fn send_screen_keyframe(&self) -> bool {
        self.peer_supports_screen() && self.inner.send_msg(Msg::ScreenKeyframe)
    }

    pub fn send_screen_config(&self, max_w: u32, max_h: u32, fps: u32, bitrate_kbps: u32) -> bool {
        self.peer_supports_screen() && self.inner.send_msg(Msg::ScreenConfig { max_w, max_h, fps, bitrate_kbps })
    }

    pub fn send_screen_pointer(&self, x: f32, y: f32, monitor: u32) -> bool {
        self.peer_supports_screen() && self.inner.send_msg(Msg::ScreenPointer { x, y, monitor })
    }

    /// Mutlak işaretçi, düşük gecikme yolu: destekleyen PC'ye datagram (güvenilmez, sırada beklemez), değilse kontrol mesajı.
    pub fn send_screen_pointer_fast(&self, x: f32, y: f32, monitor: u32) -> bool {
        self.inner.send_screen_pointer_fast(x, y, monitor)
    }

    /// Telefon: saniyelik alıcı istatistiklerini PC'ye gönder (`screen-tune` yoksa yok sayılır).
    pub fn send_screen_stats(&self, r: ScreenStatsReport) -> bool {
        self.peer_supports_screen_tune() && self.inner.send_msg(Msg::ScreenStats(r))
    }

    /// Telefon: PC ile saat senkronu döngüsünü aç/kapat (ekran görüntülenirken açık).
    pub fn screen_sync(&self, on: bool) {
        self.inner.screen_sync(on)
    }

    /// Telefon: saat kestirimi (`pc_zamanı = screen_now_us() + offset_us`). Henüz örnek yoksa `valid=false`.
    pub fn screen_clock(&self) -> ScreenClock {
        self.inner.screen_clock()
    }

    pub fn send_screen_info(&self, monitors: Vec<MonitorInfo>, current: u32, active: bool, error: Option<String>) -> bool {
        self.peer_supports_screen() && self.inner.send_msg(Msg::ScreenInfo { monitors, current, active, error })
    }

    /// PC: güvenli masaüstü (UAC / kilit) açıldı (`true`) ya da kapandı (`false`); yalnızca `screen-share` duyuran eşe.
    pub fn send_screen_notice(&self, secure: bool) -> bool {
        self.peer_supports_screen() && self.inner.send_msg(Msg::ScreenNotice { secure })
    }

    /// PC: kodlanmış kareyi gönder (geri basınç kapısı). false = atıldı/gönderilemedi.
    pub fn send_screen_frame(&self, pts_us: u64, key: bool, data: Vec<u8>) -> bool {
        self.inner.send_screen_frame(pts_us, key, data)
    }

    /// PC: (anahtar kare isteniyor mu, gönderim kuyruğu gecikmesi ms).
    pub fn screen_feedback(&self, newest_pts_us: u64) -> ScreenFeedback {
        let (want_key, backlog_ms) = self.inner.screen_feedback(newest_pts_us);
        ScreenFeedback { want_key, backlog_ms }
    }

    pub fn end_screen_stream(&self) {
        self.inner.end_screen_stream()
    }

    pub fn remove_notification(&self, key: String) -> bool {
        self.inner.send_msg(Msg::NotifRemoved { key })
    }

    pub fn send_app_icon(&self, package: String, png: Vec<u8>) -> bool {
        self.inner.send_msg(Msg::AppIcon { package, png })
    }

    pub fn send_battery(&self, level: u8, charging: bool) -> bool {
        self.inner.send_msg(Msg::Battery { level, charging })
    }

    pub fn send_now_playing(&self, info: Option<NowPlaying>) -> bool {
        self.inner.send_msg(Msg::NowPlaying(info))
    }

    /// Fare hareketi (datagram; kaybolabilir, gecikmesi en düşük yol).
    pub fn send_pointer(&self, dx: i32, dy: i32) {
        self.inner.send_pointer(dx, dy)
    }

    /// Eş `key-hold` (KeyDown/KeyUp) duyuruyor mu.
    pub fn peer_supports_key_hold(&self) -> bool {
        self.inner.session().is_some_and(|s| s.has_cap(proto::CAP_KEY_HOLD))
    }

    /// Tuşa bas/bırak (`key-hold` yoksa false; arayüz mevcut Key/combo mesajlarına döner).
    pub fn send_key(&self, key: keys::KeyCode, down: bool) -> bool {
        self.peer_supports_key_hold() && self.inner.send_msg(if down { Msg::KeyDown { key } } else { Msg::KeyUp { key } })
    }

    pub fn send_pc(&self, cmd: PcCommand) -> bool {
        self.inner.send_msg(Msg::Pc(cmd))
    }

    pub fn send_phone(&self, cmd: PhoneCommand) -> bool {
        self.inner.send_msg(Msg::Phone(cmd))
    }

    pub fn shutdown(&self) {
        self.inner.shutdown()
    }
}
