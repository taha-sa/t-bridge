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
pub mod node;
pub mod proto;
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

#[derive(uniffi::Object)]
pub struct Node {
    inner: Arc<Inner>,
    rt: Mutex<Option<tokio::runtime::Runtime>>,
}

impl Drop for Node {
    fn drop(&mut self) {
        self.inner.shutdown();
        if let Some(rt) = lk(&self.rt).take() {
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
