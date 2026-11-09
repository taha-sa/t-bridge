//! Ekran paylaşımı: video akışı çerçeveleme + geri basınç (backpressure) mantığı.
//!
//! Tel biçimi (PC -> telefon, oturum başına TEK tek yönlü QUIC akışı):
//!   8 bayt  VIDEO_MAGIC (dosya aktarım akışlarının 8 baytlık `id` başlığından ayrışır)
//!   sonra yinelenen çerçeveler:  pts_us:u64 LE | flags:u8 (bit0 = anahtar kare) | len:u32 LE | Annex-B H.264 verisi
use std::collections::VecDeque;

pub const VIDEO_MAGIC: u64 = 0x5343_5245_454E_4B31; // "SCREENK1"
pub const HEADER_LEN: usize = 13;
/// Tek kare üst sınırı (anahtar kareler büyük olabilir).
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
/// Gönderim kuyruğu bu süreyi (ms, pts farkı) aşarsa anahtar olmayan kareler atılır.
pub const MAX_BACKLOG_MS: u64 = 150;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    pub pts_us: u64,
    pub key: bool,
    pub len: u32,
}

impl FrameHeader {
    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut b = [0u8; HEADER_LEN];
        b[..8].copy_from_slice(&self.pts_us.to_le_bytes());
        b[8] = self.key as u8;
        b[9..].copy_from_slice(&self.len.to_le_bytes());
        b
    }

    pub fn decode(b: &[u8; HEADER_LEN]) -> Option<Self> {
        let len = u32::from_le_bytes(b[9..13].try_into().ok()?);
        if len as usize > MAX_FRAME_BYTES {
            return None;
        }
        Some(Self { pts_us: u64::from_le_bytes(b[..8].try_into().ok()?), key: b[8] & 1 == 1, len })
    }
}

/// Karar: bu kare gönderim kuyruğuna alınsın mı?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offer {
    Accept,
    /// Atıldı; sonraki anahtar kareye kadar tüm P kareleri de atılır (çözücü bozulmasın).
    Drop,
}

/// Geri basınç: kuyrukta bekleyen karelerin pts yayılımı (ms) 150 ms'yi aşarsa P kareleri atar ve anahtar kare ister.
#[derive(Debug, Default)]
pub struct Gate {
    queue: VecDeque<u64>, // bekleyen karelerin pts (µs)
    waiting_key: bool,
    want_key: bool,
    pub dropped: u64,
}

impl Gate {
    pub fn new() -> Self {
        Self::default()
    }

    /// Kuyruktaki en eski ile en yeni kare arasındaki fark (ms).
    pub fn backlog_ms(&self, newest_pts_us: u64) -> u64 {
        self.queue.front().map(|f| newest_pts_us.saturating_sub(*f) / 1000).unwrap_or(0)
    }

    pub fn offer(&mut self, pts_us: u64, key: bool) -> Offer {
        if key {
            self.waiting_key = false;
            self.queue.push_back(pts_us);
            return Offer::Accept;
        }
        if self.waiting_key || self.backlog_ms(pts_us) > MAX_BACKLOG_MS {
            self.waiting_key = true;
            self.want_key = true;
            self.dropped += 1;
            return Offer::Drop;
        }
        self.queue.push_back(pts_us);
        Offer::Accept
    }

    /// Yazıcı bir kareyi akışa yazdı.
    pub fn done(&mut self) {
        self.queue.pop_front();
    }

    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// Atılan kareler nedeniyle kodlayıcıdan anahtar kare istenmeli mi (bir kez döner).
    pub fn take_want_key(&mut self) -> bool {
        std::mem::take(&mut self.want_key)
    }
}

/// Uyarlanabilir bit hızı: geri basınç büyürse düşür, uzun süre rahatsa kademeli geri çıkar.
pub fn adapt_bitrate(current_kbps: u32, target_kbps: u32, backlog_ms: u64) -> u32 {
    let floor = (target_kbps / 4).max(300);
    if backlog_ms > 100 {
        ((current_kbps as f32 * 0.75) as u32).max(floor)
    } else if backlog_ms < 20 && current_kbps < target_kbps {
        ((current_kbps as f32 * 1.1) as u32 + 1).min(target_kbps)
    } else {
        current_kbps
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    pub max_w: u32,
    pub max_h: u32,
    pub fps: u32,
    pub bitrate_kbps: u32,
}

pub const PRESET_ECO: Preset = Preset { max_w: 1280, max_h: 720, fps: 15, bitrate_kbps: 2000 };
pub const PRESET_BALANCED: Preset = Preset { max_w: 1920, max_h: 1080, fps: 30, bitrate_kbps: 5000 };
pub const PRESET_SMOOTH: Preset = Preset { max_w: 1920, max_h: 1080, fps: 60, bitrate_kbps: 8000 };

/// Hedef boyutu en-boy oranını koruyarak sınırlar; H.264 için çift sayıya yuvarlar. Büyütme yok.
pub fn fit_size(src_w: u32, src_h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    if src_w == 0 || src_h == 0 {
        return (2, 2);
    }
    let s = (max_w as f64 / src_w as f64).min(max_h as f64 / src_h as f64).min(1.0);
    let w = ((src_w as f64 * s) as u32).max(2) & !1;
    let h = ((src_h as f64 * s) as u32).max(2) & !1;
    (w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip_and_limits() {
        let h = FrameHeader { pts_us: 123_456_789, key: true, len: 4242 };
        assert_eq!(FrameHeader::decode(&h.encode()), Some(h));
        let mut bad = h.encode();
        bad[9..].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(FrameHeader::decode(&bad), None);
        assert_ne!(VIDEO_MAGIC, 0);
    }

    #[test]
    fn gate_accepts_until_backlog_then_drops_until_key() {
        let mut g = Gate::new();
        assert_eq!(g.offer(0, true), Offer::Accept);
        for i in 1..=9u64 {
            assert_eq!(g.offer(i * 16_000, false), Offer::Accept, "144 ms içinde kabul");
        }
        // 160 ms > 150 ms: ilk P atılır
        assert_eq!(g.offer(160_000, false), Offer::Drop);
        assert!(g.take_want_key());
        assert!(!g.take_want_key(), "istek bir kez döner");
        // kuyruk boşalsa bile anahtar gelene kadar P atılır
        while g.pending() > 0 {
            g.done();
        }
        assert_eq!(g.offer(200_000, false), Offer::Drop);
        assert_eq!(g.offer(220_000, true), Offer::Accept);
        assert_eq!(g.offer(236_000, false), Offer::Accept);
        assert!(g.dropped >= 2);
    }

    #[test]
    fn gate_keyframes_always_accepted_even_when_backlogged() {
        let mut g = Gate::new();
        g.offer(0, true);
        assert_eq!(g.offer(1_000_000, true), Offer::Accept);
    }

    #[test]
    fn bitrate_adapts_down_and_recovers() {
        let t = 5000;
        let a = adapt_bitrate(t, t, 200);
        assert!(a < t && a >= 1250);
        let mut cur = 1300;
        for _ in 0..40 {
            cur = adapt_bitrate(cur, t, 0);
        }
        assert_eq!(cur, t);
        assert_eq!(adapt_bitrate(3000, t, 60), 3000);
    }

    #[test]
    fn fit_size_keeps_aspect_even_and_never_upscales() {
        assert_eq!(fit_size(3840, 2160, 1920, 1080), (1920, 1080));
        assert_eq!(fit_size(2560, 1440, 1280, 720), (1280, 720));
        assert_eq!(fit_size(1024, 768, 1920, 1080), (1024, 768));
        let (w, h) = fit_size(1366, 768, 1280, 720);
        assert!(w % 2 == 0 && h % 2 == 0 && w <= 1280 && h <= 720);
    }
}
