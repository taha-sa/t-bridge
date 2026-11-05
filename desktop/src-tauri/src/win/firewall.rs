//! Güvenlik duvarı: kullanıcı başına kurulumda yönetici izni yok; kural uygulama tarafından
//! bir kez, Windows onayıyla (runas) eklenir. Kural exe yoluna bağlıdır; güncellemeler yolu değiştirmez.
use std::{os::windows::process::CommandExt, process::Command};

use windows::{
    Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_HIDE},
    core::{HSTRING, w},
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
pub const RULE: &str = "T-Bridge";

/// Kural var mı? (`netsh ... show rule` kural yoksa 1 döndürür; yönetici gerekmez.)
pub fn rule_exists() -> bool {
    Command::new("netsh")
        .args(["advfirewall", "firewall", "show", "rule", &format!("name={RULE}")])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Yükseltilmiş netsh ile kuralı ekler (Windows onay penceresi çıkar). Onay verilirse true.
pub fn add_rule_elevated() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let args = format!(
        "advfirewall firewall add rule name=\"{RULE}\" dir=in action=allow protocol=UDP program=\"{}\" profile=private,public,domain enable=yes",
        exe.display()
    );
    let r = unsafe { ShellExecuteW(None, w!("runas"), w!("netsh"), &HSTRING::from(args), None, SW_HIDE) };
    // ShellExecute başarıda > 32 döndürür; reddedilirse (iptal) 5 vb.
    if r.0 as isize <= 32 {
        return false;
    }
    // netsh kısa sürede biter; sonucu doğrula.
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(250));
        if rule_exists() {
            return true;
        }
    }
    false
}

/// Geliştirme derlemesi mi (target klasöründen çalışıyor)? Orada otomatik onay istenmez.
pub fn is_dev_build() -> bool {
    std::env::current_exe()
        .map(|p| p.components().any(|c| c.as_os_str().eq_ignore_ascii_case("target")))
        .unwrap_or(true)
}
