//! Kablo protokolü: kontrol akışında uzunluk önekli postcard mesajları,
//! fare hareketleri için datagram, dosyalar için tek yönlü akışlar.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::types::*;

pub const PROTO_VERSION: u16 = 1;
pub const ALPN: &[u8] = b"kopru/1";
/// Yetenek: bildirime PC'den yanıt (NotifExt / NotifReply / NotifReplyResult). Eski eşler bunu duyurmaz; onlara yeni
/// mesaj tipleri ASLA gönderilmez (bilinmeyen varyant eski sürümde akışı kapatırdı).
pub const CAP_NOTIF_REPLY: &str = "notif-reply";
/// Yetenek: ekran paylaşımı (Screen* mesajları + video akışı). İki taraf da duyurmadıkça kullanılmaz.
pub const CAP_SCREEN: &str = "screen-share";
/// Yetenek: ekran paylaşımı ayarlama/teşhis (ScreenAck / ScreenStats / ScreenPing / ScreenPong + mutlak işaretçi datagramı).
/// Ack, gerçek ağ/alıcı gecikmesini ölçmenin tek yoludur (QUIC yazma çağrısı büyük pencere yüzünden hemen döner).
pub const CAP_SCREEN_TUNE: &str = "screen-tune";
/// Yetenek: telefon kamerası (Cam* mesajları + kamera video akışı). Masaüstü yalnızca Windows 11'de duyurur.
pub const CAP_CAMERA: &str = "camera";
/// Yetenek: T-Suite ekosistem köprüsü (SuiteApps/SuiteEvent/SuiteCmd/SuiteReply). Uygulamaya özgü varyant YOK: yeni uygulamalar otomatik görünür.
pub const CAP_SUITE: &str = "suite";

/// Bu sürümün duyurduğu yetenekler (Hello/Welcome sonuna eklenir; eski eş sondaki baytları yok sayar).
pub fn local_caps() -> Vec<String> {
    let mut v = vec![CAP_NOTIF_REPLY.to_string(), CAP_SCREEN.to_string(), CAP_SUITE.to_string(), CAP_SCREEN_TUNE.to_string()];
    if crate::camera::camera_cap() {
        v.push(CAP_CAMERA.to_string());
    }
    v
}
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
        /// Yetenek listesi (sürüm 1'e geriye uyumlu ek: eski eşte bu alan yoktur).
        caps: Vec<String>,
    },
    Welcome {
        id: String,
        name: String,
        platform: Platform,
        caps: Vec<String>,
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
    // ---- yalnızca `notif-reply` yeteneğini duyuran eşlere gönderilir (sıra DEĞİŞTİRİLMEZ: postcard varyant dizini) ----
    /// Telefon -> PC: bildirimin yanıtlanabilirliği (Notif mesajından sonra).
    NotifExt {
        key: String,
        can_reply: bool,
        /// Hazır yanıt seçenekleri (en çok 3).
        choices: Vec<String>,
    },
    /// PC -> telefon: bildirime yanıt metni.
    NotifReply {
        key: String,
        text: String,
    },
    /// Telefon -> PC: yanıtın sonucu.
    NotifReplyResult {
        key: String,
        ok: bool,
        error: Option<String>,
    },
    // ---- yalnızca `screen-share` yeteneğini duyuran eşlere gönderilir ----
    ScreenStart { monitor: u32, max_w: u32, max_h: u32, fps: u32, bitrate_kbps: u32 },
    ScreenStop,
    ScreenKeyframe,
    ScreenConfig { max_w: u32, max_h: u32, fps: u32, bitrate_kbps: u32 },
    ScreenInfo { monitors: Vec<MonitorInfo>, current: u32, active: bool, error: Option<String> },
    ScreenPointer { x: f32, y: f32, monitor: u32 },
    // ---- yalnızca `camera` yeteneğini duyuran eşlere gönderilir ----
    /// PC -> telefon: istenen kamera biçimi.
    CamStart { w: u32, h: u32, fps: u32, bitrate_kbps: u32 },
    CamStop,
    CamKeyframe,
    /// Telefon -> PC: kamera durumu. facing: 0 arka, 1 ön.
    CamState { active: bool, w: u32, h: u32, fps: u32, facing: u8, mirror: bool, rotation: u16 },
    // ---- yalnızca `suite` yeteneğini duyuran eşlere gönderilir ----
    /// PC -> telefon: T-Suite uygulama listesi (tam görüntü).
    SuiteApps { apps: Vec<SuiteApp> },
    /// PC -> telefon: bir uygulamanın olayı (json = T-Link olay verisi).
    SuiteEvent { app: String, name: String, json: String },
    /// Telefon -> PC: bir uygulamaya komut (req = telefonun verdiği kimlik).
    SuiteCmd { req: u64, app: String, name: String, json: String },
    /// PC -> telefon: komut yanıtı.
    SuiteReply { req: u64, ok: bool, json: String, error: Option<String> },
    // ---- `screen-share` yeteneği: sonradan eklenen isteğe bağlı bildirim (varyant dizini SONA eklenir) ----
    /// PC -> telefon: güvenli masaüstü (UAC / Ctrl+Alt+Del / kilit ekranı) açık; yakalama geçici olarak durdu. `secure=false` = devam.
    /// Tanımayan eş çerçeveyi atlar (decode_msg -> None); yine de yalnızca `screen-share` duyuran eşe gönderilir.
    ScreenNotice { secure: bool },
    // ---- yalnızca `screen-tune` yeteneğini duyuran eşlere gönderilir (varyant dizini SONA eklenir) ----
    /// Telefon -> PC: `pts_us` dahil öncesindeki tüm video karelerini aldım (geri basınç + gecikme ölçümü).
    ScreenAck { pts_us: u64 },
    /// Telefon -> PC: saniyelik alıcı istatistikleri (günlük + uyarlama).
    ScreenStats(ScreenStatsReport),
    /// Saat senkronu: gönderen kendi saatini (`t0_us`, `screen::now_us`) yollar.
    ScreenPing { seq: u32, t0_us: u64 },
    /// Saat senkronu yanıtı: `t0_us` aynen geri, `t1_us` = yanıtlayanın saati.
    ScreenPong { seq: u32, t0_us: u64, t1_us: u64 },
}

/// Sürüm 1 el sıkışması (caps alanından önceki biçim): eski eşten gelen Hello/Welcome çözümlemesi için.
/// Varyant dizinleri `Msg` ile aynıdır (Hello = 0, Welcome = 1).
#[derive(Deserialize)]
enum LegacyHandshake {
    Hello { version: u16, id: String, name: String, platform: Platform, pair_code: Option<String> },
    Welcome { id: String, name: String, platform: Platform },
}

/// Çerçeve gövdesini çözer. Asla hata vermez: eski biçimli el sıkışması caps=[] ile çözülür, tanınmayan çerçeve `None` döner
/// (uzunluk önekli olduğundan atlamak güvenlidir; bağlantı kapanmaz). Tel enum'unda "bilinmeyen" varyantı yoktur.
pub fn decode_msg(buf: &[u8]) -> Option<Msg> {
    if let Ok(m) = postcard::from_bytes::<Msg>(buf) {
        return Some(m);
    }
    match postcard::from_bytes::<LegacyHandshake>(buf) {
        Ok(LegacyHandshake::Hello { version, id, name, platform, pair_code }) => {
            Some(Msg::Hello { version, id, name, platform, pair_code, caps: Vec::new() })
        }
        Ok(LegacyHandshake::Welcome { id, name, platform }) => Some(Msg::Welcome { id, name, platform, caps: Vec::new() }),
        Err(_) => None,
    }
}

/// Datagram yükü (güvenilmez, sırasız olabilir).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Datagram {
    Move { dx: i16, dy: i16 },
    /// Mutlak işaretçi (ekran görüntüleyici; 0..1) — yalnızca `screen-tune` duyuran eşe gönderilir.
    Abs { x: f32, y: f32, monitor: u8 },
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

/// Sıradaki tanınan mesajı okur; tanınmayan çerçeveler (daha yeni eşten) uyarıyla atlanır.
pub async fn read_msg(recv: &mut quinn::RecvStream) -> Result<Option<Msg>> {
    loop {
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
        match decode_msg(&buf) {
            Some(m) => return Ok(Some(m)),
            None => log::warn!("tanınmayan kontrol mesajı atlandı ({} bayt)", buf.len()),
        }
    }
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

    #[test]
    fn screen_tune_messages_and_datagram_roundtrip() {
        let stats = ScreenStatsReport { rx_fps10: 298, render_fps10: 295, decode_ms10: 61, recv_to_render_ms10: 90, e2e_ms: 58, e2e_max_ms: 120, late_drops: 1, input_waits: 2, queue_max: 3, rtt_ms10: 42 };
        for m in [Msg::ScreenAck { pts_us: 77 }, Msg::ScreenStats(stats), Msg::ScreenPing { seq: 3, t0_us: 9 }, Msg::ScreenPong { seq: 3, t0_us: 9, t1_us: 11 }] {
            let e = encode(&m).unwrap();
            assert_eq!(decode_msg(&e[4..]), Some(m));
        }
        let d = Datagram::Abs { x: 0.25, y: 0.75, monitor: 1 };
        assert_eq!(decode_datagram(&encode_datagram(&d)), Some(d));
        // Eski çerçeve (ScreenNotice) hâlâ aynı varyant dizininde
        let n = Msg::ScreenNotice { secure: true };
        assert_eq!(decode_msg(&encode(&n).unwrap()[4..]), Some(n));
        assert!(local_caps().iter().any(|c| c == CAP_SCREEN_TUNE));
    }

    #[test]
    fn notif_reply_roundtrip() {
        for m in [
            Msg::NotifExt { key: "k".into(), can_reply: true, choices: vec!["Tamam".into(), "👍".into()] },
            Msg::NotifReply { key: "k".into(), text: "Merhaba ğü".into() },
            Msg::NotifReplyResult { key: "k".into(), ok: false, error: Some("Bildirim artık geçerli değil".into()) },
            Msg::Hello { version: PROTO_VERSION, id: "a".into(), name: "b".into(), platform: Platform::Android, pair_code: None, caps: local_caps() },
        ] {
            let b = postcard::to_stdvec(&m).unwrap();
            assert_eq!(decode_msg(&b), Some(m));
        }
    }

    #[test]
    fn unknown_variant_is_tolerated_not_fatal() {
        // Gelecekteki bir varyant dizini (çok büyük) -> Unknown, panik/hata yok.
        let b = [0x7Fu8, 1, 2, 3];
        assert_eq!(decode_msg(&b), None);
        assert_eq!(decode_msg(&[]), None);
    }

    #[test]
    fn legacy_handshake_without_caps_decodes_with_empty_caps() {
        #[derive(Serialize)]
        enum Old {
            Hello { version: u16, id: String, name: String, platform: Platform, pair_code: Option<String> },
            Welcome { id: String, name: String, platform: Platform },
        }
        let h = postcard::to_stdvec(&Old::Hello { version: 1, id: "i".into(), name: "n".into(), platform: Platform::Desktop, pair_code: Some("123".into()) }).unwrap();
        match decode_msg(&h).unwrap() {
            Msg::Hello { version, caps, pair_code, .. } => { assert_eq!(version, 1); assert!(caps.is_empty()); assert_eq!(pair_code.as_deref(), Some("123")); }
            o => panic!("{o:?}"),
        }
        let w = postcard::to_stdvec(&Old::Welcome { id: "i".into(), name: "n".into(), platform: Platform::Android }).unwrap();
        assert!(matches!(decode_msg(&w).unwrap(), Msg::Welcome { caps, .. } if caps.is_empty()));
    }

    #[test]
    fn old_peer_ignores_new_handshake_tail() {
        // Yeni Hello'yu (caps ile) ESKİ biçimde okuyan eş, sondaki baytları yok sayarak çözebilmeli.
        let new = Msg::Hello { version: 1, id: "i".into(), name: "n".into(), platform: Platform::Desktop, pair_code: None, caps: local_caps() };
        let b = postcard::to_stdvec(&new).unwrap();
        assert!(matches!(postcard::from_bytes::<LegacyHandshake>(&b), Ok(LegacyHandshake::Hello { version: 1, .. })));
    }

    #[test]
    fn newer_message_is_unknown_for_old_decoder_shape() {
        // Eski bir okuyucunun Msg'si NotifReply'ı tanımaz; ama yeni okuyucu bilinmeyen dizinde Unknown üretir.
        let b = postcard::to_stdvec(&Msg::NotifReply { key: "k".into(), text: "t".into() }).unwrap();
        assert!(matches!(decode_msg(&b).unwrap(), Msg::NotifReply { .. }));
    }

    #[test]
    fn suite_messages_roundtrip() {
        let apps = vec![SuiteApp { id: "t-ramcash".into(), name: "T-RamCash".into(), version: "1.1.0".into(), running: true, update_available: Some("1.2.0".into()) }];
        for m in [
            Msg::SuiteApps { apps },
            Msg::SuiteEvent { app: "t-downloader".into(), name: "download.progress".into(), json: "{\"pct\":42}".into() },
            Msg::SuiteCmd { req: 7, app: "t-downloader".into(), name: "download.add".into(), json: "{\"url\":\"https://x\"}".into() },
            Msg::SuiteReply { req: 7, ok: false, json: String::new(), error: Some("kapalı".into()) },
            Msg::ScreenNotice { secure: true },
        ] {
            let b = postcard::to_stdvec(&m).unwrap();
            assert_eq!(decode_msg(&b), Some(m));
        }
        assert!(local_caps().iter().any(|c| c == CAP_SUITE));
    }
}
