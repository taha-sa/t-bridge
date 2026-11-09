//! Ekran paylaşımı: video akışı çerçeveleme + geri basınç (backpressure) + boyut/bit hızı seçimi + uyarlanabilir kalite.
//!
//! Tel biçimi (PC -> telefon, oturum başına TEK tek yönlü QUIC akışı):
//!   8 bayt  VIDEO_MAGIC (dosya aktarım akışlarının 8 baytlık `id` başlığından ayrışır)
//!   sonra yinelenen çerçeveler:  pts_us:u64 LE | flags:u8 (bit0 = anahtar kare) | len:u32 LE | Annex-B H.264 verisi
//! `pts_us` = karenin PC'de yakalandığı an, `now_us()` saatinde (telefon ping/pong ile saat farkını kestirir).
use std::collections::VecDeque;
use std::sync::OnceLock;
use std::time::Instant;

pub const VIDEO_MAGIC: u64 = 0x5343_5245_454E_4B31; // "SCREENK1"
pub const HEADER_LEN: usize = 13;
/// Tek kare üst sınırı (anahtar kareler büyük olabilir).
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
/// Ack vermeyen (eski) eşte: gönderim kuyruğu bu süreyi (ms, pts farkı) aşarsa anahtar olmayan kareler atılır.
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

/// Ack alan eşte (yetenek `screen-tune`): yazılmış ama henüz onaylanmamış en eski karenin yaşı bu süreyi aşarsa
/// KODLAYICIYA YENİ KARE VERİLMEZ (kare üretilmez; referans zinciri bozulmaz, anahtar kare gerekmez; en yeni kare sonra gider).
pub const SKIP_LAG_MS: u64 = 140;
/// Ack modunda son çare: yazılmamış kuyrukta bu kadar birikirse P kareleri atılır + anahtar kare istenir.
pub const ACK_DROP_BACKLOG_MS: u64 = 450;
/// Onaysız kalan kayıtlar (kayıp ack) bu süreden sonra temizlenir.
const INFLIGHT_PRUNE_US: u64 = 3_000_000;

/// Geri basınç: iki aşamalı takip.
/// 1) `queue`: kodlandı, akışa henüz yazılmadı.
/// 2) `inflight`: yazıldı, alıcı henüz onaylamadı (yalnızca ack modunda). QUIC yazma çağrısı büyük pencere yüzünden hemen
///    döndüğünden, gerçek ağ/alıcı birikimini yalnızca alıcı onayı gösterir.
#[derive(Debug, Default)]
pub struct Gate {
    queue: VecDeque<(u64, u32)>,    // (pts µs, bayt)
    inflight: VecDeque<(u64, u32)>, // yazıldı, onay bekliyor
    ack_mode: bool,
    waiting_key: bool,
    want_key: bool,
    pub dropped: u64,
}

impl Gate {
    pub fn new() -> Self {
        Self::default()
    }

    /// Alıcı `screen-tune` duyuruyorsa: onay (ack) tabanlı gecikme ölçümü.
    pub fn with_ack(ack: bool) -> Self {
        Self { ack_mode: ack, ..Self::default() }
    }

    /// Yazıcı çöktükten sonra yeniden açılan akış için: ilk anahtar kareye dek P kareleri atar ve anahtar kare ister.
    pub fn awaiting_key(ack: bool) -> Self {
        Self { ack_mode: ack, waiting_key: true, want_key: true, ..Self::default() }
    }

    pub fn ack_mode(&self) -> bool {
        self.ack_mode
    }

    /// Kuyruktaki en eski ile en yeni kare arasındaki fark (ms).
    pub fn backlog_ms(&self, newest_pts_us: u64) -> u64 {
        self.queue.front().map(|f| newest_pts_us.saturating_sub(f.0) / 1000).unwrap_or(0)
    }

    /// Henüz teslim edilmemiş (yazılmamış ya da onaysız) en eski kareden şimdiye geçen süre (ms). `now_us` = pts saatinde şimdi.
    pub fn lag_ms(&self, now_us: u64) -> u64 {
        let oldest = match (self.inflight.front(), self.queue.front()) {
            (Some(a), Some(b)) => Some(a.0.min(b.0)),
            (Some(a), None) => Some(a.0),
            (None, Some(b)) => Some(b.0),
            (None, None) => None,
        };
        oldest.map(|o| now_us.saturating_sub(o) / 1000).unwrap_or(0)
    }

    /// Henüz teslimi onaylanmamış toplam bayt (kuyruk + uçuşta).
    pub fn inflight_bytes(&self) -> usize {
        self.queue.iter().chain(self.inflight.iter()).map(|x| x.1 as usize).sum()
    }

    pub fn offer(&mut self, pts_us: u64, key: bool, bytes: u32) -> Offer {
        while self.inflight.front().is_some_and(|f| f.0 + INFLIGHT_PRUNE_US < pts_us) {
            self.inflight.pop_front();
        }
        if key {
            self.waiting_key = false;
            self.queue.push_back((pts_us, bytes));
            return Offer::Accept;
        }
        let limit = if self.ack_mode { ACK_DROP_BACKLOG_MS } else { MAX_BACKLOG_MS };
        if self.waiting_key || self.backlog_ms(pts_us) > limit {
            self.waiting_key = true;
            self.want_key = true;
            self.dropped += 1;
            return Offer::Drop;
        }
        self.queue.push_back((pts_us, bytes));
        Offer::Accept
    }

    /// Yazıcı bir kareyi akışa yazdı.
    pub fn done(&mut self) {
        if let Some(f) = self.queue.pop_front()
            && self.ack_mode
        {
            self.inflight.push_back(f);
        }
    }

    /// Alıcı `pts_us` dahil öncesindeki tüm kareleri aldığını bildirdi.
    pub fn ack(&mut self, pts_us: u64) {
        while self.inflight.front().is_some_and(|f| f.0 <= pts_us) {
            self.inflight.pop_front();
        }
    }

    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    pub fn inflight(&self) -> usize {
        self.inflight.len()
    }

    /// Atılan kareler nedeniyle kodlayıcıdan anahtar kare istenmeli mi (bir kez döner).
    pub fn take_want_key(&mut self) -> bool {
        std::mem::take(&mut self.want_key)
    }
}

// ---------------------------------------------------------------- saat + senkron

/// Süreç ömrü boyunca tek, monoton saat tabanı (µs). pts, ping/pong ve varış damgaları hep bu saatten alınır.
pub fn now_us() -> u64 {
    static BASE: OnceLock<Instant> = OnceLock::new();
    BASE.get_or_init(Instant::now).elapsed().as_micros() as u64
}

/// NTP benzeri saat farkı kestirimi (ping/pong). `offset` = uzak saat − yerel saat (aynı an).
#[derive(Debug, Default, Clone)]
pub struct ClockSync {
    samples: VecDeque<(u64, i64)>, // (rtt µs, offset µs)
}

const SYNC_KEEP: usize = 8;
const SYNC_MAX_RTT_US: u64 = 2_000_000;

impl ClockSync {
    /// `t0` = yerel gönderim, `t1` = uzak saatin yanıt anındaki değeri, `t2` = yerel varış. Geçersiz örnek false döner.
    pub fn sample(&mut self, t0: u64, t1: u64, t2: u64) -> bool {
        if t2 < t0 || t2 - t0 > SYNC_MAX_RTT_US {
            return false;
        }
        let rtt = t2 - t0;
        let mid = t0 as i128 + (rtt / 2) as i128;
        let off = (t1 as i128 - mid) as i64;
        self.samples.push_back((rtt, off));
        while self.samples.len() > SYNC_KEEP {
            self.samples.pop_front();
        }
        true
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }

    /// En düşük RTT'li örnek en güvenilir olandır (asimetrik kuyruk gecikmesi en az).
    pub fn best(&self) -> Option<(u64, i64)> {
        self.samples.iter().copied().min_by_key(|s| s.0)
    }

    pub fn offset_us(&self) -> Option<i64> {
        self.best().map(|b| b.1)
    }

    pub fn rtt_us(&self) -> Option<u64> {
        self.best().map(|b| b.0)
    }

    /// Uzak saatte damgalı `pts_us` karesinin yerel `local_now_us` anındaki uçtan uca yaşı (ms). Senkron yoksa None.
    pub fn age_ms(&self, pts_us: u64, local_now_us: u64) -> Option<i64> {
        let off = self.offset_us()?;
        Some(((local_now_us as i128 + off as i128 - pts_us as i128) / 1000) as i64)
    }
}

// ---------------------------------------------------------------- boyut + bit hızı seçimi

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    pub max_w: u32,
    pub max_h: u32,
    pub fps: u32,
    /// 0 = otomatik (boyut ve fps'e göre `auto_bitrate_kbps`).
    pub bitrate_kbps: u32,
}

pub const PRESET_ECO: Preset = Preset { max_w: 1280, max_h: 720, fps: 15, bitrate_kbps: 0 };
pub const PRESET_BALANCED: Preset = Preset { max_w: 1920, max_h: 1080, fps: 30, bitrate_kbps: 0 };
pub const PRESET_SMOOTH: Preset = Preset { max_w: 1920, max_h: 1080, fps: 60, bitrate_kbps: 0 };

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

/// Kodlama boyutu: telefonun bildirdiği görüntü alanına (piksel) sığacak en büyük, kaynak en-boyunu koruyan boyut;
/// kaynaktan büyütme yok; `res_pct` (25..100) uyarlanabilir çözünürlük kademesi. Çift sayıya yuvarlanır.
pub fn choose_size(src_w: u32, src_h: u32, view_w: u32, view_h: u32, res_pct: u32) -> (u32, u32) {
    let (w, h) = fit_size(src_w, src_h, view_w.max(2), view_h.max(2));
    if res_pct >= 100 {
        return (w, h);
    }
    let s = res_pct.clamp(25, 100) as f64 / 100.0;
    ((((w as f64 * s) as u32).max(2)) & !1, (((h as f64 * s) as u32).max(2)) & !1)
}

/// Masaüstü (metin ağırlıklı) içerik için referans bit/piksel (H.264 CBR; donanım kodlayıcılar x264'ten biraz verimsiz).
const BPP_REF: f64 = 0.13;
pub const MIN_KBPS: u32 = 400;
pub const MAX_KBPS: u32 = 20_000;

/// Boyuta ve kare hızına göre bit hızı (kbps). Hareketi az olan masaüstünde bit hızı fps ile doğrusal artmaz: fps^0.7.
/// 1080p30 ≈ 8,1 Mbps · 1080p60 ≈ 13,1 Mbps · 720p15 ≈ 2,2 Mbps.
pub fn auto_bitrate_kbps(w: u32, h: u32, fps: u32) -> u32 {
    let eff = 30.0 * (fps.max(1) as f64 / 30.0).powf(0.7);
    let kbps = w as f64 * h as f64 * BPP_REF * eff / 1000.0;
    (kbps.round() as u32).clamp(MIN_KBPS, MAX_KBPS)
}

/// İstenen bit hızı (0 = otomatik tablo) ve kademe yüzdesiyle gerçek hedef.
pub fn target_bitrate_kbps(requested: u32, w: u32, h: u32, fps: u32, bitrate_pct: u32) -> u32 {
    let base = if requested == 0 { auto_bitrate_kbps(w, h, fps) } else { requested.clamp(MIN_KBPS, MAX_KBPS) };
    ((base as u64 * bitrate_pct.clamp(10, 100) as u64 / 100) as u32).max(300)
}

// ---------------------------------------------------------------- uyarlanabilir kalite

/// Bir kalite kademesi: kare hızı, bit hızı yüzdesi, çözünürlük yüzdesi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quality {
    pub fps: u32,
    pub bitrate_pct: u32,
    pub res_pct: u32,
}

/// Kademe merdiveni: önce fps düşer (60→45→30→20→15), sonra bit hızı (%75, %55), en sonda çözünürlük (%75, %50).
pub fn ladder(target_fps: u32) -> Vec<Quality> {
    let target = target_fps.clamp(5, 60);
    let mut fps_steps = vec![target];
    for f in [45, 30, 20, 15] {
        if f < *fps_steps.last().unwrap() {
            fps_steps.push(f);
        }
    }
    let mut v: Vec<Quality> = fps_steps.iter().map(|&fps| Quality { fps, bitrate_pct: 100, res_pct: 100 }).collect();
    let low = *fps_steps.last().unwrap();
    v.push(Quality { fps: low, bitrate_pct: 75, res_pct: 100 });
    v.push(Quality { fps: low, bitrate_pct: 55, res_pct: 100 });
    v.push(Quality { fps: low, bitrate_pct: 55, res_pct: 75 });
    v.push(Quality { fps: low, bitrate_pct: 45, res_pct: 50 });
    v
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Congested,
    Healthy,
    Neutral,
}

/// 500 ms'lik pencere özetinden karar: `lag_max_ms` = pencere içi en büyük teslim gecikmesi, `skipped` = geri basınçla
/// üretilmeyen/atılan kare, `phone_drops` = telefonun çözücüde attığı kare.
pub fn verdict(lag_max_ms: u64, skipped: u32, phone_drops: u32) -> Verdict {
    if skipped > 0 || phone_drops > 0 || lag_max_ms > SKIP_LAG_MS {
        Verdict::Congested
    } else if lag_max_ms < 80 {
        Verdict::Healthy
    } else {
        Verdict::Neutral
    }
}

const TICKS_HEALTHY_BASE: u32 = 8; // 8 × 500 ms = 4 sn rahat → bir kademe yukarı
const TICKS_HEALTHY_MAX: u32 = 48;
const TICKS_MIN_DWELL: u32 = 2; // iki kademe değişimi arası en az 1 sn
const TICKS_FLAP: u32 = 24; // yukarı çıktıktan bu kadar tık içinde tekrar bozulursa "çırpınma"

/// Kalite durum makinesi. Bozulmada hızlı iner (kademe kademe), toparlanmada yavaş ve histerezisle çıkar;
/// çırpınma görülürse toparlanma bekleme süresi ikiye katlanır.
#[derive(Debug, Clone)]
pub struct Adaptor {
    rungs: Vec<Quality>,
    idx: usize,
    healthy: u32,
    need_healthy: u32,
    since_change: u32,
    since_up: u32,
    stable: u32,
}

impl Adaptor {
    pub fn new(target_fps: u32) -> Self {
        Self {
            rungs: ladder(target_fps),
            idx: 0,
            healthy: 0,
            need_healthy: TICKS_HEALTHY_BASE,
            since_change: TICKS_MIN_DWELL,
            since_up: u32::MAX / 2,
            stable: 0,
        }
    }

    pub fn current(&self) -> Quality {
        self.rungs[self.idx]
    }

    pub fn level(&self) -> usize {
        self.idx
    }

    pub fn levels(&self) -> usize {
        self.rungs.len()
    }

    /// Her 500 ms'de bir çağrılır. Kademe değiştiyse yeni kaliteyi döndürür.
    pub fn tick(&mut self, v: Verdict) -> Option<Quality> {
        self.since_change = self.since_change.saturating_add(1);
        self.since_up = self.since_up.saturating_add(1);
        match v {
            Verdict::Congested => {
                self.healthy = 0;
                self.stable = 0;
                if self.idx + 1 < self.rungs.len() && self.since_change >= TICKS_MIN_DWELL {
                    if self.since_up < TICKS_FLAP {
                        self.need_healthy = (self.need_healthy * 2).min(TICKS_HEALTHY_MAX);
                    }
                    self.idx += 1;
                    self.since_change = 0;
                    return Some(self.current());
                }
            }
            Verdict::Healthy => {
                self.stable += 1;
                if self.stable > 120 {
                    self.need_healthy = TICKS_HEALTHY_BASE;
                }
                if self.idx > 0 {
                    self.healthy += 1;
                    if self.healthy >= self.need_healthy {
                        self.healthy = 0;
                        self.idx -= 1;
                        self.since_change = 0;
                        self.since_up = 0;
                        return Some(self.current());
                    }
                }
            }
            Verdict::Neutral => {}
        }
        None
    }
}

// ---------------------------------------------------------------- PC tarafı saniyelik istatistik

/// Saniyelik PC istatistik penceresi (günlük satırı için).
#[derive(Debug, Default, Clone)]
pub struct StatsWindow {
    pub grabs: u32,
    pub frames: u32,
    pub keys: u32,
    pub bytes: u64,
    pub skipped: u32,
    pub dropped: u32,
    pub enc_call_sum_us: u64,
    pub enc_call_max_us: u64,
    pub cap_to_enc_sum_us: u64,
    pub cap_to_enc_max_us: u64,
    pub lag_sum_ms: u64,
    pub lag_max_ms: u64,
    pub lag_n: u32,
    pub queue_max: u32,
    pub inflight_bytes_max: u64,
}

impl StatsWindow {
    pub fn lag(&mut self, lag_ms: u64, pending: u32, inflight_bytes: usize) {
        self.lag_sum_ms += lag_ms;
        self.lag_max_ms = self.lag_max_ms.max(lag_ms);
        self.lag_n += 1;
        self.queue_max = self.queue_max.max(pending);
        self.inflight_bytes_max = self.inflight_bytes_max.max(inflight_bytes as u64);
    }

    pub fn encoded(&mut self, key: bool, bytes: usize, call_us: u64, cap_to_enc_us: u64) {
        self.frames += 1;
        self.keys += key as u32;
        self.bytes += bytes as u64;
        self.enc_call_sum_us += call_us;
        self.enc_call_max_us = self.enc_call_max_us.max(call_us);
        self.cap_to_enc_sum_us += cap_to_enc_us;
        self.cap_to_enc_max_us = self.cap_to_enc_max_us.max(cap_to_enc_us);
    }

    /// Tek satırlık özet (`secs` = pencere uzunluğu).
    pub fn line(&self, secs: f64, target_kbps: u32, size: (u32, u32), q: Quality, level: usize) -> String {
        let s = secs.max(0.001);
        let n = self.frames.max(1) as f64;
        let lag_avg = if self.lag_n > 0 { self.lag_sum_ms as f64 / self.lag_n as f64 } else { 0.0 };
        format!(
            "[ekran-pc] yakala {:.1}/s | kodla {:.1}/s anahtar {} | yakala→kodlandı {:.1}/{:.1} ms (çağrı {:.1}/{:.1}) | {:.2} Mbps (hedef {:.2}) | teslim gecikmesi {:.0}/{} ms kuyruk {} uçuşta {} kB | atlanan {} atılan {} | {}x{}@{} L{}",
            self.grabs as f64 / s,
            self.frames as f64 / s,
            self.keys,
            self.cap_to_enc_sum_us as f64 / n / 1000.0,
            self.cap_to_enc_max_us as f64 / 1000.0,
            self.enc_call_sum_us as f64 / n / 1000.0,
            self.enc_call_max_us as f64 / 1000.0,
            self.bytes as f64 * 8.0 / s / 1e6,
            target_kbps as f64 / 1000.0,
            lag_avg,
            self.lag_max_ms,
            self.queue_max,
            self.inflight_bytes_max / 1000,
            self.skipped,
            self.dropped,
            size.0,
            size.1,
            q.fps,
            level,
        )
    }
}

/// Telefonun saniyelik alıcı istatistiklerinin tek satırlık özeti (PC günlüğü).
pub fn phone_line(r: &crate::types::ScreenStatsReport) -> String {
    let t = |v: u16| v as f64 / 10.0;
    format!(
        "[ekran-tel] alınan {:.1}/s çizilen {:.1}/s | çözme {:.1} ms alındı→çizildi {:.1} ms | uçtan uca {}/{} ms | geç atılan {} giriş beklemesi {} kuyruk {} | RTT {:.1} ms",
        t(r.rx_fps10),
        t(r.render_fps10),
        t(r.decode_ms10),
        t(r.recv_to_render_ms10),
        r.e2e_ms,
        r.e2e_max_ms,
        r.late_drops,
        r.input_waits,
        r.queue_max,
        t(r.rtt_ms10),
    )
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
        assert_eq!(g.offer(0, true, 100), Offer::Accept);
        for i in 1..=9u64 {
            assert_eq!(g.offer(i * 16_000, false, 10), Offer::Accept, "144 ms içinde kabul");
        }
        // 160 ms > 150 ms: ilk P atılır
        assert_eq!(g.offer(160_000, false, 10), Offer::Drop);
        assert!(g.take_want_key());
        assert!(!g.take_want_key(), "istek bir kez döner");
        // kuyruk boşalsa bile anahtar gelene kadar P atılır
        while g.pending() > 0 {
            g.done();
        }
        assert_eq!(g.offer(200_000, false, 10), Offer::Drop);
        assert_eq!(g.offer(220_000, true, 100), Offer::Accept);
        assert_eq!(g.offer(236_000, false, 10), Offer::Accept);
        assert!(g.dropped >= 2);
    }

    #[test]
    fn gate_keyframes_always_accepted_even_when_backlogged() {
        let mut g = Gate::new();
        g.offer(0, true, 1);
        assert_eq!(g.offer(1_000_000, true, 1), Offer::Accept);
    }

    #[test]
    fn gate_ack_mode_tracks_unacked_frames_after_write() {
        let mut g = Gate::with_ack(true);
        g.offer(1_000_000, true, 5000);
        g.offer(1_033_000, false, 800);
        g.done(); // ilk kare akışa yazıldı ama alıcı onaylamadı
        g.done();
        assert_eq!(g.pending(), 0);
        assert_eq!(g.inflight(), 2);
        assert_eq!(g.inflight_bytes(), 5800);
        // QUIC tamponu kuyruğu boşaltsa da gecikme görünür: şimdi = 1,2 sn
        assert_eq!(g.lag_ms(1_200_000), 200);
        g.ack(1_000_000);
        assert_eq!(g.inflight(), 1);
        assert_eq!(g.lag_ms(1_200_000), 167);
        g.ack(1_033_000);
        assert_eq!(g.lag_ms(1_200_000), 0);
        assert_eq!(g.inflight_bytes(), 0);
    }

    #[test]
    fn gate_without_ack_forgets_written_frames() {
        let mut g = Gate::new();
        g.offer(0, true, 10);
        g.done();
        assert_eq!(g.inflight(), 0);
        assert_eq!(g.lag_ms(500_000), 0);
    }

    #[test]
    fn gate_ack_mode_uses_larger_drop_limit_and_prunes_lost_acks() {
        let mut g = Gate::with_ack(true);
        g.offer(0, true, 10);
        // 300 ms birikim: ack modunda P hâlâ kabul edilir (önce kare üretimi durdurulur, atma değil)
        assert_eq!(g.offer(300_000, false, 10), Offer::Accept);
        assert_eq!(g.offer(460_000, false, 10), Offer::Drop);
        let mut g = Gate::with_ack(true);
        g.offer(0, true, 10);
        g.done();
        // ack hiç gelmedi: 3 sn sonra kayıt temizlenir
        g.offer(4_000_000, true, 10);
        assert_eq!(g.inflight(), 0);
    }

    #[test]
    fn clock_sync_offset_and_best_sample() {
        let mut c = ClockSync::default();
        assert_eq!(c.offset_us(), None);
        assert_eq!(c.age_ms(0, 0), None);
        // Uzak saat yerelden 5 sn ileri. Simetrik 10 ms gidiş-dönüş yarıları: kesin sonuç.
        let (t0, one_way) = (1_000_000u64, 5_000u64);
        let t1 = t0 + one_way + 5_000_000;
        assert!(c.sample(t0, t1, t0 + 2 * one_way));
        assert_eq!(c.offset_us(), Some(5_000_000));
        assert_eq!(c.rtt_us(), Some(10_000));
        // Kötü (asimetrik, yüksek RTT) örnek en iyiyi bozmaz
        assert!(c.sample(2_000_000, 2_000_000 + 90_000 + 5_000_000, 2_000_000 + 100_000));
        assert_eq!(c.offset_us(), Some(5_000_000));
        // Geçersiz: zaman geri / aşırı RTT
        assert!(!c.sample(10, 5, 4));
        assert!(!c.sample(0, 1, 3_000_000));
        // Yaş: pts uzak saatte 9,95 sn, yerelde şimdi 4,97 sn → uzakta 9,97 → 20 ms
        assert_eq!(c.age_ms(9_950_000, 4_970_000), Some(20));
    }

    #[test]
    fn clock_sync_error_bounded_by_half_rtt_for_asymmetric_path() {
        let mut c = ClockSync::default();
        // Gerçek fark 123 ms; gidiş 2 ms, dönüş 18 ms (asimetrik): hata ≤ rtt/2 = 10 ms
        let (t0, fwd, back, off) = (500_000u64, 2_000u64, 18_000u64, 123_000i64);
        let t1 = (t0 + fwd) as i64 + off;
        c.sample(t0, t1 as u64, t0 + fwd + back);
        let est = c.offset_us().unwrap();
        assert!((est - off).abs() <= 10_000, "hata {}", est - off);
        assert_eq!(c.samples.len(), 1);
        for i in 0..20u64 {
            c.sample(i * 1000, i * 1000 + 5000 + 123_000, i * 1000 + 10_000);
        }
        assert_eq!(c.samples.len(), SYNC_KEEP);
    }

    #[test]
    fn fit_size_keeps_aspect_even_and_never_upscales() {
        assert_eq!(fit_size(3840, 2160, 1920, 1080), (1920, 1080));
        assert_eq!(fit_size(2560, 1440, 1280, 720), (1280, 720));
        assert_eq!(fit_size(1024, 768, 1920, 1080), (1024, 768));
        let (w, h) = fit_size(1366, 768, 1280, 720);
        assert!(w % 2 == 0 && h % 2 == 0 && w <= 1280 && h <= 720);
    }

    #[test]
    fn choose_size_follows_phone_viewport() {
        // Yatay telefon 2400x1080 görüntü alanı, 1080p kaynak: kaynak boyutu (büyütme yok)
        assert_eq!(choose_size(1920, 1080, 2400, 1080, 100), (1920, 1080));
        // Dikey telefon 1080x2400: tüm yatay ekran genişliğe sığar (letterbox), 1080x606
        assert_eq!(choose_size(1920, 1080, 1080, 2400, 100), (1080, 606));
        // Gömülü küçük görüntüleyici (970x546): en-boya sığan çift boyut
        assert_eq!(choose_size(1920, 1080, 970, 546, 100), (970, 544));
        // 4K kaynak, 1440p yatay telefon: 3200x1440 alana sığar → 2560x1440
        assert_eq!(choose_size(3840, 2160, 3200, 1440, 100), (2560, 1440));
        // Dikey kaynak (döndürülmüş monitör) dikey telefonda yüksekliğe sığar
        assert_eq!(choose_size(1080, 1920, 1080, 2340, 100), (1080, 1920));
        // Kaynak küçükse asla büyütülmez
        assert_eq!(choose_size(1280, 720, 3000, 3000, 100), (1280, 720));
        // Uyarlanabilir çözünürlük kademesi
        assert_eq!(choose_size(1920, 1080, 1920, 1080, 75), (1440, 810));
        assert_eq!(choose_size(1920, 1080, 1920, 1080, 50), (960, 540));
        // Saçma girdiler çökmez, boyutlar çift
        let (w, h) = choose_size(0, 0, 0, 0, 0);
        assert!(w >= 2 && h >= 2 && w % 2 == 0 && h % 2 == 0);
        let (w, h) = choose_size(1366, 768, 1001, 577, 60);
        assert!(w % 2 == 0 && h % 2 == 0);
    }

    #[test]
    fn bitrate_table_scales_with_pixels_and_fps() {
        let b1080_30 = auto_bitrate_kbps(1920, 1080, 30);
        let b1080_60 = auto_bitrate_kbps(1920, 1080, 60);
        let b720_15 = auto_bitrate_kbps(1280, 720, 15);
        assert!((7500..=8800).contains(&b1080_30), "{b1080_30}");
        assert!(b1080_60 > b1080_30 && b1080_60 < b1080_30 * 2, "fps ile doğrusal değil: {b1080_60}");
        assert!(b720_15 < b1080_30 / 3 && b720_15 >= 1500, "{b720_15}");
        // Piksel başına bit hızı 1080p'de aç kalmamalı: >= 0.1 bit/piksel/kare
        assert!(b1080_30 as f64 * 1000.0 / (1920.0 * 1080.0 * 30.0) >= 0.1);
        // Alt/üst sınır
        assert_eq!(auto_bitrate_kbps(160, 90, 5), MIN_KBPS);
        assert_eq!(auto_bitrate_kbps(7680, 4320, 60), MAX_KBPS);
        // Telefon boyutu (dikey letterbox) 1080x606: 1080p'nin üçte biri kadar
        let small = auto_bitrate_kbps(1080, 606, 30);
        assert!(small < b1080_30 / 2 && small > b1080_30 / 4, "{small}");
    }

    #[test]
    fn target_bitrate_respects_explicit_request_and_pct() {
        assert_eq!(target_bitrate_kbps(0, 1920, 1080, 30, 100), auto_bitrate_kbps(1920, 1080, 30));
        assert_eq!(target_bitrate_kbps(5000, 1920, 1080, 30, 100), 5000);
        assert_eq!(target_bitrate_kbps(5000, 1920, 1080, 30, 50), 2500);
        assert_eq!(target_bitrate_kbps(999_999, 1920, 1080, 30, 100), MAX_KBPS);
        assert!(target_bitrate_kbps(1, 100, 100, 5, 10) >= 300);
    }

    #[test]
    fn ladder_degrades_fps_then_bitrate_then_resolution() {
        let l = ladder(60);
        assert_eq!(l[0], Quality { fps: 60, bitrate_pct: 100, res_pct: 100 });
        let fps: Vec<u32> = l.iter().map(|q| q.fps).collect();
        assert!(fps.windows(2).all(|w| w[0] >= w[1]), "fps azalır: {fps:?}");
        // fps kademeleri bitrate/res'ten önce
        let first_br = l.iter().position(|q| q.bitrate_pct < 100).unwrap();
        let first_res = l.iter().position(|q| q.res_pct < 100).unwrap();
        assert!(l[..first_br].iter().all(|q| q.bitrate_pct == 100 && q.res_pct == 100));
        assert!(first_br < first_res);
        assert!(l[first_br..].iter().all(|q| q.fps == 15));
        assert_eq!(l.last().unwrap().res_pct, 50);
        // hedef 15 fps: fps kademesi yok, doğrudan bit hızı
        let e = ladder(15);
        assert_eq!(e[0].fps, 15);
        assert_eq!(e[1], Quality { fps: 15, bitrate_pct: 75, res_pct: 100 });
        // hedef 30
        assert_eq!(ladder(30).iter().map(|q| q.fps).take(3).collect::<Vec<_>>(), vec![30, 20, 15]);
    }

    #[test]
    fn verdict_thresholds() {
        assert_eq!(verdict(10, 0, 0), Verdict::Healthy);
        assert_eq!(verdict(100, 0, 0), Verdict::Neutral);
        assert_eq!(verdict(200, 0, 0), Verdict::Congested);
        assert_eq!(verdict(10, 1, 0), Verdict::Congested);
        assert_eq!(verdict(10, 0, 2), Verdict::Congested);
    }

    #[test]
    fn adaptor_degrades_fast_recovers_slowly() {
        let mut a = Adaptor::new(60);
        assert_eq!(a.level(), 0);
        // İlk bozulma hemen karşılanır; sonrakiler en az 1 sn arayla kademe kademe iner
        assert!(a.tick(Verdict::Congested).is_some());
        let mut changes = 1;
        for _ in 0..40 {
            if a.tick(Verdict::Congested).is_some() {
                changes += 1;
            }
        }
        assert_eq!(a.level(), a.levels() - 1, "taban");
        assert_eq!(changes, a.levels() - 1);
        // Toparlanma: 8 sağlıklı tik = 4 sn başına tek kademe
        for _ in 0..7 {
            assert!(a.tick(Verdict::Healthy).is_none());
        }
        assert!(a.tick(Verdict::Healthy).is_some());
        let mut ups = 1;
        assert_eq!(a.level(), a.levels() - 2);
        for _ in 0..400 {
            if a.tick(Verdict::Healthy).is_some() {
                ups += 1;
            }
        }
        assert_eq!(a.level(), 0);
        assert_eq!(ups, a.levels() - 1);
    }

    #[test]
    fn adaptor_neutral_and_dwell_rules() {
        let mut a = Adaptor::new(30);
        assert!(a.tick(Verdict::Congested).is_some());
        // 1 sn bekleme süresi: hemen ardından gelen bozulma yeni kademe açmaz
        assert!(a.tick(Verdict::Congested).is_none());
        assert!(a.tick(Verdict::Congested).is_some());
        // Nötr sinyal sayacı ilerletmez/sıfırlamaz
        let lvl = a.level();
        for _ in 0..50 {
            assert!(a.tick(Verdict::Neutral).is_none());
        }
        assert_eq!(a.level(), lvl);
        // Tam kalitede sağlıklı sinyal bir şey yapmaz
        let mut b = Adaptor::new(30);
        for _ in 0..100 {
            assert!(b.tick(Verdict::Healthy).is_none());
        }
        assert_eq!(b.level(), 0);
    }

    #[test]
    fn adaptor_flapping_doubles_recovery_wait() {
        let mut a = Adaptor::new(30);
        a.tick(Verdict::Congested);
        a.tick(Verdict::Congested);
        a.tick(Verdict::Congested);
        // 8 sağlıklı tik → yukarı
        let mut up_at = None;
        for i in 0..8 {
            if a.tick(Verdict::Healthy).is_some() {
                up_at = Some(i);
            }
        }
        assert_eq!(up_at, Some(7));
        // Hemen yeniden bozulma (çırpınma) → bir sonraki yukarı çıkış 16 sağlıklı tik ister
        let before = a.level();
        let mut down = false;
        for _ in 0..2 {
            down |= a.tick(Verdict::Congested).is_some();
        }
        assert!(down && a.level() > before);
        let mut n = 0;
        while a.tick(Verdict::Healthy).is_none() {
            n += 1;
            assert!(n < 100);
        }
        assert_eq!(n + 1, 16);
    }

    #[test]
    fn phone_line_formats_tenths() {
        let r = crate::types::ScreenStatsReport { rx_fps10: 298, render_fps10: 295, decode_ms10: 61, e2e_ms: 58, e2e_max_ms: 120, rtt_ms10: 42, ..Default::default() };
        let l = phone_line(&r);
        assert!(l.contains("alınan 29.8/s") && l.contains("çözme 6.1 ms") && l.contains("58/120 ms") && l.contains("RTT 4.2 ms"), "{l}");
    }

    #[test]
    fn stats_window_line_is_readable() {
        let mut w = StatsWindow { grabs: 30, ..Default::default() };
        for _ in 0..30 {
            w.encoded(false, 20_000, 3_000, 5_000);
            w.lag(40, 1, 25_000);
        }
        w.encoded(true, 90_000, 9_000, 12_000);
        w.skipped = 2;
        let l = w.line(1.0, 8000, (1920, 1080), Quality { fps: 30, bitrate_pct: 100, res_pct: 100 }, 0);
        assert!(l.starts_with("[ekran-pc]"));
        assert!(l.contains("1920x1080@30 L0"), "{l}");
        assert!(l.contains("atlanan 2"), "{l}");
        assert!(l.contains("kodla 31.0/s"), "{l}");
    }
}
