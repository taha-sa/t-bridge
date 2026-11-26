//! Kalıcı durum: kimlik (sertifika), eşleşmiş cihazlar, giden dosya kuyruğu.
//! Küçük JSON dosyaları; atomik yazım (geçici dosya + yeniden adlandırma).
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::types::{FileSource, PeerInfo};

static SAVE_LOCK: Mutex<()> = Mutex::new(());
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

pub fn load_json<T: DeserializeOwned + Default>(path: &Path) -> T {
    match fs::read(path) {
        Ok(b) => serde_json::from_slice(&b).unwrap_or_else(|e| {
            log::warn!("{} okunamadı ({e}); varsayılan kullanılıyor", path.display());
            T::default()
        }),
        Err(_) => T::default(),
    }
}

pub fn save_json<T: Serialize>(path: &Path, v: &T) -> Result<()> {
    let _g = SAVE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp_name = format!(
        "{}.{}.{}.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("state"),
        std::process::id(),
        seq
    );
    let tmp = path.with_file_name(tmp_name);
    let bytes = serde_json::to_vec_pretty(v)?;
    if let Err(e) = fs::write(&tmp, bytes) {
        let _ = fs::remove_file(&tmp);
        return Err(e).with_context(|| format!("{}", tmp.display()));
    }
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e).with_context(|| format!("{}", path.display()));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub id: String,
    pub cert_der: Vec<u8>,
    pub key_der: Vec<u8>,
}

impl Identity {
    pub fn load_or_create(dir: &Path) -> Result<Self> {
        let path = dir.join("identity.json");
        if let Ok(b) = fs::read(&path)
            && let Ok(id) = serde_json::from_slice::<Identity>(&b) {
                return Ok(id);
            }
        let id = format!("{:016x}", rand::random::<u64>());
        let cert = rcgen::generate_simple_self_signed(vec![format!("kopru-{id}")])?;
        let me = Identity {
            id,
            cert_der: cert.cert.der().to_vec(),
            key_der: cert.signing_key.serialize_der(),
        };
        save_json(&path, &me)?;
        Ok(me)
    }

    pub fn fingerprint(&self) -> String {
        fingerprint(&self.cert_der)
    }
}

pub fn fingerprint(cert_der: &[u8]) -> String {
    blake3::hash(cert_der).to_hex().to_string()
}

/// Giden kuyruk öğesi (kalıcı).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutItem {
    pub id: u64,
    pub peer: String,
    pub src: FileSource,
    pub paused: bool,
    /// Çevrimdışıyken iptal edildi; bağlanınca karşıya bildirilip silinecek.
    #[serde(default)]
    pub cancelled: bool,
}

#[derive(Debug)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir).with_context(|| format!("{}", dir.display()))?;
        Ok(Self { dir })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn peers(&self) -> Vec<PeerInfo> {
        load_json(&self.dir.join("peers.json"))
    }

    pub fn save_peers(&self, p: &[PeerInfo]) -> Result<()> {
        save_json(&self.dir.join("peers.json"), &p)
    }

    pub fn queue(&self) -> Vec<OutItem> {
        load_json(&self.dir.join("queue.json"))
    }

    pub fn save_queue(&self, q: &[OutItem]) -> Result<()> {
        save_json(&self.dir.join("queue.json"), &q)
    }
}
