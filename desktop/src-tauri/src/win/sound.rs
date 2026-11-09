//! Bildirim sesi: exe'ye gömülü kısa WAV'ı Win32 ile çalar (pencere/WebView gerekmez).
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use windows::{
    Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT},
    core::PCWSTR,
};

/// `SND_ASYNC` süresince bellek geçerli kalmalı: 'static gömülü veri.
static NOTIFY_WAV: &[u8] = include_bytes!("../../assets/notify.wav");

/// Art arda bildirimlerde ses seli olmasın.
const MIN_GAP: Duration = Duration::from_millis(1500);

static LAST: Mutex<Option<Instant>> = Mutex::new(None);

/// Sesi hemen çalar (engellemez; önceki çalan ses kesilir).
pub fn play() {
    // SAFETY: SND_MEMORY ile ilk argüman WAV baytlarının adresidir; veri 'static olduğundan
    // asenkron çalma boyunca geçerli kalır.
    unsafe {
        let _ = PlaySoundW(PCWSTR(NOTIFY_WAV.as_ptr().cast()), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
    }
}

/// Son sesten bu yana yeterli süre geçtiyse çalar.
pub fn play_throttled() {
    {
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        if last.is_some_and(|t| now.duration_since(t) < MIN_GAP) {
            return;
        }
        *last = Some(now);
    }
    play();
}
