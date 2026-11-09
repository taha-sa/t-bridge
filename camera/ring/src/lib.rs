//! NV12 kare halkası. Bellek düzeni (küçük-endian, 8 bayt hizalı tabanda):
//!
//! ```text
//! başlık (256 B): magic u32 | version u32 | latest_seq u64 | want_w u32 | want_h u32 | consumer_ms u64 | producer_ms u64
//! yuva[SLOTS]   : seq_begin u64 | seq_end u64 | w u32 | h u32 | pts_us u64 | len u32 | pad | NV12 verisi (MAX_FRAME)
//! ```
//! Yazıcı (uygulama) ve okuyucu (Frame Server kaynağı) farklı süreçlerdir; kilit yok: yuva başına seqlock
//! (begin/end sıra numarası) ile yırtık okuma algılanır. `*_ms` alanları sistem genelinde ortak bir milisaniye saatidir
//! (Windows: GetTickCount64) ve bayatlık/canlılık tespiti içindir.
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering, fence};

pub const MAGIC: u32 = 0x4D41_4354; // "TCAM"
pub const VERSION: u32 = 1;
pub const SLOTS: usize = 3;
pub const MAX_W: u32 = 1920;
pub const MAX_H: u32 = 1080;
pub const MAX_FRAME: usize = (MAX_W * MAX_H * 3 / 2) as usize;
pub const HEADER_SIZE: usize = 256;
pub const SLOT_HEADER: usize = 64;
pub const SLOT_SIZE: usize = (SLOT_HEADER + MAX_FRAME + 63) & !63;
pub const TOTAL_SIZE: usize = HEADER_SIZE + SLOTS * SLOT_SIZE;
/// Bu süredir (ms) taze kare yoksa okuyucu yer tutucu kare üretir.
pub const STALE_MS: u64 = 1000;

const OFF_MAGIC: usize = 0;
const OFF_VERSION: usize = 4;
const OFF_LATEST: usize = 8;
const OFF_WANT_W: usize = 16;
const OFF_WANT_H: usize = 20;
const OFF_CONSUMER_MS: usize = 24;
const OFF_PRODUCER_MS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameInfo {
    pub seq: u64,
    pub w: u32,
    pub h: u32,
    pub pts_us: u64,
    pub write_ms: u64,
}

/// Halka görünümü. `base`, en az `TOTAL_SIZE` baytlık, 8 bayt hizalı ve Ring'in ömrü boyunca geçerli bellektir.
#[derive(Clone, Copy)]
pub struct Ring {
    base: *mut u8,
}

// Bellek paylaşımlı eşzamanlı erişim için tasarlandı (atomikler + seqlock).
unsafe impl Send for Ring {}
unsafe impl Sync for Ring {}

impl Ring {
    /// # Safety
    /// `base` en az `TOTAL_SIZE` bayt, 8 bayt hizalı ve Ring'in ömrü boyunca geçerli olmalı.
    pub unsafe fn from_raw(base: *mut u8) -> Self {
        Self { base }
    }

    fn a64(&self, off: usize) -> &AtomicU64 {
        unsafe { &*(self.base.add(off) as *const AtomicU64) }
    }
    fn a32(&self, off: usize) -> &AtomicU32 {
        unsafe { &*(self.base.add(off) as *const AtomicU32) }
    }
    fn slot(&self, i: usize) -> *mut u8 {
        unsafe { self.base.add(HEADER_SIZE + i * SLOT_SIZE) }
    }

    /// Yaratıcı (kaynak/okuyucu tarafı) başlığı sıfırlar ve damgalar.
    pub fn init(&self) {
        unsafe { std::ptr::write_bytes(self.base, 0, HEADER_SIZE) };
        self.a32(OFF_VERSION).store(VERSION, Ordering::Relaxed);
        self.a32(OFF_MAGIC).store(MAGIC, Ordering::Release);
    }

    pub fn is_valid(&self) -> bool {
        self.a32(OFF_MAGIC).load(Ordering::Acquire) == MAGIC && self.a32(OFF_VERSION).load(Ordering::Relaxed) == VERSION
    }

    // ---- okuyucu -> yazıcı geri bildirimi ----
    pub fn set_want(&self, w: u32, h: u32) {
        self.a32(OFF_WANT_W).store(w, Ordering::Relaxed);
        self.a32(OFF_WANT_H).store(h, Ordering::Relaxed);
    }
    pub fn want(&self) -> (u32, u32) {
        (self.a32(OFF_WANT_W).load(Ordering::Relaxed), self.a32(OFF_WANT_H).load(Ordering::Relaxed))
    }
    pub fn touch_consumer(&self, now_ms: u64) {
        self.a64(OFF_CONSUMER_MS).store(now_ms, Ordering::Relaxed);
    }
    pub fn consumer_ms(&self) -> u64 {
        self.a64(OFF_CONSUMER_MS).load(Ordering::Relaxed)
    }

    // ---- yazıcı ----
    /// NV12 kareyi (`w*h*3/2` bayt) yazar. Başarısızsa false (boyut sınırı/uyuşmazlık).
    pub fn write(&self, w: u32, h: u32, pts_us: u64, now_ms: u64, nv12: &[u8]) -> bool {
        let need = (w as usize) * (h as usize) * 3 / 2;
        if w == 0 || h == 0 || w > MAX_W || h > MAX_H || nv12.len() != need {
            return false;
        }
        let seq = self.a64(OFF_LATEST).load(Ordering::Relaxed) + 1;
        let s = self.slot((seq as usize) % SLOTS);
        unsafe {
            (*(s as *const AtomicU64)).store(seq, Ordering::Relaxed); // seq_begin
            fence(Ordering::Release);
            (*(s.add(16) as *const AtomicU32)).store(w, Ordering::Relaxed);
            (*(s.add(20) as *const AtomicU32)).store(h, Ordering::Relaxed);
            (*(s.add(24) as *const AtomicU64)).store(pts_us, Ordering::Relaxed);
            (*(s.add(32) as *const AtomicU32)).store(need as u32, Ordering::Relaxed);
            std::ptr::copy_nonoverlapping(nv12.as_ptr(), s.add(SLOT_HEADER), need);
            fence(Ordering::Release);
            (*(s.add(8) as *const AtomicU64)).store(seq, Ordering::Release); // seq_end
        }
        self.a64(OFF_PRODUCER_MS).store(now_ms, Ordering::Relaxed);
        self.a64(OFF_LATEST).store(seq, Ordering::Release);
        true
    }

    pub fn producer_ms(&self) -> u64 {
        self.a64(OFF_PRODUCER_MS).load(Ordering::Relaxed)
    }

    /// Yazıcı bu süredir (ms) kare yazmadıysa (veya hiç yazmadıysa) bayat.
    pub fn is_stale(&self, now_ms: u64, timeout_ms: u64) -> bool {
        let p = self.producer_ms();
        p == 0 || now_ms.saturating_sub(p) > timeout_ms
    }

    // ---- okuyucu ----
    /// `last_seq`'ten yeni bir kare varsa tutarlı bir kopya alır (`out` yeniden boyutlandırılır).
    pub fn read_latest(&self, last_seq: u64, out: &mut Vec<u8>) -> Option<FrameInfo> {
        let seq = self.a64(OFF_LATEST).load(Ordering::Acquire);
        if seq == 0 || seq == last_seq {
            return None;
        }
        let s = self.slot((seq as usize) % SLOTS);
        unsafe {
            let end = (*(s.add(8) as *const AtomicU64)).load(Ordering::Acquire);
            if end != seq {
                return None; // yazıcı bu yuvayı yeniden yazıyor
            }
            let w = (*(s.add(16) as *const AtomicU32)).load(Ordering::Relaxed);
            let h = (*(s.add(20) as *const AtomicU32)).load(Ordering::Relaxed);
            let pts_us = (*(s.add(24) as *const AtomicU64)).load(Ordering::Relaxed);
            let len = (*(s.add(32) as *const AtomicU32)).load(Ordering::Relaxed) as usize;
            if len == 0 || len > MAX_FRAME || len != (w as usize) * (h as usize) * 3 / 2 {
                return None;
            }
            out.resize(len, 0);
            std::ptr::copy_nonoverlapping(s.add(SLOT_HEADER), out.as_mut_ptr(), len);
            fence(Ordering::Acquire);
            let begin = (*(s as *const AtomicU64)).load(Ordering::Relaxed);
            if begin != seq {
                return None; // kopyalarken ezildi: yırtık okuma, at
            }
            Some(FrameInfo { seq, w, h, pts_us, write_ms: self.producer_ms() })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 8 bayt hizalı, TOTAL_SIZE'lık sahte "paylaşımlı" bellek.
    struct Mem(Vec<u64>);
    impl Mem {
        fn new() -> Self {
            Mem(vec![0u64; TOTAL_SIZE.div_ceil(8)])
        }
        fn ring(&mut self) -> Ring {
            unsafe { Ring::from_raw(self.0.as_mut_ptr() as *mut u8) }
        }
    }

    fn frame(w: u32, h: u32, fill: u8) -> Vec<u8> {
        vec![fill; (w * h * 3 / 2) as usize]
    }

    #[test]
    fn layout_is_sane() {
        assert_eq!(SLOT_SIZE % 64, 0);
        assert!(TOTAL_SIZE > 3 * MAX_FRAME);
        assert!(TOTAL_SIZE < 16 * 1024 * 1024);
    }

    #[test]
    fn writer_reader_roundtrip() {
        let mut m = Mem::new();
        let r = m.ring();
        r.init();
        assert!(r.is_valid());
        let mut out = Vec::new();
        assert!(r.read_latest(0, &mut out).is_none(), "henüz kare yok");
        assert!(r.write(1280, 720, 123, 1000, &frame(1280, 720, 7)));
        let info = r.read_latest(0, &mut out).unwrap();
        assert_eq!((info.seq, info.w, info.h, info.pts_us), (1, 1280, 720, 123));
        assert!(out.iter().all(|b| *b == 7));
        assert!(r.read_latest(info.seq, &mut out).is_none(), "aynı kare ikinci kez verilmez");
    }

    #[test]
    fn wraps_around_slots_and_returns_latest() {
        let mut m = Mem::new();
        let r = m.ring();
        r.init();
        let mut out = Vec::new();
        let mut last = 0;
        for i in 1..=10u8 {
            assert!(r.write(640, 360, i as u64, 10 * i as u64, &frame(640, 360, i)));
            if i % 4 == 0 {
                let info = r.read_latest(last, &mut out).unwrap();
                assert_eq!(info.seq, i as u64);
                assert!(out.iter().all(|b| *b == i));
                last = info.seq;
            }
        }
        // okuyucu geride kalırsa yalnızca en yeniyi görür (ara kareler atlanır)
        assert_eq!(r.read_latest(last, &mut out).unwrap().seq, 10);
    }

    #[test]
    fn resolution_change_between_frames() {
        let mut m = Mem::new();
        let r = m.ring();
        r.init();
        let mut out = Vec::new();
        r.write(1280, 720, 1, 1, &frame(1280, 720, 1));
        r.write(1920, 1080, 2, 2, &frame(1920, 1080, 2));
        let info = r.read_latest(0, &mut out).unwrap();
        assert_eq!((info.w, info.h), (1920, 1080));
        assert_eq!(out.len(), 1920 * 1080 * 3 / 2);
    }

    #[test]
    fn rejects_bad_sizes() {
        let mut m = Mem::new();
        let r = m.ring();
        r.init();
        assert!(!r.write(1280, 720, 0, 0, &frame(640, 360, 0)), "uzunluk uyuşmazlığı");
        assert!(!r.write(4096, 2160, 0, 0, &[]), "sınır üstü");
        assert!(!r.write(0, 0, 0, 0, &[]));
    }

    #[test]
    fn stale_detection_and_liveness() {
        let mut m = Mem::new();
        let r = m.ring();
        r.init();
        assert!(r.is_stale(5000, STALE_MS), "hiç yazılmadı = bayat");
        r.write(320, 180, 0, 1000, &frame(320, 180, 0));
        assert!(!r.is_stale(1500, STALE_MS));
        assert!(r.is_stale(2001, STALE_MS));
        r.set_want(1920, 1080);
        assert_eq!(r.want(), (1920, 1080));
        r.touch_consumer(42);
        assert_eq!(r.consumer_ms(), 42);
    }

    #[test]
    fn torn_read_is_rejected() {
        let mut m = Mem::new();
        let r = m.ring();
        r.init();
        r.write(320, 180, 0, 1, &frame(320, 180, 9));
        // yuvanın seq_end'ini boz: okuyucu reddetmeli
        unsafe { *(r.slot(1) as *mut u64).add(1) = 99 };
        let mut out = Vec::new();
        assert!(r.read_latest(0, &mut out).is_none());
    }

    #[test]
    fn concurrent_writer_reader_never_returns_mixed_frames() {
        use std::sync::{Arc, atomic::AtomicBool};
        let mut m = Mem::new();
        let r = m.ring();
        r.init();
        let stop = Arc::new(AtomicBool::new(false));
        let st = stop.clone();
        let t = std::thread::spawn(move || {
            let mut i = 0u8;
            while !st.load(Ordering::Relaxed) {
                i = i.wrapping_add(1);
                r.write(320, 180, i as u64, 1, &frame(320, 180, i));
            }
        });
        let mut out = Vec::new();
        let mut last = 0;
        let end = std::time::Instant::now() + std::time::Duration::from_millis(300);
        let mut got = 0;
        while std::time::Instant::now() < end {
            if let Some(info) = r.read_latest(last, &mut out) {
                last = info.seq;
                got += 1;
                let first = out[0];
                assert!(out.iter().all(|b| *b == first), "yırtık/karışık kare okundu");
            }
        }
        stop.store(true, Ordering::Relaxed);
        t.join().unwrap();
        assert!(got > 0);
    }
}
