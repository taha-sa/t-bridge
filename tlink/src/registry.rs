//! Kayıt defteri: `%LOCALAPPDATA%\T-Suite\apps\<id>.json` (okuma + değişiklik izleme).
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(rename = "exePath")]
    pub exe_path: String,
    #[serde(rename = "pipeName")]
    pub pipe_name: String,
    pub events: Vec<String>,
    pub commands: Vec<String>,
    #[serde(rename = "releasesRepo", skip_serializing_if = "Option::is_none")]
    pub releases_repo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// 0 = çalışmıyor.
    pub pid: i32,
    /// İsteğe bağlı: hub'ın telefona ileteceği olay önekleri (yoksa `events` önekleri).
    #[serde(rename = "forwardPrefixes", skip_serializing_if = "Option::is_none")]
    pub forward_prefixes: Option<Vec<String>>,
    /// İsteğe bağlı: kapalıyken başlatma argümanları (yoksa argümansız).
    #[serde(rename = "launchArgs", skip_serializing_if = "Option::is_none")]
    pub launch_args: Option<Vec<String>>,
}

impl Manifest {
    /// `forwardPrefixes` (boş değilse) yoksa `events` adlarındaki önekler (`download.progress` -> `download.`).
    pub fn effective_forward_prefixes(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let explicit = self.forward_prefixes.as_deref().filter(|v| !v.is_empty());
        let src: Vec<String> = match explicit {
            Some(v) => v.iter().map(|p| p.to_string()).collect(),
            None => self.events.iter().filter_map(|e| e.find('.').map(|i| e[..=i].to_string())).collect(),
        };
        for p in src {
            if !p.is_empty() && !out.contains(&p) {
                out.push(p);
            }
        }
        out
    }

    pub fn effective_launch_args(&self) -> Vec<String> {
        self.launch_args.clone().unwrap_or_default()
    }
}

pub fn default_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("T-Suite").join("apps")
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn read_all(dir: &Path) -> Vec<Manifest> {
    let mut v = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return v };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let Ok(b) = std::fs::read(&p) else { continue };
        if let Ok(mut m) = serde_json::from_slice::<Manifest>(&b) {
            if valid_id(&m.id) {
                if m.pipe_name.is_empty() {
                    m.pipe_name = format!("{}{}", crate::PIPE_PREFIX, m.id);
                }
                v.push(m);
            }
        }
    }
    v.sort_by(|a, b| a.id.cmp(&b.id));
    v
}

/// Atomik yazım (tmp + rename).
pub fn write(dir: &Path, m: &Manifest) -> std::io::Result<()> {
    if !valid_id(&m.id) {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "geçersiz uygulama kimliği"));
    }
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.json", m.id));
    let tmp = dir.join(format!("{}.json.{}.tmp", m.id, std::process::id()));
    std::fs::write(&tmp, serde_json::to_vec_pretty(m).unwrap_or_default())?;
    std::fs::rename(&tmp, path)
}

pub fn remove(dir: &Path, id: &str) {
    if valid_id(id) {
        let _ = std::fs::remove_file(dir.join(format!("{id}.json")));
    }
}

#[cfg(windows)]
pub fn pid_alive(pid: i32) -> bool {
    use windows::Win32::{
        Foundation::CloseHandle,
        System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };
    if pid <= 0 {
        return false;
    }
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid as u32) else { return false };
        let mut code = 0u32;
        let ok = GetExitCodeProcess(h, &mut code).is_ok() && code == 259; // STILL_ACTIVE
        let _ = CloseHandle(h);
        ok
    }
}

#[cfg(not(windows))]
pub fn pid_alive(_pid: i32) -> bool {
    false
}

#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    Added(Manifest),
    Updated(Manifest),
    Removed(String),
}

/// Kayıt klasörü izleyicisi (yoklama): `poll()` son çağrıdan beri olan değişiklikleri döner.
pub struct Watcher {
    dir: PathBuf,
    known: HashMap<String, Manifest>,
}

impl Watcher {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir, known: HashMap::new() }
    }

    pub fn poll(&mut self) -> Vec<Change> {
        let now = read_all(&self.dir);
        let changes = diff(&self.known, &now);
        self.known = now.into_iter().map(|m| (m.id.clone(), m)).collect();
        changes
    }

    pub fn current(&self) -> Vec<Manifest> {
        let mut v: Vec<_> = self.known.values().cloned().collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }
}

pub fn diff(old: &HashMap<String, Manifest>, now: &[Manifest]) -> Vec<Change> {
    let mut out = Vec::new();
    for m in now {
        match old.get(&m.id) {
            None => out.push(Change::Added(m.clone())),
            Some(o) if o != m => out.push(Change::Updated(m.clone())),
            _ => {}
        }
    }
    for id in old.keys() {
        if !now.iter().any(|m| &m.id == id) {
            out.push(Change::Removed(id.clone()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_prefixes_and_args() {
        let mut m = Manifest { events: vec!["download.progress".into(), "download.done".into(), "x.y".into(), "nodot".into()], ..Default::default() };
        assert_eq!(m.effective_forward_prefixes(), vec!["download.".to_string(), "x.".to_string()]);
        assert!(m.effective_launch_args().is_empty());
        m.forward_prefixes = Some(vec!["foo.".into()]);
        m.launch_args = Some(vec!["--tray".into()]);
        assert_eq!(m.effective_forward_prefixes(), vec!["foo.".to_string()]);
        assert_eq!(m.effective_launch_args(), vec!["--tray".to_string()]);
        let j: Manifest = serde_json::from_str(r#"{"id":"a","forwardPrefixes":["p."],"launchArgs":["--q"]}"#).unwrap();
        assert_eq!(j.forward_prefixes, Some(vec!["p.".to_string()]));
        assert_eq!(j.launch_args, Some(vec!["--q".to_string()]));
    }

    /// t-suite C# AppRegistry'nin yazdığı biçim (camelCase, girintili).
    const CSHARP_JSON: &str = r#"{
  "id": "t-downloader",
  "name": "T-Downloader",
  "version": "1.1.0",
  "exePath": "C:\\Apps\\TDownloader.exe",
  "pipeName": "TSuite.t-downloader",
  "events": ["download.progress"],
  "commands": ["download.add"],
  "releasesRepo": "taha-sa/t-downloader-releases",
  "pid": 4242
}"#;

    #[test]
    fn reads_csharp_manifest_and_ignores_bad_files() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("t-downloader.json"), CSHARP_JSON).unwrap();
        std::fs::write(d.path().join("bad.json"), "{bozuk").unwrap();
        std::fs::write(d.path().join("..evil.json"), r#"{"id":"../evil"}"#).unwrap();
        std::fs::write(d.path().join("not-json.txt"), "x").unwrap();
        let all = read_all(d.path());
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].exe_path, r"C:\Apps\TDownloader.exe");
        assert_eq!(all[0].pid, 4242);
        assert_eq!(all[0].commands, vec!["download.add"]);
    }

    #[test]
    fn write_roundtrip_remove_and_watch_diff() {
        let d = tempfile::tempdir().unwrap();
        let mut w = Watcher::new(d.path().to_path_buf());
        assert!(w.poll().is_empty());
        let mut m = Manifest { id: "t-bridge".into(), name: "T-Bridge".into(), version: "1.0.0".into(), pid: 1, ..Default::default() };
        write(d.path(), &m).unwrap();
        assert!(matches!(w.poll().as_slice(), [Change::Added(x)] if x.id == "t-bridge" && x.pipe_name == "TSuite.t-bridge"));
        assert!(w.poll().is_empty());
        m.version = "1.1.0".into();
        write(d.path(), &m).unwrap();
        assert!(matches!(w.poll().as_slice(), [Change::Updated(_)]));
        remove(d.path(), "t-bridge");
        assert_eq!(w.poll(), vec![Change::Removed("t-bridge".into())]);
        assert!(write(d.path(), &Manifest { id: "../x".into(), ..Default::default() }).is_err());
    }
}
