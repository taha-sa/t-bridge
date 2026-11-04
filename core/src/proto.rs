//! Kablo protokolü: kontrol akışında uzunluk önekli postcard mesajları,
//! fare hareketleri için datagram, dosyalar için tek yönlü akışlar.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::types::*;

pub const PROTO_VERSION: u16 = 1;
pub const ALPN: &[u8] = b"kopru/1";
/// Tek kontrol mesajı üst sınırı (pano 1 MB + pay).
pub const MAX_FRAME: usize = 4 * 1024 * 1024;
/// Pano metni üst sınırı.
pub const MAX_CLIP: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Msg {
    Hello {
        version: u16,
        id: String,
        name: String,
        platform: Platform,
        pair_code: Option<String>,
    },
    Welcome {
        id: String,
        name: String,
        platform: Platform,
    },
    Reject {
        reason: String,
    },
    Clip {
        text: String,
    },
    Battery {
        level: u8,
        charging: bool,
    },
    Notif(NotificationData),
    NotifRemoved {
        key: String,
    },
    AppIcon {
        package: String,
        png: Vec<u8>,
    },
    NowPlaying(Option<NowPlaying>),
    Pc(PcCommand),
    Phone(PhoneCommand),
    /// Gönderen: yeni (veya devam edilecek) dosya teklifi.
    FileOffer {
        id: u64,
        name: String,
        size: u64,
    },
    /// Alıcı: `offset`'ten itibaren gönder.
    FileAccept {
        id: u64,
        offset: u64,
    },
    /// Alıcı: sonuç (hash doğrulandı mı).
    FileResult {
        id: u64,
        ok: bool,
        detail: Option<String>,
    },
    /// Her iki yön: duraklat / iptal.
    FilePause {
        id: u64,
    },
    FileCancel {
        id: u64,
    },
    /// Alıcıdan: duraklatılmış aktarıma devam iste.
    FileResume {
        id: u64,
    },
}

/// Datagram yükü (güvenilmez, sırasız olabilir).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Datagram {
    Move { dx: i16, dy: i16 },
}

pub fn encode(msg: &Msg) -> Result<Vec<u8>> {
    let body = postcard::to_stdvec(msg)?;
    if body.len() > MAX_FRAME {
        bail!("mesaj çok büyük: {}", body.len());
    }
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

pub async fn read_msg(recv: &mut quinn::RecvStream) -> Result<Option<Msg>> {
    let mut len = [0u8; 4];
    match recv.read_exact(&mut len).await {
        Ok(()) => {}
        Err(quinn::ReadExactError::FinishedEarly(0)) => return Ok(None),
        Err(e) => return Err(e).context("çerçeve uzunluğu"),
    }
    let len = u32::from_le_bytes(len) as usize;
    if len > MAX_FRAME {
        bail!("çerçeve çok büyük: {len}");
    }
    let mut buf = vec![0u8; len];
    recv.read_exact(&mut buf).await.context("çerçeve gövdesi")?;
    Ok(Some(postcard::from_bytes(&buf).context("çözümleme")?))
}

pub fn encode_datagram(d: &Datagram) -> Vec<u8> {
    postcard::to_stdvec(d).unwrap_or_default()
}

pub fn decode_datagram(b: &[u8]) -> Option<Datagram> {
    postcard::from_bytes(b).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let m = Msg::Pc(PcCommand::Text { text: "ğüşıöç İ".into() });
        let e = encode(&m).unwrap();
        let len = u32::from_le_bytes(e[..4].try_into().unwrap()) as usize;
        assert_eq!(len, e.len() - 4);
        let back: Msg = postcard::from_bytes(&e[4..]).unwrap();
        assert_eq!(back, m);
        let d = Datagram::Move { dx: -5, dy: 300 };
        assert_eq!(decode_datagram(&encode_datagram(&d)), Some(d));
    }
}
