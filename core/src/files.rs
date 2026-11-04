//! Dosya hedefleri: masaüstünde doğrudan dosya sistemi, Android'de Host'tan gelen fd.
use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::types::FileSource;

/// Uzaktan gelen göreli yolu güvenli parçalara ayırır ("..", sürücü, yasak karakter temizlenir).
pub fn sanitize_rel(name: &str) -> Vec<String> {
    let parts: Vec<String> = name
        .split(['/', '\\'])
        .filter(|p| !p.is_empty() && *p != "." && *p != "..")
        .map(|p| {
            let s: String = p
                .chars()
                .map(|c| if c.is_control() || "<>:\"|?*".contains(c) { '_' } else { c })
                .collect();
            let s = s.trim_end_matches(['.', ' ']).trim_start().to_string();
            // Windows'ta ayrılmış adlar.
            let stem = s.split('.').next().unwrap_or("").to_ascii_uppercase();
            let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.as_bytes()[3].is_ascii_digit());
            if s.is_empty() { "_".to_string() } else if reserved { format!("_{s}") } else { s }
        })
        .collect();
    if parts.is_empty() { vec!["dosya".to_string()] } else { parts }
}

/// `dir/rel` yoksa onu, varsa "ad (1).uzantı" biçiminde boş bir yol döndürür.
pub fn unique_path(dir: &Path, rel: &[String]) -> PathBuf {
    let mut base = dir.to_path_buf();
    for p in &rel[..rel.len() - 1] {
        base.push(p);
    }
    let file = &rel[rel.len() - 1];
    let candidate = base.join(file);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = match file.rfind('.') {
        Some(i) if i > 0 => (&file[..i], &file[i..]),
        _ => (file.as_str(), ""),
    };
    (1..)
        .map(|n| base.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .expect("sonsuz aralık")
}

/// Masaüstü: klasörleri özyinelemeli açar; göreli adlar "Klasör/alt/dosya" olur.
pub fn expand_paths(paths: &[String]) -> Result<Vec<FileSource>> {
    let mut out = Vec::new();
    for p in paths {
        let path = PathBuf::from(p);
        let meta = fs::metadata(&path).with_context(|| p.clone())?;
        let base_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "dosya".into());
        if meta.is_dir() {
            walk(&path, &base_name, &mut out)?;
        } else {
            out.push(FileSource { source: p.clone(), name: base_name, size: meta.len() });
        }
    }
    Ok(out)
}

fn walk(dir: &Path, rel: &str, out: &mut Vec<FileSource>) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(dir)?.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let ft = match e.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if ft.is_symlink() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        let child_rel = format!("{rel}/{name}");
        if ft.is_dir() {
            walk(&e.path(), &child_rel, out)?;
        } else if ft.is_file() {
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            out.push(FileSource { source: e.path().to_string_lossy().to_string(), name: child_rel, size });
        }
    }
    Ok(())
}

/// Masaüstü: yarım dosyaların tutulduğu gizli klasör.
pub fn part_path(download_dir: &Path, key: &str) -> PathBuf {
    download_dir.join(".kopru-part").join(format!("{key}.part"))
}

pub fn open_part(download_dir: &Path, key: &str, size: u64) -> Result<(fs::File, u64)> {
    let path = part_path(download_dir, key);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        #[cfg(windows)]
        hide_dir(parent);
    }
    let f = fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&path)?;
    let mut existing = f.metadata()?.len();
    if existing > size {
        f.set_len(0)?;
        existing = 0;
    }
    Ok((f, existing))
}

pub fn finish_part(download_dir: &Path, key: &str, name: &str, ok: bool) -> Result<Option<String>> {
    let part = part_path(download_dir, key);
    if !ok {
        let _ = fs::remove_file(&part);
        return Ok(None);
    }
    let rel = sanitize_rel(name);
    let dest = unique_path(download_dir, &rel);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(&part, &dest).with_context(|| format!("{} → {}", part.display(), dest.display()))?;
    Ok(Some(dest.to_string_lossy().to_string()))
}

#[cfg(windows)]
fn hide_dir(p: &Path) {
    use std::os::windows::ffi::OsStrExt;
    let w: Vec<u16> = p.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe extern "system" {
        fn SetFileAttributesW(name: *const u16, attrs: u32) -> i32;
    }
    // FILE_ATTRIBUTE_HIDDEN
    unsafe {
        SetFileAttributesW(w.as_ptr(), 0x2);
    }
}

#[cfg(unix)]
pub fn file_from_fd(fd: i32) -> Result<fs::File> {
    use std::os::fd::FromRawFd;
    if fd < 0 {
        bail!("geçersiz fd");
    }
    // Sahiplik bize geçer (Kotlin detachFd ile verir).
    Ok(unsafe { fs::File::from_raw_fd(fd) })
}

#[cfg(not(unix))]
pub fn file_from_fd(_fd: i32) -> Result<fs::File> {
    bail!("fd tabanlı dosya bu platformda yok")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize() {
        assert_eq!(sanitize_rel("../../etc/passwd"), vec!["etc", "passwd"]);
        assert_eq!(sanitize_rel("C:\\a\\b.txt"), vec!["C_", "a", "b.txt"]);
        assert_eq!(sanitize_rel("a/CON.txt"), vec!["a", "_CON.txt"]);
        assert_eq!(sanitize_rel("x?.txt. "), vec!["x_.txt"]);
        assert_eq!(sanitize_rel(""), vec!["dosya"]);
    }

    #[test]
    fn unique() {
        let d = tempfile::tempdir().unwrap();
        let rel = vec!["a.txt".to_string()];
        let p1 = unique_path(d.path(), &rel);
        fs::write(&p1, b"x").unwrap();
        let p2 = unique_path(d.path(), &rel);
        assert!(p2.ends_with("a (1).txt"));
    }

    #[test]
    fn expand() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("Klasör");
        fs::create_dir_all(root.join("alt")).unwrap();
        fs::write(root.join("bir.txt"), b"1").unwrap();
        fs::write(root.join("alt/iki.txt"), b"22").unwrap();
        let v = expand_paths(&[root.to_string_lossy().to_string()]).unwrap();
        let names: Vec<_> = v.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["Klasör/alt/iki.txt", "Klasör/bir.txt"]);
        assert_eq!(v[0].size, 2);
    }
}
