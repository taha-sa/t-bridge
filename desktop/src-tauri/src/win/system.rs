//! Güç işlemleri, otomatik başlatma (HKCU\…\Run), Gezgin'de gösterme.
use std::{os::windows::process::CommandExt, process::Command};

use kopru_core::PowerAction;
use windows::{
    Win32::System::{
        Power::SetSuspendState,
        Registry::{HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW},
        Shutdown::LockWorkStation,
    },
    core::w,
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const RUN_KEY: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const RUN_VALUE: windows::core::PCWSTR = w!("T-Bridge");
/// v0.1 (Köprü) kaydı: güncellemede temizlenir.
const OLD_RUN_VALUE: windows::core::PCWSTR = w!("Kopru");

pub fn power(a: PowerAction) {
    match a {
        PowerAction::Lock => unsafe {
            let _ = LockWorkStation();
        },
        PowerAction::Sleep => {
            // Ayrı iş parçacığı: çağrı makine uyanana kadar dönmeyebilir.
            std::thread::spawn(|| unsafe {
                let _ = SetSuspendState(false, false, false);
            });
        }
        PowerAction::Shutdown => {
            let _ = Command::new("shutdown").args(["/s", "/t", "0"]).creation_flags(CREATE_NO_WINDOW).spawn();
        }
        PowerAction::Restart => {
            let _ = Command::new("shutdown").args(["/r", "/t", "0"]).creation_flags(CREATE_NO_WINDOW).spawn();
        }
    }
}

pub fn set_autostart(on: bool) -> bool {
    unsafe {
        if on {
            let Ok(exe) = std::env::current_exe() else { return false };
            let cmd = format!("\"{}\" --hidden", exe.display());
            let wide: Vec<u16> = cmd.encode_utf16().chain(Some(0)).collect();
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                RUN_VALUE,
                REG_SZ.0,
                Some(wide.as_ptr() as *const _),
                (wide.len() * 2) as u32,
            )
            .is_ok()
        } else {
            let r = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE);
            r.is_ok() || r.0 == 2 // ERROR_FILE_NOT_FOUND: zaten yok
        }
    }
}

/// Eski adla kalmış otomatik başlatma kaydını siler.
pub fn remove_legacy_autostart() {
    unsafe {
        let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, OLD_RUN_VALUE);
    }
}

pub fn autostart_enabled() -> bool {
    unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE, RRF_RT_REG_SZ, None, None, None).is_ok() }
}

/// URL'yi varsayılan tarayıcıda aç.
pub fn reveal_url(url: &str) {
    use windows::{Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL}, core::HSTRING};
    unsafe {
        ShellExecuteW(None, w!("open"), &HSTRING::from(url), None, None, SW_SHOWNORMAL);
    }
}

/// Dosyayı Gezgin'de seçili göster; klasörse aç.
pub fn reveal(path: &str) {
    let p = std::path::Path::new(path);
    if p.is_dir() {
        let _ = Command::new("explorer").arg(path).spawn();
    } else {
        let _ = Command::new("explorer").raw_arg(format!("/select,\"{path}\"")).spawn();
    }
}

/// Dosyayı varsayılan uygulamayla aç.
pub fn open_file(path: &str) {
    use windows::{Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL}, core::HSTRING};
    unsafe {
        ShellExecuteW(None, w!("open"), &HSTRING::from(path), None, None, SW_SHOWNORMAL);
    }
}

fn reg_query(args: &[&str]) -> Option<String> {
    let o = Command::new("reg").arg("query").args(args).creation_flags(CREATE_NO_WINDOW).output().ok()?;
    if !o.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&o.stdout).into_owned();
    text.lines().find_map(|l| l.split_once("REG_SZ").map(|(_, v)| v.trim().to_string())).filter(|v| !v.is_empty())
}

fn powerpoint_exe() -> Option<String> {
    let p = reg_query(&[r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\POWERPNT.EXE", "/ve"])?;
    std::path::Path::new(&p).is_file().then_some(p)
}

/// .pdf için kullanıcının seçtiği işleyicinin tam ekran kısayolu. Güvenilir olanlar: Acrobat/Reader (Ctrl+L), Edge/Chrome/Firefox (F11),
/// SumatraPDF (F5 sunum). Bilinmeyen işleyicide hiçbir tuş gönderilmez (yanlış kısayol riski).
fn pdf_fullscreen_key(ext: &str) -> Option<&'static str> {
    let prog = reg_query(&[&format!(r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.{ext}\UserChoice"), "/v", "ProgId"])?.to_lowercase();
    if prog.contains("acro") {
        Some("ctrl_l")
    } else if prog.contains("msedge") || prog.contains("chrome") || prog.contains("firefox") || prog.contains("brave") {
        Some("f11")
    } else if prog.contains("sumatra") {
        Some("f5")
    } else {
        None
    }
}

/// Sunum aç: PowerPoint kuruluysa "/S" ile doğrudan slayt gösterisi; değilse varsayılan uygulama. PDF'te varsayılan işleyici açılır,
/// birkaç sn sonra bilinen işleyiciye göre tam ekran kısayolu gönderilir (en iyi çaba; Windows odağı başka pencereye verirse kaçabilir).
pub fn present_file(path: &str) {
    let ext = std::path::Path::new(path).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if matches!(ext.as_str(), "pptx" | "ppt" | "pptm" | "ppsx" | "pps") {
        if let Some(exe) = powerpoint_exe() {
            if Command::new(exe).arg("/S").arg(path).spawn().is_ok() {
                return;
            }
        }
        open_file(path);
        return;
    }
    open_file(path);
    if ext == "pdf" {
        std::thread::sleep(std::time::Duration::from_millis(2500));
        if let Some(k) = pdf_fullscreen_key("pdf") {
            crate::win::input::presentation_fullscreen(k);
        }
    }
}
