//! Telefon kamerası (PC'de sanal web kamerası): Windows 11 kapısı, yetenek bayrağı, video akışı sihirli sayısı.
use std::sync::atomic::{AtomicBool, Ordering};

/// MFCreateVirtualCamera yalnızca Windows 11 (build >= 22000) üzerinde vardır.
pub const MIN_CAMERA_BUILD: u32 = 22000;

/// Telefon -> PC kamera video akışı (tek yönlü QUIC akışı) başlığı; ekran paylaşımı sihirli sayısından farklıdır.
pub const CAM_MAGIC: u64 = 0x4341_4D45_5241_4B31; // "CAMERAK1"

/// Saf kapı mantığı (birim testli): Windows değilse veya build bilinmiyorsa/22000'in altındaysa kamera kapalı.
pub fn camera_allowed(is_windows: bool, build: Option<u32>) -> bool {
    is_windows && build.is_some_and(|b| b >= MIN_CAMERA_BUILD)
}

/// Bu cihazın `camera` yeteneğini duyurup duyurmayacağı. Varsayılan KAPALI: Android açılışta açar,
/// masaüstü yalnızca Windows 11'de açar. Kapalıyken eş bu özelliği "desteklenmiyor" gösterir.
static CAMERA_CAP: AtomicBool = AtomicBool::new(false);

pub fn set_camera_cap(on: bool) {
    CAMERA_CAP.store(on, Ordering::SeqCst);
}

pub fn camera_cap() -> bool {
    CAMERA_CAP.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_gate_with_injected_builds() {
        assert!(!camera_allowed(true, Some(19045)), "Windows 10 22H2 -> kapalı");
        assert!(!camera_allowed(true, Some(21999)));
        assert!(camera_allowed(true, Some(22000)), "Windows 11 21H2 -> açık");
        assert!(camera_allowed(true, Some(26200)), "Windows 11 25H2 -> açık");
        assert!(!camera_allowed(true, None), "build okunamadı -> kapalı");
        assert!(!camera_allowed(false, Some(26200)), "Windows değil -> kapalı");
    }

    #[test]
    fn capability_flag_controls_advertised_caps() {
        set_camera_cap(false);
        assert!(!crate::proto::local_caps().iter().any(|c| c == crate::proto::CAP_CAMERA));
        set_camera_cap(true);
        assert!(crate::proto::local_caps().iter().any(|c| c == crate::proto::CAP_CAMERA));
        set_camera_cap(false);
    }
}
