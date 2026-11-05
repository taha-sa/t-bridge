//! Güvenlik duvarı: kullanıcı başına kurulumda yönetici izni yok; kural uygulama tarafından
//! bir kez, Windows onayıyla (runas) eklenir. Kural çalışan exe'nin yoluna bağlıdır: adı da yoldan
//! türetilir, böylece başka bir konumdaki exe için açılmış kural "izin var" sanılmaz.
use std::{os::windows::process::CommandExt, path::PathBuf, process::Command};

use windows::{
    Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_HIDE},
    core::{HSTRING, w},
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// Yalnızca ada bakan önceki sürümün kuralı; temizlenir.
const LEGACY_RULE: &str = "T-Bridge";

/// FNV-1a: exe yolundan kısa, kararlı bir kural adı üretir (komut çıktısı ayrıştırmaya gerek kalmaz).
fn rule_name(exe: &str) -> String {
    let mut h: u32 = 0x811c_9dc5;
    for b in exe.to_lowercase().bytes() {
        h = (h ^ b as u32).wrapping_mul(0x0100_0193);
    }
    format!("T-Bridge-{h:08x}")
}

fn exe() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

/// Bu exe için kural var mı? (`netsh ... show rule` kural yoksa hata kodu döndürür; yönetici gerekmez.)
pub fn rule_exists() -> bool {
    let Some(e) = exe() else { return false };
    Command::new("netsh")
        .args(["advfirewall", "firewall", "show", "rule", &format!("name={}", rule_name(&e.to_string_lossy()))])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Yükseltilmiş komutla kuralı ekler (Windows onay penceresi çıkar). Önce bu exe'ye ait eski kuralları
/// siler; Windows'un kendi soru penceresinden kalan "engelle" kuralları izin kuralını ezerdi.
pub fn add_rule_elevated() -> bool {
    let Some(e) = exe() else { return false };
    let path = e.to_string_lossy().to_string();
    let name = rule_name(&path);
    let args = format!(
        "/c \"netsh advfirewall firewall delete rule name=all program=\"{path}\" & netsh advfirewall firewall delete rule name=\"{LEGACY_RULE}\" & netsh advfirewall firewall add rule name=\"{name}\" dir=in action=allow protocol=UDP program=\"{path}\" profile=private,public,domain enable=yes\""
    );
    let r = unsafe { ShellExecuteW(None, w!("runas"), w!("cmd.exe"), &HSTRING::from(args), None, SW_HIDE) };
    // ShellExecute başarıda > 32 döndürür; reddedilirse (iptal) 5 vb.
    if r.0 as isize <= 32 {
        return false;
    }
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
    exe().map(|p| p.components().any(|c| c.as_os_str().eq_ignore_ascii_case("target"))).unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::rule_name;

    #[test]
    fn rule_name_depends_on_path_not_case() {
        assert_eq!(rule_name(r"C:\A\t-bridge.exe"), rule_name(r"c:\a\T-BRIDGE.EXE"));
        assert_ne!(rule_name(r"C:\A\t-bridge.exe"), rule_name(r"C:\B\t-bridge.exe"));
        assert!(rule_name("x").starts_with("T-Bridge-"));
    }
}
