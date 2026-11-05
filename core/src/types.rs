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

/// Çekirdekten platforma giden olaylar.
#[derive(Debug, Clone, Serialize, uniffi::Enum)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    State { state: ConnState, peer: Option<PeerInfo> },
    Paired { peer: PeerInfo },
    PairFailed { reason: String },
    Discovered { servers: Vec<ServerInfo> },
    ClipboardReceived { text: String },
    Transfer { info: TransferInfo },
    TransferRemoved { id: u64, direction: Direction },
    Notification { data: NotificationData },
    NotificationRemoved { key: String },
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
