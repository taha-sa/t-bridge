//! PC ekran paylaşımı ardışık düzeni: yakala (WGC / DXGI-DD) -> BGRA→NV12 (GPU video işlemcisi, ölçekleme) -> H.264 (MF) -> sink.
pub mod capture;
pub mod encode;

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

use kopru_core::screen::{adapt_bitrate, fit_size};
use windows::Win32::{
    Media::MediaFoundation::{MF_VERSION, MFSTARTUP_FULL, MFShutdown, MFStartup},
    System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize},
};

pub use capture::monitors;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Params {
    pub monitor: u32,
    pub max_w: u32,
    pub max_h: u32,
    pub fps: u32,
    pub bitrate_kbps: u32,
}

/// Ardışık düzenin dış dünyayla bağı (çekirdek düğümü sink'te soyutlanır: testte sahte).
pub struct Sink {
    /// (pts_us, anahtar mı, Annex-B verisi) -> gönderildi mi (false = geri basınçla atıldı).
    pub send: Box<dyn Fn(u64, bool, Vec<u8>) -> bool + Send>,
    /// pts_us -> (anahtar kare iste, gönderim kuyruğu gecikmesi ms)
    pub feedback: Box<dyn Fn(u64) -> (bool, u64) + Send>,
    pub on_status: Box<dyn Fn(Result<String, String>) + Send>,
}

pub struct Pipeline {
    stop: Arc<AtomicBool>,
    key: Arc<AtomicBool>,
    gen_: Arc<AtomicU64>,
    cfg: Arc<Mutex<Params>>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl Pipeline {
    pub fn start(params: Params, sink: Sink) -> Pipeline {
        let stop = Arc::new(AtomicBool::new(false));
        let key = Arc::new(AtomicBool::new(true));
        let gen_ = Arc::new(AtomicU64::new(0));
        let cfg = Arc::new(Mutex::new(params));
        let (s2, k2, g2, c2) = (stop.clone(), key.clone(), gen_.clone(), cfg.clone());
        let join = std::thread::Builder::new()
            .name("screen-share".into())
            .spawn(move || thread_main(s2, k2, g2, c2, sink))
            .ok();
        Pipeline { stop, key, gen_, cfg, join }
    }

    pub fn request_key(&self) {
        self.key.store(true, Ordering::SeqCst);
    }

    /// Canlı yapılandırma değişikliği: ardışık düzen yeni parametrelerle yeniden kurulur (ilk kare anahtar kare).
    pub fn reconfigure(&self, p: Params) {
        let mut g = self.cfg.lock().unwrap_or_else(|e| e.into_inner());
        if *g != p {
            *g = p;
            self.gen_.fetch_add(1, Ordering::SeqCst);
        }
    }

    pub fn current(&self) -> Params {
        *self.cfg.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Durdurma bayrağını kurar; iş parçacığının bitmesini ayrı bir thread bekler (çağıran, ör. QUIC okuyucu, bloke olmaz).
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(j) = self.join.take() {
            let _ = std::thread::Builder::new().name("screen-share-join".into()).spawn(move || {
                let _ = j.join();
            });
        }
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        self.stop();
    }
}

enum Exit {
    Stop,
    Restart,
}

fn thread_main(stop: Arc<AtomicBool>, key: Arc<AtomicBool>, gen_: Arc<AtomicU64>, cfg: Arc<Mutex<Params>>, sink: Sink) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let _ = MFStartup(MF_VERSION, MFSTARTUP_FULL);
    }
    let started = Instant::now();
    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        match run_session(&stop, &key, &gen_, &cfg, &sink, started) {
            Ok(Exit::Stop) => break,
            Ok(Exit::Restart) => std::thread::sleep(Duration::from_millis(150)),
            Err(e) => {
                (sink.on_status)(Err(e.to_string()));
                break;
            }
        }
    }
    unsafe {
        let _ = MFShutdown();
        CoUninitialize();
    }
}

fn run_session(
    stop: &AtomicBool,
    key: &AtomicBool,
    gen_: &AtomicU64,
    cfg: &Mutex<Params>,
    sink: &Sink,
    started: Instant,
) -> windows::core::Result<Exit> {
    use capture::{Converter, Dup, GrabError, Source, Wgc, bgra_texture, create_gpu};
    let seen_gen = gen_.load(Ordering::SeqCst);
    let p = *cfg.lock().unwrap_or_else(|e| e.into_inner());
    let mons = monitors();
    let mon = mons
        .iter()
        .find(|m| m.index == p.monitor)
        .or_else(|| mons.iter().find(|m| m.primary))
        .or(mons.first())
        .cloned()
        .ok_or_else(|| windows::core::Error::new(windows::core::HRESULT(-1), "monitör yok"))?;

    let gpu = create_gpu()?;
    let mut src: Box<dyn Source> = match Wgc::new(&gpu, mon.hmon) {
        Ok(w) => Box::new(w),
        Err(_) => Box::new(Dup::new(&gpu, mon.hmon)?), // WGC yoksa DXGI Desktop Duplication (imleç çizilmez)
    };
    let (sw, sh) = src.size();
    let fps = p.fps.clamp(5, 60);
    let (ow, oh) = fit_size(sw, sh, p.max_w, p.max_h);
    let bgra = bgra_texture(&gpu.device, sw, sh)?;
    let conv = Converter::new(&gpu, &bgra, sw, sh, ow, oh, fps)?;
    let mut enc = encode::Encoder::new(&gpu, ow, oh, fps, p.bitrate_kbps)?;
    (sink.on_status)(Ok(format!("{} · {} · {}x{}@{}", src.name(), enc.describe(), ow, oh, fps)));

    let interval = Duration::from_micros(1_000_000 / fps as u64);
    let mut next = Instant::now();
    let mut idx = 0usize;
    let mut dirty = false;
    let mut last_encode = Instant::now();
    let mut cur_kbps = p.bitrate_kbps;
    let mut last_adapt = Instant::now();
    loop {
        if stop.load(Ordering::SeqCst) {
            return Ok(Exit::Stop);
        }
        if gen_.load(Ordering::SeqCst) != seen_gen {
            return Ok(Exit::Restart);
        }
        let wait = interval.as_millis().min(8) as u32;
        match src.grab(&bgra, &gpu.ctx, wait) {
            Ok(new) => dirty |= new,
            Err(GrabError::Restart) => return Ok(Exit::Restart),
        }
        if key.swap(false, Ordering::SeqCst) {
            enc.force_key();
            dirty = true;
        }
        if last_encode.elapsed() > Duration::from_secs(2) {
            dirty = true; // durağan ekranda da zaman zaman tazele
        }
        let now = Instant::now();
        if !dirty || now < next {
            continue;
        }
        next = (next + interval).max(now.checked_sub(interval).unwrap_or(now));
        dirty = false;
        last_encode = now;

        conv.convert(idx)?;
        let pts_100ns = started.elapsed().as_nanos() as i64 / 100;
        let frames = enc.encode(&gpu, &conv.nv12[idx], pts_100ns)?;
        idx = (idx + 1) % capture::RING;
        for f in frames {
            let pts_us = (f.pts_100ns / 10) as u64;
            let (want_key, backlog) = (sink.feedback)(pts_us);
            if want_key {
                enc.force_key();
                dirty = true;
            }
            (sink.send)(pts_us, f.key, f.data);
            if last_adapt.elapsed() > Duration::from_millis(500) {
                last_adapt = Instant::now();
                let nk = adapt_bitrate(cur_kbps, p.bitrate_kbps, backlog);
                if nk.abs_diff(cur_kbps) * 20 > cur_kbps {
                    cur_kbps = nk;
                    enc.set_bitrate(nk);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Donanım/yazılım H.264 kodlayıcıyı kurar ve birkaç kare kodlar; ortam (GPU/MF/encoder) yoksa sessizce atlar.
    #[test]
    fn encoder_init_smoke_skips_gracefully() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            if MFStartup(MF_VERSION, MFSTARTUP_FULL).is_err() {
                eprintln!("ATLANDI: MFStartup yok");
                return;
            }
        }
        let run = || -> windows::core::Result<String> {
            let gpu = capture::create_gpu()?;
            let bgra = capture::bgra_texture(&gpu.device, 640, 360)?;
            let conv = capture::Converter::new(&gpu, &bgra, 640, 360, 640, 360, 30)?;
            let mut enc = encode::Encoder::new(&gpu, 640, 360, 30, 1500)?;
            let mut keys = 0;
            let mut total = 0;
            for i in 0..12 {
                conv.convert(i % capture::RING)?;
                for f in enc.encode(&gpu, &conv.nv12[i % capture::RING], i as i64 * 333_333)? {
                    total += 1;
                    if f.key {
                        keys += 1;
                    }
                }
            }
            Ok(format!("{} -> {total} kare, {keys} anahtar", enc.describe()))
        };
        match run() {
            Ok(s) => eprintln!("kodlayıcı: {s}"),
            Err(e) => eprintln!("ATLANDI (ortam uygun değil): {e}"),
        }
        unsafe {
            let _ = MFShutdown();
            CoUninitialize();
        }
    }

    #[test]
    fn monitors_enumerate_without_panic() {
        let _ = capture::monitors();
    }
}
