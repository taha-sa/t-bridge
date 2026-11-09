//! PC ekran paylaşımı ardışık düzeni: yakala (WGC / DXGI-DD) -> BGRA→NV12 (GPU video işlemcisi, ölçekleme) -> H.264 (MF) -> sink.
pub mod capture;
pub mod encode;
pub mod secure;

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

use kopru_core::ScreenFeedbackEx;
use kopru_core::ScreenStatsReport;
use kopru_core::screen::{Adaptor, Quality, SKIP_LAG_MS, StatsWindow, choose_size, now_us, phone_line, target_bitrate_kbps, verdict};
use windows::Win32::{
    Foundation::HANDLE,
    Graphics::Direct3D11::ID3D11Texture2D,
    Media::{MediaFoundation::{MF_VERSION, MFSTARTUP_FULL, MFShutdown, MFStartup}, timeBeginPeriod, timeEndPeriod},
    System::{
        Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize},
        Threading::{
            AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, GetCurrentProcess, GetCurrentThread,
            PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,
            PROCESS_POWER_THROTTLING_STATE, ProcessPowerThrottling, SetProcessInformation, SetThreadPriority, THREAD_PRIORITY_HIGHEST,
        },
    },
};

pub use capture::monitors;
use capture::Dup;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Params {
    pub monitor: u32,
    /// Telefonun bildirdiği görüntü alanı (piksel). Kodlama boyutu buna sığar (en-boy korunur, büyütme yok).
    pub max_w: u32,
    pub max_h: u32,
    /// Hedef kare hızı (uyarlama bunu kademe kademe düşürüp geri çıkarabilir).
    pub fps: u32,
    /// 0 = otomatik (boyut ve fps'e göre tablo).
    pub bitrate_kbps: u32,
}

/// Ardışık düzenin dış dünyayla bağı (çekirdek düğümü sink'te soyutlanır: testte sahte).
pub struct Sink {
    /// (pts_us, anahtar mı, Annex-B verisi) -> gönderildi mi (false = geri basınçla atıldı).
    pub send: Box<dyn Fn(u64, bool, Vec<u8>) -> bool + Send>,
    /// `now_us` -> geri bildirim (anahtar kare iste, teslim gecikmesi, uçuştaki bayt, telefon istatistiği)
    pub feedback: Box<dyn Fn(u64) -> ScreenFeedbackEx + Send>,
    pub on_status: Box<dyn Fn(Result<String, String>) + Send>,
    /// Güvenli masaüstü (UAC / kilit) açıldı (true) ya da kapandı (false).
    pub on_secure: Box<dyn Fn(bool) + Send>,
}

/// Boru hatları arası dışlama: durdurulan eski hat (WGC oturumu / D3D aygıtı / MF kodlayıcı / QUIC video akışı) hâlâ sönerken
/// yeni hat kaynaklara dokunmasın. Bekleme çağıranın değil, yeni hattın kendi iş parçacığında yapılır (UI/async bloke olmaz).
pub(crate) struct Gate {
    active: Mutex<usize>,
    cv: std::sync::Condvar,
}

pub(crate) struct Permit(Arc<Gate>);

impl Gate {
    pub(crate) fn new() -> Arc<Gate> {
        Arc::new(Gate { active: Mutex::new(0), cv: std::sync::Condvar::new() })
    }

    /// Önceki hatlar bitene dek en çok `max` bekler; süre dolarsa uyarıyla devam eder. (İzin, bekleme başarılı mı)
    pub(crate) fn acquire(self: &Arc<Self>, max: Duration) -> (Permit, bool) {
        let g = self.active.lock().unwrap_or_else(|e| e.into_inner());
        let (mut g, res) = self.cv.wait_timeout_while(g, max, |n| *n > 0).unwrap_or_else(|e| e.into_inner());
        *g += 1;
        (Permit(self.clone()), !res.timed_out())
    }
}

impl Drop for Permit {
    fn drop(&mut self) {
        let mut g = self.0.active.lock().unwrap_or_else(|e| e.into_inner());
        *g = g.saturating_sub(1);
        self.0.cv.notify_all();
    }
}

fn gate() -> &'static Arc<Gate> {
    static G: std::sync::OnceLock<Arc<Gate>> = std::sync::OnceLock::new();
    G.get_or_init(Gate::new)
}

const GATE_WAIT: Duration = Duration::from_secs(2);

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
        let stop_flag = stop.clone();
        let join = std::thread::Builder::new()
            .name("screen-share".into())
            .spawn(move || {
                let (_permit, ok) = gate().acquire(GATE_WAIT);
                if !ok {
                    log::warn!("ekran paylaşımı: önceki ardışık düzen {GATE_WAIT:?} içinde bitmedi, yine de başlatılıyor");
                }
                if stop_flag.load(Ordering::SeqCst) {
                    return; // beklerken durduruldu
                }
                thread_main(s2, k2, g2, c2, sink)
            })
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

/// Güvenli masaüstü izleyicisi: oturum yeniden kurulumlarında da yaşar (iş parçacığı ömrü).
struct SecureMonitor {
    watch: secure::SecureWatch,
    last_probe: Instant,
}

impl SecureMonitor {
    fn new() -> Self {
        let now = Instant::now();
        Self { watch: secure::SecureWatch::new(now), last_probe: now.checked_sub(secure::PROBE_EVERY).unwrap_or(now) }
    }

    fn secure(&self) -> bool {
        self.watch.is_secure()
    }

    fn apply(&self, changed: Option<bool>, sink: &Sink, key: &AtomicBool) -> bool {
        if let Some(s) = changed {
            log::info!("ekran paylaşımı: güvenli masaüstü {}", if s { "açıldı, görüntü durdu" } else { "kapandı, görüntü sürüyor" });
            (sink.on_secure)(s);
            if !s {
                key.store(true, Ordering::SeqCst); // devam eden ilk kare anahtar kare olsun
            }
        }
        changed.is_some()
    }

    /// Yeni kare yakalandı.
    fn frame(&mut self, sink: &Sink, key: &AtomicBool) {
        let c = self.watch.frame(Instant::now());
        self.apply(c, sink, key);
    }

    /// Aralıklı masaüstü sınaması (`force` = aralığı bekleme). Durum değiştiyse true.
    fn poll(&mut self, sink: &Sink, key: &AtomicBool, force: bool) -> bool {
        let now = Instant::now();
        if !force && now.duration_since(self.last_probe) < secure::PROBE_EVERY {
            return false;
        }
        self.last_probe = now;
        let c = self.watch.probe(now, secure::probe_desktop());
        self.apply(c, sink, key)
    }
}

/// Gerçek zamanlı davranış: 1 ms sistem zamanlayıcı çözünürlüğü (varsayılan ~15,6 ms: her `sleep(1ms)` bir kare süresi
/// uzar), Win11'de arka plan işleminin zamanlayıcı çözünürlüğünün yok sayılmaması, MMCSS "Playback" ve yüksek iş parçacığı önceliği.
struct Realtime {
    mmcss: Option<HANDLE>,
}

impl Realtime {
    fn enter() -> Self {
        unsafe {
            timeBeginPeriod(1);
            let st = PROCESS_POWER_THROTTLING_STATE {
                Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
                ControlMask: PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,
                StateMask: 0,
            };
            let _ = SetProcessInformation(
                GetCurrentProcess(),
                ProcessPowerThrottling,
                &st as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
            );
            let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
            let mut idx = 0u32;
            let mmcss = AvSetMmThreadCharacteristicsW(windows::core::w!("Playback"), &mut idx).ok();
            Realtime { mmcss }
        }
    }
}

impl Drop for Realtime {
    fn drop(&mut self) {
        unsafe {
            if let Some(h) = self.mmcss.take() {
                let _ = AvRevertMmThreadCharacteristics(h);
            }
            timeEndPeriod(1);
        }
    }
}

fn thread_main(stop: Arc<AtomicBool>, key: Arc<AtomicBool>, gen_: Arc<AtomicU64>, cfg: Arc<Mutex<Params>>, sink: Sink) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let _ = MFStartup(MF_VERSION, MFSTARTUP_FULL);
    }
    let _rt = Realtime::enter();
    let mut mon = SecureMonitor::new();
    loop {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        match run_session(&stop, &key, &gen_, &cfg, &sink, &mut mon) {
            Ok(Exit::Stop) => break,
            Ok(Exit::Restart) => {
                mon.poll(&sink, &key, true);
                std::thread::sleep(Duration::from_millis(if mon.secure() { 500 } else { 150 }));
            }
            Err(e) => {
                // Güvenli masaüstünde kurulum başarısız olabilir (yakalama kaynağı yok): hata değil, bekle ve yeniden dene.
                if mon.poll(&sink, &key, true) || mon.secure() || secure::probe_desktop() != secure::Desktop::Default {
                    log::info!("ekran paylaşımı: güvenli masaüstü, yeniden denenecek ({e})");
                    std::thread::sleep(Duration::from_millis(500));
                    continue;
                }
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

/// Kodlama zinciri: GPU dönüştürücü + kodlayıcı + etkin boyut/fps/bit hızı.
struct Chain {
    conv: capture::Converter,
    enc: encode::Encoder,
    size: (u32, u32),
    fps: u32,
    kbps: u32,
}

/// İstenen parametreler + uyarlama kademesi → (boyut, fps, kbps).
fn plan(src: (u32, u32), p: &Params, q: Quality) -> ((u32, u32), u32, u32) {
    let fps = q.fps.min(p.fps).clamp(5, 60);
    let size = choose_size(src.0, src.1, p.max_w, p.max_h, q.res_pct);
    let kbps = target_bitrate_kbps(p.bitrate_kbps, size.0, size.1, fps, q.bitrate_pct);
    (size, fps, kbps)
}

fn build_chain(gpu: &capture::Gpu, bgra: &ID3D11Texture2D, src: (u32, u32), p: &Params, q: Quality) -> windows::core::Result<Chain> {
    let (size, fps, kbps) = plan(src, p, q);
    let conv = capture::Converter::new(gpu, bgra, src.0, src.1, size.0, size.1, fps)?;
    let enc = encode::Encoder::new(gpu, size.0, size.1, fps, kbps)?;
    Ok(Chain { conv, enc, size, fps, kbps })
}

/// Uyarlama kademesi değişti: fps/boyut değiştiyse zinciri yeniden kur, yalnız bit hızıysa canlı güncelle.
fn apply_quality(mut chain: Chain, gpu: &capture::Gpu, bgra: &ID3D11Texture2D, src: (u32, u32), p: &Params, q: Quality) -> windows::core::Result<(Chain, bool)> {
    let (size, fps, kbps) = plan(src, p, q);
    if size != chain.size || fps != chain.fps {
        // Eski kodlayıcı/dönüştürücü önce bırakılır (bazı donanım MFT'ler tek örnek ister).
        drop(chain);
        return Ok((build_chain(gpu, bgra, src, p, q)?, true));
    }
    if kbps != chain.kbps {
        chain.enc.set_bitrate(kbps);
        chain.kbps = kbps;
    }
    Ok((chain, false))
}

const TICK: Duration = Duration::from_millis(500);

fn run_session(stop: &AtomicBool, key: &AtomicBool, gen_: &AtomicU64, cfg: &Mutex<Params>, sink: &Sink, smon: &mut SecureMonitor) -> windows::core::Result<Exit> {
    use capture::{GrabError, Source, Wgc, bgra_texture, create_gpu};
    let mut seen_gen = gen_.load(Ordering::SeqCst);
    let mut p = *cfg.lock().unwrap_or_else(|e| e.into_inner());
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
    let src_size = src.size();
    let bgra = bgra_texture(&gpu.device, src_size.0, src_size.1)?;
    let mut adaptor = Adaptor::new(p.fps);
    let mut chain = build_chain(&gpu, &bgra, src_size, &p, adaptor.current())?;
    (sink.on_status)(Ok(format!(
        "{} · {} · {}x{}@{} · {} kbps (kaynak {}x{}, görüntü alanı {}x{})",
        src.name(),
        chain.enc.describe(),
        chain.size.0,
        chain.size.1,
        chain.fps,
        chain.kbps,
        src_size.0,
        src_size.1,
        p.max_w,
        p.max_h
    )));

    let mut interval = Duration::from_micros(1_000_000 / chain.fps as u64);
    let mut next = Instant::now();
    let mut idx = 0usize;
    let mut dirty = false;
    let mut cap_us = now_us(); // son yakalanan karenin zamanı (pts)
    let mut last_encode = Instant::now();
    let mut last_sent = Instant::now();
    // 1 sn günlük penceresi + 500 ms uyarlama penceresi
    let mut win = StatsWindow::default();
    let mut win_start = Instant::now();
    let mut tick_start = Instant::now();
    let (mut tick_lag_max, mut tick_skipped, mut tick_phone_drops) = (0u64, 0u32, 0u32);
    let mut last_phone: Option<ScreenStatsReport> = None;
    let mut last_dropped = 0u64;
    loop {
        if stop.load(Ordering::SeqCst) {
            return Ok(Exit::Stop);
        }
        let g = gen_.load(Ordering::SeqCst);
        if g != seen_gen {
            // Canlı yeniden müzakere (döndürme/görüntü alanı/kalite/fps): yakalama oturumu AYNI kalır, yalnız zincir yeniden kurulur.
            seen_gen = g;
            let np = *cfg.lock().unwrap_or_else(|e| e.into_inner());
            if np.monitor != p.monitor {
                return Ok(Exit::Restart); // monitör değişimi: yakalama kaynağı yeniden kurulur
            }
            if np.fps != p.fps {
                adaptor = Adaptor::new(np.fps);
            }
            p = np;
            let (c, rebuilt) = apply_quality(chain, &gpu, &bgra, src_size, &p, adaptor.current())?;
            chain = c;
            log::info!(
                "ekran paylaşımı: yeniden müzakere {}x{}@{} {} kbps ({}) görüntü alanı {}x{}",
                chain.size.0,
                chain.size.1,
                chain.fps,
                chain.kbps,
                if rebuilt { "zincir yenilendi" } else { "canlı" },
                p.max_w,
                p.max_h
            );
            interval = Duration::from_micros(1_000_000 / chain.fps as u64);
            key.store(true, Ordering::SeqCst);
        }
        let wait = interval.as_millis().min(8) as u32;
        match src.grab(&bgra, &gpu.ctx, wait) {
            Ok(new) => {
                dirty |= new;
                if new {
                    cap_us = now_us();
                    win.grabs += 1;
                    smon.frame(sink, key);
                }
            }
            Err(GrabError::Restart) => return Ok(Exit::Restart),
        }
        smon.poll(sink, key, false);
        if key.swap(false, Ordering::SeqCst) {
            chain.enc.force_key();
            dirty = true;
            cap_us = now_us(); // yeniden kodlanan içerik güncel ekrandır; eski pts gecikme ölçümünü bozmasın
        }
        if last_encode.elapsed() > Duration::from_secs(2) && !smon.secure() {
            dirty = true; // durağan ekranda da zaman zaman tazele
            cap_us = now_us();
        }

        // ---- 500 ms uyarlama + 1 sn günlük
        if tick_start.elapsed() >= TICK {
            tick_start = Instant::now();
            let v = verdict(tick_lag_max, tick_skipped, tick_phone_drops);
            (tick_lag_max, tick_skipped, tick_phone_drops) = (0, 0, 0);
            if let Some(nq) = adaptor.tick(v) {
                log::info!(
                    "ekran paylaşımı: kalite kademesi L{} → {} fps, %{} bit hızı, %{} çözünürlük",
                    adaptor.level(),
                    nq.fps,
                    nq.bitrate_pct,
                    nq.res_pct
                );
                let (c, rebuilt) = apply_quality(chain, &gpu, &bgra, src_size, &p, nq)?;
                chain = c;
                if rebuilt {
                    interval = Duration::from_micros(1_000_000 / chain.fps as u64);
                    key.store(true, Ordering::SeqCst);
                }
            }
        }
        if win_start.elapsed() >= Duration::from_secs(1) {
            let secs = win_start.elapsed().as_secs_f64();
            win_start = Instant::now();
            log::info!("{}", win.line(secs, chain.kbps, chain.size, adaptor.current(), adaptor.level()));
            if let Some(r) = last_phone {
                log::info!("{}", phone_line(&r));
            }
            win = StatsWindow::default();
        }

        let now = Instant::now();
        if !dirty || now < next {
            continue;
        }
        // ---- üretim kapısı: alıcı onayı geride kaldıysa YENİ KARE ÜRETME (kodlayıcıya verme). En yeni kare sonra gider.
        let fb = (sink.feedback)(now_us());
        if fb.want_key {
            chain.enc.force_key();
            dirty = true;
            cap_us = now_us();
        }
        win.lag(fb.lag_ms, fb.pending, fb.inflight_bytes);
        tick_lag_max = tick_lag_max.max(fb.lag_ms);
        if fb.dropped_total > last_dropped {
            let d = (fb.dropped_total - last_dropped) as u32;
            win.dropped += d;
            tick_skipped += d;
            last_dropped = fb.dropped_total;
        }
        if let Some(r) = fb.phone
            && Some(r) != last_phone
        {
            tick_phone_drops += r.late_drops as u32;
            last_phone = Some(r);
        }
        if fb.ack_mode && fb.lag_ms > SKIP_LAG_MS && last_sent.elapsed() < Duration::from_millis(500) {
            win.skipped += 1;
            tick_skipped += 1;
            next = now + interval / 2;
            continue;
        }
        next = (next + interval).max(now.checked_sub(interval).unwrap_or(now));
        dirty = false;
        last_encode = now;

        let t_enc = Instant::now();
        chain.conv.convert(idx)?;
        let frames = chain.enc.encode(&gpu, &chain.conv.nv12[idx], (cap_us * 10) as i64)?;
        let call_us = t_enc.elapsed().as_micros() as u64;
        idx = (idx + 1) % capture::RING;
        for f in frames {
            let pts_us = (f.pts_100ns / 10) as u64;
            let (len, is_key) = (f.data.len(), f.key);
            win.encoded(is_key, len, call_us, now_us().saturating_sub(pts_us));
            if (sink.send)(pts_us, is_key, f.data) {
                last_sent = Instant::now();
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

#[cfg(test)]
mod gate_tests {
    use super::*;

    #[test]
    fn second_waits_for_first_release() {
        let g = Gate::new();
        let (p1, ok1) = g.acquire(Duration::from_millis(50));
        assert!(ok1);
        let g2 = g.clone();
        let h = std::thread::spawn(move || {
            let t = Instant::now();
            let (_p, ok) = g2.acquire(Duration::from_secs(2));
            (ok, t.elapsed())
        });
        std::thread::sleep(Duration::from_millis(150));
        drop(p1);
        let (ok, el) = h.join().unwrap();
        assert!(ok);
        assert!(el >= Duration::from_millis(100), "{el:?}");
    }

    #[test]
    fn times_out_and_proceeds() {
        let g = Gate::new();
        let (_p1, _) = g.acquire(Duration::from_millis(10));
        let (_p2, ok) = g.acquire(Duration::from_millis(80));
        assert!(!ok);
    }
}
