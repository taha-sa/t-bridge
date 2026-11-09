//! Güvenli masaüstü (UAC istemi, Ctrl+Alt+Del, kilit ekranı) algılama.
//! Güvenli masaüstü açıkken yakalama kare üretmez; telefona "geçici durdu" bildirimi gider, kareler dönünce kalkar.
//! Mantık saf (`SecureWatch`): zaman ve masaüstü sınaması dışarıdan verilir, birim testlidir.
use std::time::{Duration, Instant};

use windows::Win32::{
    Foundation::HANDLE,
    System::StationsAndDesktops::{
        CloseDesktop, DESKTOP_ACCESS_FLAGS, DESKTOP_CONTROL_FLAGS, GetUserObjectInformationW, OpenInputDesktop, UOI_NAME,
    },
};

/// Güvenli masaüstü sayılması için kare gelmeme süresi (en az).
pub const QUIET_BEFORE_SECURE: Duration = Duration::from_secs(2);
/// Masaüstü sınama aralığı.
pub const PROBE_EVERY: Duration = Duration::from_millis(250);

/// Giriş masaüstünün sınama sonucu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desktop {
    /// Normal kullanıcı masaüstü ("Default").
    Default,
    /// Başka bir masaüstü (ör. "Winlogon", "Screen-saver").
    Other,
    /// Açılamadı (güvenli masaüstü SYSTEM'e aittir: erişim reddedilir).
    Denied,
}

/// Masaüstü adından sınıf (ad okunamadıysa `None` -> yanlış alarm olmasın diye Default).
pub fn classify(name: Option<&str>) -> Desktop {
    match name {
        Some(n) if !n.eq_ignore_ascii_case("default") => Desktop::Other,
        _ => Desktop::Default,
    }
}

/// Giriş masaüstünü sınar (hafif; ~250 ms'de bir çağrılır).
pub fn probe_desktop() -> Desktop {
    unsafe {
        let Ok(h) = OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_ACCESS_FLAGS(0x0001)) else {
            return Desktop::Denied; // DESKTOP_READOBJECTS reddedildi
        };
        let mut buf = [0u16; 128];
        let mut need = 0u32;
        let r = GetUserObjectInformationW(
            HANDLE(h.0),
            UOI_NAME,
            Some(buf.as_mut_ptr().cast()),
            std::mem::size_of_val(&buf) as u32,
            Some(&mut need),
        );
        let _ = CloseDesktop(h);
        if r.is_err() {
            return classify(None);
        }
        let n = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
        classify(Some(&String::from_utf16_lossy(&buf[..n])))
    }
}

/// Güvenli masaüstü durum makinesi.
#[derive(Debug)]
pub struct SecureWatch {
    secure: bool,
    last_frame: Instant,
    last_probe: Desktop,
}

impl SecureWatch {
    pub fn new(now: Instant) -> Self {
        Self { secure: false, last_frame: now, last_probe: Desktop::Default }
    }

    pub fn is_secure(&self) -> bool {
        self.secure
    }

    /// Yeni bir kare yakalandı. Durum değiştiyse yeni değeri döndürür.
    pub fn frame(&mut self, now: Instant) -> Option<bool> {
        self.last_frame = now;
        // Kare, normal masaüstü sınamasıyla birlikte geliyorsa güvenli durum bitmiştir.
        if self.secure && self.last_probe == Desktop::Default {
            self.secure = false;
            return Some(false);
        }
        None
    }

    /// Masaüstü sınaması. Durum değiştiyse yeni değeri döndürür.
    /// Güvenli sayılma: giriş masaüstü Default değil VE en az `QUIET_BEFORE_SECURE` kare gelmedi.
    /// Biter: giriş masaüstü Default'a döndü (kare gelmesi beklenmez; durağan ekranda gelmeyebilir).
    pub fn probe(&mut self, now: Instant, d: Desktop) -> Option<bool> {
        self.last_probe = d;
        match d {
            Desktop::Default if self.secure => {
                self.secure = false;
                Some(false)
            }
            Desktop::Other | Desktop::Denied if !self.secure && now.saturating_duration_since(self.last_frame) >= QUIET_BEFORE_SECURE => {
                self.secure = true;
                Some(true)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(t0: Instant, ms: u64) -> Instant {
        t0 + Duration::from_millis(ms)
    }

    #[test]
    fn classify_names() {
        assert_eq!(classify(Some("Default")), Desktop::Default);
        assert_eq!(classify(Some("default")), Desktop::Default);
        assert_eq!(classify(Some("Winlogon")), Desktop::Other);
        assert_eq!(classify(None), Desktop::Default);
    }

    #[test]
    fn needs_quiet_period_before_secure() {
        let t0 = Instant::now();
        let mut w = SecureWatch::new(t0);
        assert_eq!(w.probe(at(t0, 500), Desktop::Denied), None); // erken: geçici geçiş olabilir
        assert!(!w.is_secure());
        assert_eq!(w.probe(at(t0, 2000), Desktop::Denied), Some(true));
        assert!(w.is_secure());
        assert_eq!(w.probe(at(t0, 2250), Desktop::Denied), None); // tekrar bildirilmez
    }

    #[test]
    fn idle_static_screen_is_not_secure() {
        // Kare gelmiyor ama masaüstü Default: durağan ekran, güvenli değil.
        let t0 = Instant::now();
        let mut w = SecureWatch::new(t0);
        assert_eq!(w.probe(at(t0, 30_000), Desktop::Default), None);
        assert!(!w.is_secure());
    }

    #[test]
    fn recent_frame_delays_secure() {
        let t0 = Instant::now();
        let mut w = SecureWatch::new(t0);
        assert_eq!(w.frame(at(t0, 1500)), None);
        assert_eq!(w.probe(at(t0, 3000), Desktop::Other), None); // son kare 1.5 sn önce
        assert_eq!(w.probe(at(t0, 3600), Desktop::Other), Some(true));
    }

    #[test]
    fn clears_when_default_returns_or_frames_resume() {
        let t0 = Instant::now();
        let mut w = SecureWatch::new(t0);
        assert_eq!(w.probe(at(t0, 2500), Desktop::Denied), Some(true));
        assert_eq!(w.probe(at(t0, 3000), Desktop::Default), Some(false));
        assert!(!w.is_secure());
        // İkinci tur: kare tek başına bitirmez; kare + Default sınaması birlikte bitirir.
        assert_eq!(w.probe(at(t0, 9000), Desktop::Other), Some(true));
        assert_eq!(w.frame(at(t0, 9100)), None);
        assert!(w.is_secure());
        w.last_probe = Desktop::Default;
        assert_eq!(w.frame(at(t0, 9200)), Some(false));
    }
}
