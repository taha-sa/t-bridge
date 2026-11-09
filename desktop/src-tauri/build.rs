fn main() {
  // Tauri `bundle.resources` girdisi (camera/tbridge_vcam.dll) derleme sırasında var olmalı. Geliştirme derlemelerinde gerçek
  // DLL henüz yoksa boş bir yer tutucu bırakılır (çalışma zamanında 4 KB altı dosya "yok" sayılır). Sürüm derlemesinde
  // beforeBuildCommand (scripts/build-vcam.mjs) gerçek DLL'i bu yola yazar.
  let dll = std::path::Path::new("camera").join("tbridge_vcam.dll");
  if !dll.exists() {
    let _ = std::fs::create_dir_all("camera");
    let _ = std::fs::write(&dll, b"");
  }
  println!("cargo:rerun-if-changed=camera/tbridge_vcam.dll");
  // Yükseltilmiş --camera-register yardımcısı yalnızca bu SHA-256'ya uyan DLL'i kaydeder. Yer tutucu (<=4 KB) için boş bırakılır:
  // sürüm derlemesinde bu, kaydı reddeder (yalnız debug derlemelerde boş karma kabul edilir).
  {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(&dll).unwrap_or_default();
    let hex = if bytes.len() > 4096 { Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect::<String>() } else { String::new() };
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("vcam_sha256.txt");
    std::fs::write(out, hex).unwrap();
  }
  tauri_build::build()
}
