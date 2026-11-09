//! Platformlar arası paylaşılan veri tipleri. Hem UniFFI (Kotlin) hem serde (ağ + Tauri) için.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum Platform {
    Desktop,
    Android,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum Role {
    /// Dinler (PC).
    Server,
    /// Bağlanır (telefon).
    Client,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct NodeConfig {
    /// Kimlik, eşler ve kuyruk dosyalarının tutulduğu klasör.
    pub data_dir: String,
    pub device_name: String,
    pub platform: Platform,
    pub role: Role,
    /// Sunucu için dinleme portu (0 = otomatik). İstemcide yok sayılır.
    pub port: u16,
    /// Masaüstü: gelen dosyaların klasörü. `None` ise dosyalar Host üzerinden (Android fd) yazılır.
    pub download_dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct PeerInfo {
    pub id: String,
    pub name: String,
    pub platform: Platform,
    /// Karşı tarafın sertifika parmak izi (blake3, hex).
    pub fingerprint: String,
    /// Son bilinen adresler ("ip:port").
    pub addrs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum ConnState {
    Disconnected,
    Connecting,
    Connected,
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct PairingInfo {
    /// QR içeriği: tbridge://pair?... (eski kopru:// da kabul edilir)
    pub uri: String,
    /// 6 haneli kod (elle giriş için).
    pub code: String,
    pub addrs: Vec<String>,
    pub fingerprint: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct ServerInfo {
    pub id: String,
    pub name: String,
    pub fingerprint: String,
    pub addrs: Vec<String>,
    /// Daha önce eşleşilmiş mi?
    pub paired: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum Direction {
    Outgoing,
    Incoming,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum TransferState {
    /// Sırada, bağlantı veya sıra bekliyor.
    Queued,
    Active,
    Paused,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct TransferInfo {
    pub id: u64,
    pub direction: Direction,
    /// Göreli yol ("Klasör/alt/dosya.txt" veya "dosya.txt").
    pub name: String,
    pub size: u64,
    pub done: u64,
    pub state: TransferState,
    /// Bayt/sn (anlık).
    pub speed: u64,
    /// Tamamlanan gelen dosyanın konumu veya hata mesajı.
    pub detail: Option<String>,
}

/// Gönderilecek dosya. Masaüstünde `source` dosya yolu, Android'de content:// URI'dir.
#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct FileSource {
    pub source: String,
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct NotificationData {
    pub key: String,
    pub package: String,
    pub app_name: String,
    pub title: String,
    pub text: String,
    /// Unix ms.
    pub time: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub playing: bool,
    pub app_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum SpecialKey {
    Enter,
    Backspace,
    Delete,
    Escape,
    Tab,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Win,
    AltTab,
    CopyShortcut,
    PasteShortcut,
    F5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum MediaAction {
    PlayPause,
    Next,
    Previous,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum VolumeAction {
    Up,
    Down,
    Mute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum PowerAction {
    Lock,
    Sleep,
    Shutdown,
    Restart,
}

/// Telefon → PC kumanda komutları (güvenilir kanal).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Enum)]
pub enum PcCommand {
    Button { button: MouseButton, down: bool },
    Click { button: MouseButton, double: bool },
    Scroll { dx: i32, dy: i32 },
    Text { text: String },
    Key { key: SpecialKey },
    Media { action: MediaAction },
    Volume { action: VolumeAction },
    Power { action: PowerAction },
}

/// PC → telefon kumanda komutları.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Enum)]
pub enum PhoneCommand {
    Ring { on: bool },
    Media { action: MediaAction },
    Volume { action: VolumeAction },
    DismissNotification { key: String },
}

/// PC'deki bir monitör (ekran paylaşımı).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct MonitorInfo {
    pub index: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

/// Telefonun saniyelik ekran alıcı istatistikleri (PC günlüğü + uyarlama girdisi). Ondalıklı değerler ×10 tamsayıdır.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct ScreenStatsReport {
    /// Alınan kare/sn ×10.
    pub rx_fps10: u16,
    /// Ekrana çizilen kare/sn ×10.
    pub render_fps10: u16,
    /// Çözücü girişi → çıkış ortalaması (ms ×10).
    pub decode_ms10: u16,
    /// Ağdan alındı → çizildi ortalaması (ms ×10).
    pub recv_to_render_ms10: u16,
    /// Uçtan uca (PC yakalama → telefon çizim) ortalama ve en büyük (ms); saat senkronu yoksa 0.
    pub e2e_ms: u16,
    pub e2e_max_ms: u16,
    /// Geç/dolu çözücü nedeniyle atılan kare sayısı.
    pub late_drops: u16,
    /// Çözücü girişi için bekleme zaman aşımı sayısı.
    pub input_waits: u16,
    /// Çözücü kuyruğunda bekleyen en çok kare.
    pub queue_max: u16,
    /// Saat senkronu ağ gidiş-dönüş süresi (ms ×10).
    pub rtt_ms10: u16,
}

/// PC içi (uniffi dışı): video üretimi öncesi geri bildirim.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScreenFeedbackEx {
    pub want_key: bool,
    /// Teslim edilmemiş en eski kareden şimdiye (ms).
    pub lag_ms: u64,
    pub inflight_bytes: usize,
    pub pending: u32,
    pub dropped_total: u64,
    pub ack_mode: bool,
    /// Telefonun son (≤3 sn eski) alıcı istatistikleri.
    pub phone: Option<ScreenStatsReport>,
}

/// Telefon saat kestirimi: `pc_time = yerel_time + offset_us` (screen_now_us saatinde).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, uniffi::Record)]
pub struct ScreenClock {
    pub valid: bool,
    pub offset_us: i64,
    pub rtt_us: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct ScreenFeedback {
    pub want_key: bool,
    pub backlog_ms: u64,
}

/// Telefondan PC'ye ekran paylaşımı denetim komutları.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Enum)]
pub enum ScreenCmd {
    Start { monitor: u32, max_w: u32, max_h: u32, fps: u32, bitrate_kbps: u32 },
    Stop,
    Keyframe,
    Config { max_w: u32, max_h: u32, fps: u32, bitrate_kbps: u32 },
    /// Mutlak işaretçi konumu (0..1, seçili monitöre göre).
    Pointer { x: f32, y: f32, monitor: u32 },
}

/// T-Suite uygulama kaydı (genel: uygulamaya özgü alan yok).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct SuiteApp {
    pub id: String,
    pub name: String,
    pub version: String,
    pub running: bool,
    pub update_available: Option<String>,
}

/// PC'den telefona kamera komutları.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Enum)]
pub enum CamCmd {
    Start { w: u32, h: u32, fps: u32, bitrate_kbps: u32 },
    Stop,
    Keyframe,
}

/// Çekirdekten platforma giden olaylar.
#[derive(Debug, Clone, Serialize, uniffi::Enum)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    State { state: ConnState, peer: Option<PeerInfo> },
    Paired { peer: PeerInfo },
    PairFailed { reason: String },
    /// PC'de: yanlış denemeler yüzünden eşleştirme kodu geçersiz kılındı; arayüz yeni kodu okumalı.
    PairCodeRotated,
    Discovered { servers: Vec<ServerInfo> },
    /// İstemci: sınırlı mDNS tarama penceresi açıldı/kapandı (Android MulticastLock buna bağlı).
    Scanning { on: bool },
    ClipboardReceived { text: String },
    Transfer { info: TransferInfo },
    TransferRemoved { id: u64, direction: Direction },
    Notification { data: NotificationData },
    NotificationRemoved { key: String },
    /// Telefon: bildirim yanıtlanabilir mi (+ hazır yanıtlar).
    NotificationExt { key: String, can_reply: bool, choices: Vec<String> },
    /// Telefonda: PC bir bildirime yanıt yazdı.
    NotificationReply { key: String, text: String },
    /// PC'de: yanıtın telefondaki sonucu.
    NotificationReplyResult { key: String, ok: bool, error: Option<String> },
    /// PC'de: telefon ekran paylaşımı komutu gönderdi.
    Screen { cmd: ScreenCmd },
    /// Telefonda: PC monitör listesi / durum (error doluysa reddedildi veya başlatılamadı).
    ScreenInfo { monitors: Vec<MonitorInfo>, current: u32, active: bool, error: Option<String> },
    /// Telefonda: PC güvenli masaüstüne geçti (`secure=true`, yakalama durur) ya da geri döndü.
    ScreenNotice { secure: bool },
    /// Telefonda: kodlanmış H.264 (Annex-B) video karesi.
    /// `recv_us` = karenin ağdan tam alındığı an (`screen_now_us` saatinde).
    ScreenFrame { pts_us: u64, recv_us: u64, key: bool, data: Vec<u8> },
    /// Telefonda: T-Suite uygulama listesi.
    SuiteApps { apps: Vec<SuiteApp> },
    /// Telefonda: bir T-Suite uygulamasının olayı.
    SuiteEvent { app: String, name: String, json: String },
    /// PC'de: telefon bir T-Suite uygulamasına komut gönderdi.
    SuiteCmd { req: u64, app: String, name: String, json: String },
    /// Telefonda: komut yanıtı.
    SuiteReply { req: u64, ok: bool, json: String, error: Option<String> },
    /// Telefonda: PC kamera komutu gönderdi.
    Cam { cmd: CamCmd },
    /// PC'de: telefon kamera durumu.
    CamState { active: bool, w: u32, h: u32, fps: u32, facing: u8, mirror: bool, rotation: u16 },
    /// PC'de: telefon kamerasından kodlanmış H.264 (Annex-B) kare.
    CamFrame { pts_us: u64, key: bool, data: Vec<u8> },
    AppIcon { package: String, png: Vec<u8> },
    Battery { level: u8, charging: bool },
    NowPlaying { info: Option<NowPlaying> },
    /// PC'de: fare hareketi (datagram, düşük gecikme).
    PointerMove { dx: i32, dy: i32 },
    Pc { cmd: PcCommand },
    Phone { cmd: PhoneCommand },
}

/// Android'in dosya açması için fd (sahipliği çekirdeğe geçer).
#[derive(Debug, Clone, uniffi::Record)]
pub struct OpenedFd {
    pub fd: i32,
    /// Hedefte zaten yazılı olan bayt sayısı (devam için).
    pub existing: u64,
}
