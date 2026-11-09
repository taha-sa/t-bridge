//! Telefon kamerası (Windows 11): sanal kamera yaşam döngüsü, DLL kaydı (tek seferlik yönetici izni), kare hattı.
//!
//! Hat: telefon --QUIC (H.264)--> `Hub::push_frame` --> çözücü thread (MF H.264 -> NV12) --> paylaşımlı bellek halkası
//!      --> tbridge_vcam.dll (Frame Server içinde, IMFMediaSourceEx) --> Zoom/Teams/Kamera uygulaması.
//! Halka bölümünü DLL yaratır; kimse kamerayı açmamışsa bölüm yoktur ve kareler çözülmeden atılır (CPU harcanmaz).
pub mod decode;

use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{self, Receiver, Sender, SyncSender},
};
use std::time::Duration;

use serde::Serialize;
use windows::{
    Wdk::System::SystemServices::RtlGetVersion,
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        Media::MediaFoundation::*,
        System::{
            Com::{COINIT_MULTITHREADED, CoInitializeEx},
            Memory::{FILE_MAP_READ, FILE_MAP_WRITE, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile},
            Registry::*,
            SystemInformation::{GetTickCount64, OSVERSIONINFOW},
            Threading::{GetExitCodeProcess, WaitForSingleObject},
        },
        UI::{
            Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW},
            WindowsAndMessaging::SW_HIDE,
        },
    },
    core::{PCWSTR, w},
};

use kopru_camring::Ring;

/// Sanal kamera medya kaynağının CLSID'si (camera/vcam ile aynı).
pub const CLSID_STR: &str = "{7B3F1C52-4D2E-4C71-9A0B-5E2D8F6A1C34}";
pub const DLL_NAME: &str = "tbridge_vcam.dll";
pub const FRIENDLY_NAME: &str = "T-Bridge Kamera";

// ---------------------------------------------------------------- Windows sürüm kapısı

/// RtlGetVersion ile (GetVersionEx uyumluluk katmanında yalan söyler) derleme numarası.
pub fn windows_build() -> Option<u32> {
    let mut v = OSVERSIONINFOW { dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32, ..Default::default() };
    let st = unsafe { RtlGetVersion(&mut v) };
    if st.0 == 0 { Some(v.dwBuildNumber) } else { None }
}

/// Kamera özelliği bu makinede kullanılabilir mi (Windows 11, build >= 22000).
pub fn camera_allowed_now() -> bool {
    kopru_core::camera::camera_allowed(cfg!(windows), windows_build())
}

// ---------------------------------------------------------------- kayıt (HKLM, yükseltilmiş yardımcı)

pub fn install_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("ProgramFiles").unwrap_or_else(|| "C:\\Program Files".into())).join("T-Bridge").join("camera")
}

fn clsid_key() -> String {
    format!("SOFTWARE\\Classes\\CLSID\\{CLSID_STR}")
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn reg_set_sz(sub: &str, name: Option<&str>, value: &str) -> Result<(), String> {
    unsafe {
        let mut h = HKEY::default();
        let sub = wide(sub);
        RegCreateKeyExW(HKEY_LOCAL_MACHINE, PCWSTR(sub.as_ptr()), None, PCWSTR::null(), REG_OPTION_NON_VOLATILE, KEY_WRITE, None, &mut h, None)
            .ok()
            .map_err(|e| format!("kayıt defteri anahtarı: {e}"))?;
        let data: Vec<u8> = wide(value).iter().flat_map(|c| c.to_le_bytes()).collect();
        let nm = name.map(wide);
        let r = RegSetValueExW(h, nm.as_ref().map_or(PCWSTR::null(), |n| PCWSTR(n.as_ptr())), None, REG_SZ, Some(&data));
        let _ = RegCloseKey(h);
        r.ok().map_err(|e| format!("kayıt defteri değeri: {e}"))
    }
}

/// Bu sürecin kullanıcısının SID dizgesi (S-1-5-21-...).
pub fn current_user_sid() -> Option<String> {
    use windows::Win32::{
        Security::{Authorization::ConvertSidToStringSidW, GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };
    unsafe {
        let mut tok = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut tok).ok()?;
        let mut len = 0u32;
        let _ = GetTokenInformation(tok, TokenUser, None, 0, &mut len);
        let mut buf = vec![0u8; len as usize];
        let r = GetTokenInformation(tok, TokenUser, Some(buf.as_mut_ptr() as *mut _), len, &mut len);
        let _ = CloseHandle(tok);
        r.ok()?;
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut p = windows::core::PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut p).ok()?;
        let s = p.to_string().ok();
        let _ = windows::Win32::Foundation::LocalFree(Some(windows::Win32::Foundation::HLOCAL(p.0 as *mut _)));
        s
    }
}

/// Kayıtta saklanan sahip SID'i (HKLM).
pub fn registered_owner_sid() -> Option<String> {
    unsafe {
        let key = wide(&clsid_key());
        let name = wide("OwnerSid");
        let mut buf = [0u16; 256];
        let mut len = (buf.len() * 2) as u32;
        let r = RegGetValueW(HKEY_LOCAL_MACHINE, PCWSTR(key.as_ptr()), PCWSTR(name.as_ptr()), RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr() as *mut _), Some(&mut len));
        if r.0 != 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..(len as usize / 2).saturating_sub(1).min(buf.len())]))
    }
}

/// Kayıt var ve DLL yerinde VE sahip SID'i bu kullanıcı (başka kullanıcı etkinleştirdiyse yeniden kayıt gerekir).
pub fn is_installed_for_me() -> bool {
    is_installed() && registered_owner_sid().is_some() && registered_owner_sid() == current_user_sid()
}

/// Derleme zamanında gömülen, paketlenen tbridge_vcam.dll'in SHA-256'sı (build.rs). Boş = yer tutucu DLL (yalnız debug).
const EXPECTED_DLL_SHA256: &str = include_str!(concat!(env!("OUT_DIR"), "/vcam_sha256.txt"));

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// DLL'i bir kez okur ve karmasını doğrular; yazılacak baytlar doğrulananlarla aynıdır (kontrol-sonra-kullan yarışı yok).
/// `expected` boşsa yalnız debug derlemede kabul edilir.
fn read_verified(src: &Path, expected: &str) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(src).map_err(|e| format!("DLL okunamadı ({}): {e}", src.display()))?;
    if expected.trim().is_empty() {
        if cfg!(debug_assertions) {
            return Ok(bytes);
        }
        return Err("gömülü DLL karması yok".into());
    }
    if !sha256_hex(&bytes).eq_ignore_ascii_case(expected.trim()) {
        return Err("DLL karması beklenenle uyuşmuyor".into());
    }
    Ok(bytes)
}

/// Paketli DLL'in sabit kaynağı: yalnızca bu exe'nin kendi klasörü (argv'den yol alınmaz).
fn bundled_dll() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let d = exe.parent().ok_or("exe klasörü yok")?;
    [d.join("camera").join(DLL_NAME), d.join(DLL_NAME)]
        .into_iter()
        .find(|p| p.is_file())
        .or_else(|| if cfg!(debug_assertions) { locate_dll(None) } else { None }) // geliştirme: target/ altında arar
        .ok_or_else(|| "paketli DLL bulunamadı".to_string())
}

/// Yükseltilmiş süreçte çalışır: paketli DLL'i (karması doğrulanmış) %ProgramFiles%\T-Bridge\camera\ altına yazar ve yalnız o yolu HKLM'ye kaydeder.
pub fn register_elevated(owner_sid: &str) -> Result<(), String> {
    register_elevated_from(&bundled_dll()?, EXPECTED_DLL_SHA256, owner_sid)
}

fn register_elevated_from(src_dll: &Path, expected_sha: &str, owner_sid: &str) -> Result<(), String> {
    if !kopru_camring::is_valid_sid(owner_sid) {
        return Err("geçersiz kullanıcı SID'i".into());
    }
    let bytes = read_verified(src_dll, expected_sha)?;
    let dir = install_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("klasör: {e}"))?;
    let dst = dir.join(DLL_NAME);
    std::fs::write(&dst, &bytes).map_err(|e| format!("DLL yazılamadı: {e}"))?;
    let key = clsid_key();
    reg_set_sz(&key, None, FRIENDLY_NAME)?;
    reg_set_sz(&format!("{key}\\InprocServer32"), None, &dst.to_string_lossy())?;
    reg_set_sz(&format!("{key}\\InprocServer32"), Some("ThreadingModel"), "Both")?;
    // Halka DACL'i yalnızca bu kullanıcıya açılır (DLL bu değeri okur).
    reg_set_sz(&key, Some("OwnerSid"), owner_sid)?;
    Ok(())
}

/// `--camera-register <sid>`: tam olarak bir argüman; fazlası/azı reddedilir. SID sıkı doğrulanır.
fn parse_register_args(args: &[String]) -> Result<String, String> {
    match args {
        [flag, sid] if flag == "--camera-register" && kopru_camring::is_valid_sid(sid) => Ok(sid.clone()),
        _ => Err("geçersiz argümanlar".into()),
    }
}

pub fn unregister_elevated() -> Result<(), String> {
    unsafe {
        let key = wide(&clsid_key());
        let r = RegDeleteTreeW(HKEY_LOCAL_MACHINE, PCWSTR(key.as_ptr()));
        if r.0 != 0 && r.0 != 2 {
            return Err(format!("kayıt silinemedi: {}", r.0));
        }
    }
    let dir = install_dir();
    let _ = std::fs::remove_file(dir.join(DLL_NAME)); // Frame Server DLL'i tutuyorsa silinemeyebilir: sonraki açılışta temizlenir
    let _ = std::fs::remove_dir(&dir);
    Ok(())
}

/// COM kaydı var mı ve DLL yerinde mi.
pub fn is_installed() -> bool {
    unsafe {
        let key = wide(&format!("{}\\InprocServer32", clsid_key()));
        let mut h = HKEY::default();
        if RegOpenKeyExW(HKEY_LOCAL_MACHINE, PCWSTR(key.as_ptr()), None, KEY_READ, &mut h).0 != 0 {
            return false;
        }
        let _ = RegCloseKey(h);
    }
    install_dir().join(DLL_NAME).exists()
}

/// Bu exe'yi UAC ile yükseltilmiş başlatıp çıkışını bekler. Dönüş: çıkış kodu (0 = başarılı).
pub fn run_elevated(args: &str) -> Result<u32, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let (file, params) = (wide(&exe.to_string_lossy()), wide(args));
    unsafe {
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            lpVerb: w!("runas"),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(params.as_ptr()),
            nShow: SW_HIDE.0,
            ..Default::default()
        };
        ShellExecuteExW(&mut info).map_err(|e| format!("yönetici izni alınamadı veya reddedildi: {e}"))?;
        let h = info.hProcess;
        if h.is_invalid() {
            return Err("yükseltilmiş süreç başlatılamadı".into());
        }
        WaitForSingleObject(h, 60_000);
        let mut code = 1u32;
        let _ = GetExitCodeProcess(h, &mut code);
        let _ = CloseHandle(h);
        Ok(code)
    }
}

/// DLL'in aranacağı yerler: exe yanı, kaynak klasörü, geliştirme derleme çıktısı.
#[cfg_attr(not(debug_assertions), allow(dead_code))]
fn locate_dll(extra_dir: Option<PathBuf>) -> Option<PathBuf> {
    // 4 KB'tan küçük dosya build.rs'in bıraktığı yer tutucudur (geliştirme): gerçek DLL sayılmaz.
    let real = |p: &PathBuf| p.metadata().map(|m| m.len() > 4096).unwrap_or(false);
    let mut c: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            c.push(d.join("camera").join(DLL_NAME)); // Tauri kaynakları (kurulu sürüm)
            c.push(d.join(DLL_NAME));
            // geliştirme: <repo>/target/{release,debug}/ (exe zaten target\debug içinde)
            let mut up = Some(d);
            for _ in 0..4 {
                if let Some(u) = up {
                    c.push(u.join("release").join(DLL_NAME));
                    c.push(u.join(DLL_NAME));
                    up = u.parent();
                }
            }
        }
    }
    if let Some(d) = extra_dir {
        c.push(d.join("camera").join(DLL_NAME));
        c.push(d.join(DLL_NAME));
    }
    c.into_iter().find(real)
}

/// Komut satırı yükseltilmiş yardımcı modu. `Some(çıkış kodu)` dönerse süreç bununla çıkmalıdır.
pub fn maybe_run_helper() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("--camera-register") => Some(match parse_register_args(&args[1..]).and_then(|sid| register_elevated(&sid)) {
            Ok(()) => 0,
            Err(e) => {
                let _ = std::fs::write(std::env::temp_dir().join("tbridge-camera-register.log"), e);
                1
            }
        }),
        Some("--camera-unregister") if args.len() == 2 => Some(match unregister_elevated() {
            Ok(()) => 0,
            Err(_) => 1,
        }),
        _ => None,
    }
}

// ---------------------------------------------------------------- sanal kamera (MFCreateVirtualCamera)

enum VcamCmd {
    Enable(Sender<Result<(), String>>),
    Disable(Sender<()>),
}

fn vcam_thread(rx: Receiver<VcamCmd>, running: Arc<AtomicBool>) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let _ = MFStartup(MF_VERSION, MFSTARTUP_FULL);
    }
    let mut cam: Option<IMFVirtualCamera> = None;
    while let Ok(cmd) = rx.recv() {
        match cmd {
            VcamCmd::Enable(reply) => {
                if cam.is_some() {
                    let _ = reply.send(Ok(()));
                    continue;
                }
                let r = (|| -> Result<IMFVirtualCamera, String> {
                    unsafe {
                        let name = wide(FRIENDLY_NAME);
                        let id = wide(CLSID_STR);
                        // Oturum ömürlü + yalnızca geçerli kullanıcı: T-Bridge kapanınca kamera da kalkar.
                        let c = MFCreateVirtualCamera(
                            MFVirtualCameraType_SoftwareCameraSource,
                            MFVirtualCameraLifetime_Session,
                            MFVirtualCameraAccess_CurrentUser,
                            PCWSTR(name.as_ptr()),
                            PCWSTR(id.as_ptr()),
                            None,
                        )
                        .map_err(|e| format!("MFCreateVirtualCamera: {e}"))?;
                        c.Start(None).map_err(|e| format!("sanal kamera başlatılamadı: {e}"))?;
                        Ok(c)
                    }
                })();
                match r {
                    Ok(c) => {
                        cam = Some(c);
                        running.store(true, Ordering::SeqCst);
                        let _ = reply.send(Ok(()));
                    }
                    Err(e) => {
                        let _ = reply.send(Err(e));
                    }
                }
            }
            VcamCmd::Disable(reply) => {
                if let Some(c) = cam.take() {
                    unsafe {
                        let _ = c.Stop();
                        let _ = c.Remove();
                        let _ = c.Shutdown();
                    }
                }
                running.store(false, Ordering::SeqCst);
                let _ = reply.send(());
            }
        }
    }
    if let Some(c) = cam.take() {
        unsafe {
            let _ = c.Remove();
            let _ = c.Shutdown();
        }
    }
}

// ---------------------------------------------------------------- merkez

#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoneCam {
    pub active: bool,
    pub w: u32,
    pub h: u32,
    pub fps: u32,
    pub facing: u8,
    pub mirror: bool,
    /// Karenin dik görünmesi için saat yönünde uygulanacak derece.
    pub rotation: u16,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraStatus {
    /// Windows 11 mi (değilse arayüz kilitli kartı gösterir).
    pub supported: bool,
    pub installed: bool,
    /// Sanal kamera şu an Windows'a kayıtlı/çalışıyor.
    pub running: bool,
    pub phone_supports: bool,
    pub phone: PhoneCam,
    /// Bir uygulama kamerayı açmış (paylaşımlı bellek bağlı).
    pub in_use: bool,
    pub decoded_frames: u64,
}

pub type FrameMsg = (u64, bool, Vec<u8>);

/// Kare hattı geri çağrıları: anahtar kare / biçim isteği.
pub trait PhoneLink: Send + Sync + 'static {
    fn request_keyframe(&self);
    fn request_format(&self, w: u32, h: u32);
    /// Tüketici kapandı / akış bitti: telefona CamStop gönder.
    fn request_stop(&self);
}

pub struct Hub {
    vcam_tx: Sender<VcamCmd>,
    frame_tx: SyncSender<FrameMsg>,
    pub running: Arc<AtomicBool>,
    pub in_use: Arc<AtomicBool>,
    pub decoded: Arc<AtomicU64>,
    pub phone: Arc<Mutex<PhoneCam>>,
    link: Arc<dyn PhoneLink>,
}

impl Hub {
    pub fn start(link: Arc<dyn PhoneLink>) -> Hub {
        let running = Arc::new(AtomicBool::new(false));
        let (vtx, vrx) = mpsc::channel();
        let r2 = running.clone();
        let _ = std::thread::Builder::new().name("vcam".into()).spawn(move || vcam_thread(vrx, r2));

        let (ftx, frx) = mpsc::sync_channel::<FrameMsg>(8);
        let in_use = Arc::new(AtomicBool::new(false));
        let decoded = Arc::new(AtomicU64::new(0));
        let phone = Arc::new(Mutex::new(PhoneCam::default()));
        let (iu, dc, ph) = (in_use.clone(), decoded.clone(), phone.clone());
        let l2 = link.clone();
        let _ = std::thread::Builder::new().name("cam-decode".into()).spawn(move || decode_thread(frx, l2, iu, dc, ph));
        Hub { vcam_tx: vtx, frame_tx: ftx, running, in_use, decoded, phone, link }
    }

    pub fn enable(&self) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.vcam_tx.send(VcamCmd::Enable(tx)).map_err(|e| e.to_string())?;
        rx.recv_timeout(Duration::from_secs(20)).map_err(|_| "sanal kamera zaman aşımı".to_string())?
    }

    pub fn disable(&self) {
        let (tx, rx) = mpsc::channel();
        if self.vcam_tx.send(VcamCmd::Disable(tx)).is_ok() {
            let _ = rx.recv_timeout(Duration::from_secs(10));
        }
        self.link.request_stop();
    }

    /// Telefondan gelen kodlanmış kare (QUIC okuyucu thread'i): kuyruk doluysa kare atılır.
    pub fn push_frame(&self, f: FrameMsg) {
        let _ = self.frame_tx.try_send(f);
    }
}

struct RingMap {
    view: *mut u8,
    h: HANDLE,
    ring: Ring,
}

impl RingMap {
    fn open() -> Option<Self> {
        unsafe {
            let name = wide(kopru_camring::SECTION_NAME);
            // DACL (kopru_camring::build_sddl) yalnızca GR|GW verir: FILE_MAP_ALL_ACCESS istemek ACCESS_DENIED döner.
            let h = OpenFileMappingW((FILE_MAP_READ | FILE_MAP_WRITE).0, false, PCWSTR(name.as_ptr())).ok()?;
            let view = MapViewOfFile(h, FILE_MAP_READ | FILE_MAP_WRITE, 0, 0, 0);
            if view.Value.is_null() {
                let _ = CloseHandle(h);
                return None;
            }
            let ring = Ring::from_raw(view.Value as *mut u8);
            if !ring.is_valid() {
                let _ = UnmapViewOfFile(view);
                let _ = CloseHandle(h);
                return None;
            }
            Some(Self { view: view.Value as *mut u8, h, ring })
        }
    }
}

impl Drop for RingMap {
    fn drop(&mut self) {
        unsafe {
            let _ = UnmapViewOfFile(windows::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS { Value: self.view as *mut _ });
            let _ = CloseHandle(self.h);
        }
    }
}

fn decode_thread(rx: Receiver<FrameMsg>, link: Arc<dyn PhoneLink>, in_use: Arc<AtomicBool>, decoded: Arc<AtomicU64>, phone: Arc<Mutex<PhoneCam>>) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let _ = MFStartup(MF_VERSION, MFSTARTUP_FULL);
    }
    let mut ring: Option<RingMap> = None;
    let mut dec: Option<decode::Decoder> = None;
    let mut need_key = true;
    let mut last_want = (0u32, 0u32);
    let mut last_check = std::time::Instant::now() - Duration::from_secs(1);
    loop {
        let msg = rx.recv_timeout(Duration::from_millis(250));
        // Halka bağlantısını ~500 ms'de bir yokla: yoksa (kimse kamerayı açmadı) çözmeden at.
        if last_check.elapsed() > Duration::from_millis(500) {
            last_check = std::time::Instant::now();
            let tick = unsafe { GetTickCount64() };
            if let Some(r) = &ring {
                // Tüketici 3 sn'dir dokunmadıysa kamera kapatıldı: bağlantıyı bırak.
                if tick.saturating_sub(r.ring.consumer_ms()) > 3000 && r.ring.consumer_ms() != 0 {
                    ring = None;
                    last_want = (0, 0);
                    link.request_stop(); // tüketici kapandı: telefon kamerayı/kodlayıcıyı durdursun
                }
            }
            if ring.is_none() {
                ring = RingMap::open();
                if ring.is_some() {
                    need_key = true;
                    link.request_keyframe();
                }
            }
            in_use.store(ring.is_some(), Ordering::SeqCst);
            if let Some(r) = &ring {
                let want = r.ring.want();
                if want != last_want && want.0 > 0 {
                    last_want = want;
                    link.request_format(want.0, want.1);
                }
            }
        }
        let Ok((pts, key, data)) = msg else { continue };
        let Some(r) = &ring else {
            need_key = true;
            continue;
        };
        if need_key && !key {
            continue;
        }
        if key {
            need_key = false;
        }
        if dec.is_none() {
            dec = decode::Decoder::new().ok();
        }
        let Some(d) = dec.as_mut() else { continue };
        match d.decode(&data, pts as i64 * 10) {
            Ok(frames) => {
                let (mirror, rot) = phone.lock().map(|p| (p.mirror, p.rotation as u32)).unwrap_or((false, 0));
                for f in frames {
                    // Dik görüntü: önce döndür (portre tutuş), sonra ayna; ardından halkanın istediği çerçeveye sığdır.
                    let (mut nv, mut w, mut h) = kopru_camring::rotate_nv12(&f.nv12, f.w as usize, f.h as usize, rot);
                    if mirror {
                        kopru_camring::mirror_nv12(&mut nv, w, h);
                    }
                    let (ww, wh) = r.ring.want();
                    if ww > 0 && wh > 0 && (w as u32 != ww || h as u32 != wh) {
                        nv = kopru_camring::letterbox_nv12(&nv, w, h, ww as usize, wh as usize);
                        w = ww as usize;
                        h = wh as usize;
                    }
                    let now = unsafe { GetTickCount64() };
                    if r.ring.write(w as u32, h as u32, pts, now, &nv) {
                        decoded.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            Err(_) => {
                dec = None; // bozuk akış: çözücüyü sıfırla, yeni anahtar kare iste
                need_key = true;
                link.request_keyframe();
            }
        }
    }
}

#[cfg(test)]
mod helper_tests {
    use super::*;

    /// Y1: yaratıcı tarafın SDDL'iyle bölüm yarat, uygulama tarafı hakları (READ|WRITE) ile aç; ALL_ACCESS reddedilmeli.
    #[test]
    fn ring_section_opens_with_read_write() {
        use windows::Win32::Foundation::INVALID_HANDLE_VALUE;
        use windows::Win32::Security::Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1};
        use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
        use windows::Win32::System::Memory::{CreateFileMappingW, FILE_MAP_ALL_ACCESS, PAGE_READWRITE};
        let sid = current_user_sid().expect("sid");
        let sddl = wide(&kopru_camring::build_sddl(Some(&sid)));
        let name_s = format!(r"Local\TBridgeCameraRingTest{}", std::process::id());
        let name = wide(&name_s);
        unsafe {
            let mut sd = PSECURITY_DESCRIPTOR::default();
            ConvertStringSecurityDescriptorToSecurityDescriptorW(PCWSTR(sddl.as_ptr()), SDDL_REVISION_1, &mut sd, None).unwrap();
            let sa = SECURITY_ATTRIBUTES { nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32, lpSecurityDescriptor: sd.0, bInheritHandle: false.into() };
            let size = kopru_camring::TOTAL_SIZE as u64;
            let creator = CreateFileMappingW(INVALID_HANDLE_VALUE, Some(&sa), PAGE_READWRITE, (size >> 32) as u32, size as u32, PCWSTR(name.as_ptr())).unwrap();
            let h = OpenFileMappingW((FILE_MAP_READ | FILE_MAP_WRITE).0, false, PCWSTR(name.as_ptr())).expect("READ|WRITE açılmalı");
            let view = MapViewOfFile(h, FILE_MAP_READ | FILE_MAP_WRITE, 0, 0, 0);
            assert!(!view.Value.is_null());
            let _ = UnmapViewOfFile(view);
            let _ = CloseHandle(h);
            // Belgeleme: tam erişim (WRITE_DAC vb.) DACL'de yok.
            assert!(OpenFileMappingW(FILE_MAP_ALL_ACCESS.0, false, PCWSTR(name.as_ptr())).is_err());
            let _ = CloseHandle(creator);
        }
    }

    #[test]
    fn register_args_strict() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(parse_register_args(&a(&["--camera-register", "S-1-5-21-1-2-3-1001"])).is_ok());
        assert!(parse_register_args(&a(&["--camera-register", "C:/evil.dll", "S-1-5-21-1-2-3-1001"])).is_err());
        assert!(parse_register_args(&a(&["--camera-register", "C:/evil.dll"])).is_err());
        assert!(parse_register_args(&a(&["--camera-register"])).is_err());
        assert!(parse_register_args(&a(&["--camera-register", "S-1-5-21-1-2-3-1001", "x"])).is_err());
        assert!(parse_register_args(&a(&["--camera-register", "S-1-5-21; calc"])).is_err());
    }

    #[test]
    fn dll_hash_verified() {
        let f = std::env::temp_dir().join(format!("kopru_vcam_{}.dll", std::process::id()));
        std::fs::write(&f, b"hello").unwrap();
        let good = sha256_hex(b"hello");
        assert_eq!(read_verified(&f, &good).unwrap(), b"hello");
        assert!(read_verified(&f, &good.to_uppercase()).is_ok());
        assert!(read_verified(&f, &sha256_hex(b"evil")).is_err());
        let _ = std::fs::remove_file(&f);
    }
}
