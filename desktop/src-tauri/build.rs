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
  tauri_build::build()
}
