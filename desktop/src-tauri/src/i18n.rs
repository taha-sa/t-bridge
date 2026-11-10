//! Arka uçtaki kullanıcıya görünen metinler (tepsi, bildirim, pencere başlıkları, arayüze dönen hatalar).
//! Dil, arayüzle aynı ayardan gelir: "system" | "tr" | "en" | "de".

use std::sync::atomic::{AtomicU8, Ordering};

const TR: u8 = 0;
const EN: u8 = 1;
const DE: u8 = 2;

static LANG: AtomicU8 = AtomicU8::new(EN);

/// Metin tablosu: anahtar → [TR, EN, DE]. `{ad}` yer tutucuları `tf` ile doldurulur.
const TABLE: &[(&str, [&str; 3])] = &[
    ("tray.open", ["T-Bridge'i aç", "Open T-Bridge", "T-Bridge öffnen"]),
    ("tray.ring", ["Telefonu çaldır", "Ring phone", "Smartphone klingeln lassen"]),
    ("tray.send", ["Dosya gönder…", "Send files…", "Dateien senden…"]),
    ("tray.quit", ["Çıkış", "Exit", "Beenden"]),
    ("tip.connected", ["T-Bridge — {name} bağlı", "T-Bridge — {name} connected", "T-Bridge — {name} verbunden"]),
    ("tip.connecting", ["T-Bridge — bağlanıyor…", "T-Bridge — connecting…", "T-Bridge — Verbindung wird hergestellt…"]),
    ("tip.idle", ["T-Bridge — telefon bağlı değil", "T-Bridge — phone not connected", "T-Bridge — kein Smartphone verbunden"]),
    ("tip.battery", ["Pil: %{level}", "Battery: {level}%", "Akku: {level} %"]),
    ("toast.lowbat.title", ["Telefonun şarjı azaldı", "Phone battery is low", "Smartphone-Akku fast leer"]),
    ("toast.lowbat.body", ["Pil %{level}", "Battery {level}%", "Akku {level} %"]),
    ("toast.filein", ["Dosya alındı", "File received", "Datei empfangen"]),
    ("dlg.sendfiles", ["Gönderilecek dosyalar", "Files to send", "Zu sendende Dateien"]),
    ("dlg.sendfolder", ["Gönderilecek klasör", "Folder to send", "Zu sendender Ordner"]),
    ("dlg.downloaddir", ["İndirme klasörü", "Download folder", "Download-Ordner"]),
    ("win.consent", ["T-Bridge · Ekran paylaşımı isteği", "T-Bridge · Screen sharing request", "T-Bridge · Anfrage zur Bildschirmfreigabe"]),
    ("win.pill", ["T-Bridge · Ekran paylaşılıyor", "T-Bridge · Sharing screen", "T-Bridge · Bildschirm wird freigegeben"]),
    ("screen.monitor", ["Monitör {n}", "Display {n}", "Monitor {n}"]),
    (
        "screen.off",
        [
            "Bu bilgisayarda uzaktan ekran görüntüleme kapalı. Açmak için bilgisayarda T-Bridge ayarlarına bakın.",
            "Remote screen viewing is turned off on this PC. Turn it on in T-Bridge settings on the PC.",
            "Die Bildschirmfreigabe ist auf diesem PC deaktiviert. Aktivieren Sie sie in den T-Bridge-Einstellungen am PC.",
        ],
    ),
    (
        "screen.denied",
        [
            "Bilgisayar kullanıcısı isteği reddetti. Tekrar denemek için isteği yeniden gönderin.",
            "The PC user denied the request. Send the request again to retry.",
            "Der PC-Benutzer hat die Anfrage abgelehnt. Senden Sie die Anfrage erneut, um es noch einmal zu versuchen.",
        ],
    ),
    (
        "screen.capfail",
        [
            "Ekran yakalanamadı ({err}). Ekran paylaşımını tekrar başlatın.",
            "Could not capture the screen ({err}). Start screen sharing again.",
            "Der Bildschirm konnte nicht erfasst werden ({err}). Starten Sie die Bildschirmfreigabe erneut.",
        ],
    ),
    (
        "err.core",
        [
            "Çekirdek başlatılamadı. Uygulamayı yeniden başlatın.",
            "The core could not start. Restart the app.",
            "Der Kern konnte nicht gestartet werden. Starten Sie die App neu.",
        ],
    ),
    (
        "err.suite",
        [
            "T-Suite henüz hazır değil. Birkaç saniye sonra tekrar deneyin.",
            "T-Suite is not ready yet. Try again in a few seconds.",
            "T-Suite ist noch nicht bereit. Versuchen Sie es in einigen Sekunden erneut.",
        ],
    ),
    (
        "err.reply.len",
        [
            "Yanıt boş veya çok uzun. Yanıtı kısaltıp tekrar deneyin.",
            "The reply is empty or too long. Shorten it and try again.",
            "Die Antwort ist leer oder zu lang. Kürzen Sie die Antwort und versuchen Sie es erneut.",
        ],
    ),
    (
        "err.reply.old",
        [
            "Telefondaki T-Bridge sürümü yanıtı desteklemiyor. T-Bridge'i telefonda güncelleyin.",
            "The T-Bridge version on your phone does not support replies. Update T-Bridge on your phone.",
            "Die T-Bridge-Version auf Ihrem Smartphone unterstützt keine Antworten. Aktualisieren Sie T-Bridge auf dem Smartphone.",
        ],
    ),
    (
        "err.reply.fail",
        [
            "Telefona gönderilemedi çünkü bağlantı yok. Telefonun bağlanmasını bekleyip tekrar deneyin.",
            "Could not send to the phone because it is not connected. Wait for the phone to connect and try again.",
            "Senden an das Smartphone nicht möglich, da keine Verbindung besteht. Warten Sie auf die Verbindung und versuchen Sie es erneut.",
        ],
    ),
    (
        "err.win11",
        [
            "Bu özellik yalnızca Windows 11 için tasarlanmıştır.",
            "This feature is designed only for Windows 11.",
            "Diese Funktion ist nur für Windows 11 konzipiert.",
        ],
    ),
    (
        "err.cam.sid",
        [
            "Kullanıcı kimliği okunamadı. Uygulamayı yeniden başlatıp tekrar deneyin.",
            "Could not read the user ID. Restart the app and try again.",
            "Die Benutzerkennung konnte nicht gelesen werden. Starten Sie die App neu und versuchen Sie es erneut.",
        ],
    ),
    (
        "err.cam.uac",
        [
            "Yönetici izni verilmedi, kamera kurulumu iptal edildi. Tekrar denemek için Etkinleştir'e basıp açılan izin penceresini onaylayın.",
            "Administrator permission was not granted, so camera setup was cancelled. To try again, select Enable and confirm the permission prompt.",
            "Die Administratorberechtigung wurde nicht erteilt, die Kamera-Einrichtung wurde abgebrochen. Wählen Sie zum Wiederholen „Aktivieren“ und bestätigen Sie die Berechtigungsabfrage.",
        ],
    ),
    (
        "err.cam.reg",
        [
            "Kamera kaydı tamamlanamadı (yönetici izni verilmedi veya DLL kopyalanamadı). Etkinleştir'e basıp tekrar deneyin.",
            "Camera registration could not be completed (administrator permission was not granted or the DLL could not be copied). Select Enable and try again.",
            "Die Kameraregistrierung konnte nicht abgeschlossen werden (Administratorberechtigung fehlt oder DLL konnte nicht kopiert werden). Wählen Sie „Aktivieren“ und versuchen Sie es erneut.",
        ],
    ),
    (
        "err.cam.hub",
        [
            "Kamera merkezi çalışmıyor. Uygulamayı yeniden başlatın.",
            "The camera hub is not running. Restart the app.",
            "Der Kamera-Hub läuft nicht. Starten Sie die App neu.",
        ],
    ),
    (
        "err.cam.unreg",
        [
            "Kamera kaydı kaldırılamadı. Tekrar deneyin.",
            "Could not remove the camera registration. Try again.",
            "Die Kameraregistrierung konnte nicht entfernt werden. Versuchen Sie es erneut.",
        ],
    ),
    (
        "err.app.missing",
        [
            "Uygulama kurulu değil. Önce uygulamayı edinin.",
            "The app is not installed. Get the app first.",
            "Die App ist nicht installiert. Holen Sie sich zuerst die App.",
        ],
    ),
    (
        "err.app.nofile",
        [
            "Uygulama dosyası bulunamadı. Uygulamayı yeniden kurun.",
            "The app file was not found. Reinstall the app.",
            "Die App-Datei wurde nicht gefunden. Installieren Sie die App erneut.",
        ],
    ),
    (
        "err.app.noresp",
        [
            "Uygulama açılamadı veya yanıt vermedi. Uygulamayı elle açıp tekrar deneyin.",
            "The app did not open or did not respond. Open it manually and try again.",
            "Die App wurde nicht geöffnet oder reagiert nicht. Öffnen Sie sie manuell und versuchen Sie es erneut.",
        ],
    ),
    (
        "err.app.disc",
        [
            "Uygulamayla bağlantı yok. Uygulamayı açıp tekrar deneyin.",
            "Not connected to the app. Open the app and try again.",
            "Keine Verbindung zur App. Öffnen Sie die App und versuchen Sie es erneut.",
        ],
    ),
    (
        "err.file.missing",
        [
            "Dosya bulunamadı. Dosyanın hâlâ yerinde olduğunu denetleyin.",
            "The file was not found. Check that the file is still in place.",
            "Die Datei wurde nicht gefunden. Prüfen Sie, ob die Datei noch vorhanden ist.",
        ],
    ),
    (
        "err.phone.off",
        [
            "Telefon bağlı değil. Telefonda T-Bridge'i açıp tekrar deneyin.",
            "The phone is not connected. Open T-Bridge on the phone and try again.",
            "Das Smartphone ist nicht verbunden. Öffnen Sie T-Bridge auf dem Smartphone und versuchen Sie es erneut.",
        ],
    ),
    (
        "err.update.none",
        [
            "Hazır bir güncelleme yok. Güncellemeleri yeniden denetleyin.",
            "There is no update ready. Check for updates again.",
            "Es ist kein Update bereit. Suchen Sie erneut nach Updates.",
        ],
    ),
];

fn code(lang: &str) -> u8 {
    match lang {
        "tr" => TR,
        "de" => DE,
        _ => EN,
    }
}

/// Windows kullanıcı yerel ayarından "tr" | "de" | "en".
fn system_lang() -> &'static str {
    #[cfg(windows)]
    {
        use windows::Win32::Globalization::GetUserDefaultLocaleName;
        let mut buf = [0u16; 85];
        let n = unsafe { GetUserDefaultLocaleName(&mut buf) };
        if n > 1 {
            let s = String::from_utf16_lossy(&buf[..(n as usize - 1)]).to_lowercase();
            if s.starts_with("tr") {
                return "tr";
            }
            if s.starts_with("de") {
                return "de";
            }
        }
    }
    "en"
}

/// Ayar değerini ("system" | "tr" | "en" | "de") gerçek dile çözer.
pub fn resolve(pref: &str) -> &'static str {
    match pref {
        "tr" => "tr",
        "en" => "en",
        "de" => "de",
        _ => system_lang(),
    }
}

/// Etkin dili ayara göre ayarlar.
pub fn apply(pref: &str) {
    LANG.store(code(resolve(pref)), Ordering::Relaxed);
}

/// Anahtarın etkin dildeki metni (bulunamazsa anahtarın kendisi).
pub fn t(key: &str) -> String {
    TABLE
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v[LANG.load(Ordering::Relaxed) as usize].to_string())
        .unwrap_or_else(|| key.to_string())
}

/// `t` + `{ad}` yer tutucularını doldurur.
pub fn tf(key: &str, args: &[(&str, &str)]) -> String {
    let mut s = t(key);
    for (k, v) in args {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_complete() {
        for (k, v) in TABLE {
            for (i, s) in v.iter().enumerate() {
                assert!(!s.is_empty(), "{k} dil {i} boş");
                assert!(!s.contains('!'), "{k}: ünlem olmamalı");
            }
            let ph = |s: &str| s.matches('{').count();
            assert!(ph(v[0]) == ph(v[1]) && ph(v[1]) == ph(v[2]), "{k}: yer tutucu sayısı uyuşmuyor");
        }
    }

    #[test]
    fn placeholders_fill() {
        apply("en");
        assert_eq!(tf("tip.battery", &[("level", "50")]), "Battery: 50%");
        apply("de");
        assert_eq!(tf("screen.monitor", &[("n", "2")]), "Monitor 2");
        apply("en");
    }
}
