//! T-Link (T-Suite) Rust uygulaması: docs/PROTOCOL.md v1.
//! - [`Envelope`]: tek satır JSON zarfı (`{v,type,id,name,data,ts}`)
//! - [`registry`]: `%LOCALAPPDATA%\T-Suite\apps\<id>.json` okuyucu + değişiklik izleyici
//! - [`client`]: bağlan / abone ol / zaman aşımlı komut / otomatik yeniden bağlanma
//! - [`server`]: çok istemcili named pipe sunucusu (yalnızca geçerli kullanıcı)
pub mod client;
pub mod registry;
#[cfg(windows)]
pub mod server;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_LINE: usize = 1 << 20;
pub const PIPE_PREFIX: &str = "TSuite.";

pub fn pipe_path(app_id: &str) -> String {
    format!(r"\\.\pipe\{PIPE_PREFIX}{app_id}")
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Tek satırlık JSON zarfı. Bilinmeyen alanlar yok sayılır; boş alanlar yazılmaz (C# `WhenWritingNull` ile aynı).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    #[serde(default = "default_v")]
    pub v: u32,
    #[serde(rename = "type", default)]
    pub typ: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(default)]
    pub ts: i64,
}

fn default_v() -> u32 {
    PROTOCOL_VERSION
}

impl Envelope {
    pub fn new(typ: &str) -> Self {
        Self { v: PROTOCOL_VERSION, typ: typ.into(), ts: now_ms(), ..Default::default() }
    }

    /// Satır (sonunda `\n`) olarak seri hâle getirir.
    pub fn to_line(&self) -> Vec<u8> {
        let mut b = serde_json::to_vec(self).unwrap_or_default();
        b.push(b'\n');
        b
    }

    /// Bozuk satır = None (alıcı yok sayar, bağlantı kopmaz).
    pub fn parse(line: &[u8]) -> Option<Envelope> {
        let line = line.strip_suffix(b"\n").unwrap_or(line);
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        serde_json::from_slice(line).ok()
    }
}

#[cfg(windows)]
pub(crate) mod winsid {
    use windows::Win32::{
        Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree},
        Security::{Authorization::ConvertSidToStringSidW, GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };

    /// Geçerli kullanıcının SID dizgesi.
    pub fn current_user_sid() -> Option<String> {
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
            let _ = LocalFree(Some(HLOCAL(p.0 as *mut _)));
            s
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Her iki taraf (C# ve Rust) testlerinde aynı dosya: t-suite/tests/TSuite.Tests/golden.jsonl ile birebir.
    const GOLDEN: &str = include_str!("../tests/golden.jsonl");

    #[test]
    fn golden_lines_parse_and_roundtrip() {
        let mut n = 0;
        for line in GOLDEN.lines().filter(|l| !l.trim().is_empty()) {
            let e = Envelope::parse(line.as_bytes()).unwrap_or_else(|| panic!("çözülemedi: {line}"));
            assert_eq!(e.v, 1, "{line}");
            let back = Envelope::parse(&e.to_line()).unwrap();
            assert_eq!(back, e, "{line}");
            n += 1;
        }
        assert!(n >= 6);
    }

    #[test]
    fn malformed_and_unknown_fields_are_tolerated() {
        assert!(Envelope::parse(b"{bozuk").is_none());
        let e = Envelope::parse(br#"{"v":1,"type":"event","name":"x.y","extra":123,"ts":5}"#).unwrap();
        assert_eq!(e.name.as_deref(), Some("x.y"));
        let line = Envelope::new("cmd").to_line();
        assert_eq!(*line.last().unwrap(), b'\n');
        assert!(!String::from_utf8_lossy(&line).contains("null"), "boş alan yazılmamalı");
    }

    #[test]
    fn pipe_path_matches_spec() {
        assert_eq!(pipe_path("t-bridge"), r"\\.\pipe\TSuite.t-bridge");
    }
}
