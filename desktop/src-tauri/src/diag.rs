//! Hata ve çökme raporlama (T-Suite `TSuite.Diagnostics` ile aynı davranış): günlük dosyası, panik kancası,
//! kişisel verilerden arındırma (Scrubber) ve önceden doldurulmuş GitHub issue bağlantısı. Hiçbir şey kendiliğinden
//! gönderilmez; rapor yalnızca kullanıcı "Raporu gönder"e basarsa tarayıcıda açılır.
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::Ipv6Addr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use regex::{Captures, Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

pub const RELEASES_REPO: &str = "taha-sa/t-bridge-releases";
const APP_NAME: &str = "T-Bridge";
const LOG_MAX: u64 = 1024 * 1024;
const LOG_KEEP: usize = 3;
const URL_LIMIT: usize = 7000;
const LOG_LINES: usize = 200;

// ---------------------------------------------------------------------------------------------------------------------
// Scrubber
// ---------------------------------------------------------------------------------------------------------------------

struct Patterns {
    url: Regex,
    creds: Regex,
    email: Regex,
    user_path: Regex,
    ipv6: Regex,
    ipv4: Regex,
    bearer: Regex,
    secret_kv: Regex,
    guid: Regex,
    token: Regex,
}

fn ci(p: &str) -> Regex {
    RegexBuilder::new(p).case_insensitive(true).build().expect("geçerli desen")
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        url: ci(r#"https?://[^\s<>"`]+"#),
        creds: ci(r"(https?://)[^/@\s]+@"),
        email: ci(r"\b[A-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b"),
        user_path: ci(r#"(\b[A-Z]:[\\/]Users[\\/])[^\\/\r\n"'<>]+"#),
        // Önceki karakter \w ya da ':' olmamalı (look-behind yerine tüketilen önek).
        ipv6: ci(r"(^|[^\w:])((?:[0-9a-f]{0,4}:){2,}[0-9a-f:.]*(?:%[\w.-]+)?)"),
        ipv4: ci(r"\b(?:\d{1,3}\.){3}\d{1,3}\b"),
        bearer: ci(r"\bBearer\s+[^\s,;]+"),
        secret_kv: ci(
            r#"\b((?:[\w-]*(?:token|key|auth|secret|password)[\w-]*)["']?\s*[:=]\s*)(?:\[secret\]|"[^"\r\n]*"|'[^'\r\n]*'|Bearer\s+[^\s,;]+|[^\s,;"'&}\]]+)"#,
        ),
        guid: ci(r"\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b"),
        token: ci(r"(^|[^\w+/=-])((?:[0-9a-f]{32,}|[A-Za-z0-9_+/-]{40,}={0,2}))([^\w+/=-]|$)"),
    })
}

/// En iyi çaba gizlilik filtresi; kullanıcı yine de göndermeden önce önizlemeyi görür.
pub struct Scrubber {
    identities: Vec<String>,
    mask_guids: bool,
}

impl Scrubber {
    pub fn new(user: &str, machine: &str, mask_guids: bool) -> Self {
        Self { identities: vec![user.to_string(), machine.to_string()], mask_guids }
    }

    pub fn from_env() -> Self {
        let user = std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_default();
        let machine = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_default();
        Self::new(&user, &machine, false)
    }

    pub fn scrub(&self, input: &str) -> String {
        let p = patterns();
        // Kimlik bilgilerini ve sorgu/parça kısmını (zararsız görünenler dahil) at.
        let mut s = p
            .url
            .replace_all(input, |m: &Captures| {
                let url = &m[0];
                let url = match url.find(['?', '#']) {
                    Some(i) => format!("{}[query]", &url[..i]),
                    None => url.to_string(),
                };
                p.creds.replace_all(&url, "${1}[credentials]@").into_owned()
            })
            .into_owned();
        s = p.email.replace_all(&s, "[email]").into_owned();
        s = p.user_path.replace_all(&s, "${1}[user]").into_owned();
        for id in self.identities.iter().filter(|i| !i.trim().is_empty()) {
            let re = ci(&format!(r"(^|[^\p{{L}}\p{{N}}_]){}([^\p{{L}}\p{{N}}_]|$)", regex::escape(id)));
            // Bitişik eşleşmeler komşu karakteri paylaştığı için iki geçiş yapılır.
            for _ in 0..2 {
                s = re.replace_all(&s, "${1}[identity]${2}").into_owned();
            }
        }
        s = p
            .ipv6
            .replace_all(&s, |m: &Captures| {
                let cand = &m[2];
                let bare = cand.split('%').next().unwrap_or(cand);
                if bare.parse::<Ipv6Addr>().is_ok() {
                    return format!("{}[ip]", &m[1]);
                }
                let trimmed = cand.trim_end_matches(['.', ':']);
                if trimmed.split('%').next().unwrap_or(trimmed).parse::<Ipv6Addr>().is_ok() {
                    return format!("{}[ip]{}", &m[1], &cand[trimmed.len()..]);
                }
                m[0].to_string()
            })
            .into_owned();
        s = p.ipv4.replace_all(&s, "[ip]").into_owned();
        s = p.bearer.replace_all(&s, "Bearer [secret]").into_owned();
        s = p.secret_kv.replace_all(&s, "${1}[secret]").into_owned();
        if self.mask_guids {
            s = p.guid.replace_all(&s, "[id]").into_owned();
        }
        for _ in 0..2 {
            s = p.token.replace_all(&s, "${1}[token]${3}").into_owned();
        }
        s
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Rapor biçimi
// ---------------------------------------------------------------------------------------------------------------------

pub struct CrashReport {
    pub version: String,
    pub utc: String,
    pub os: String,
    pub culture: String,
    pub uptime_secs: u64,
    pub exc_type: String,
    pub exc_message: String,
    pub stack: String,
    pub log_lines: Vec<String>,
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace(['\r', '\n'], " ").replace('`', "'")
}

/// Uri.EscapeDataString ile aynı: yalnız A-Z a-z 0-9 - _ . ~ olduğu gibi kalır.
pub fn escape_data(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn prefix(s: &str, chars: usize) -> &str {
    match s.char_indices().nth(chars) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

impl CrashReport {
    fn header(&self, sc: &Scrubber) -> String {
        sc.scrub(&format!(
            "# Crash report\n\n| Environment | Value |\n| --- | --- |\n| App | {APP_NAME} |\n| Version | {} |\n| UTC | {} |\n| OS / build | {} |\n| Runtime | Tauri 2 / Rust |\n| Culture | {} |\n| Uptime | {} |\n\n",
            cell(&self.version),
            cell(&self.utc),
            cell(&self.os),
            cell(&self.culture),
            fmt_uptime(self.uptime_secs),
        ))
    }

    fn content(&self, sc: &Scrubber) -> String {
        let mut t = format!("{}: {}\n{}\n", self.exc_type, self.exc_message, self.stack);
        t.push_str("--- Recent log (up to 200 lines) ---\n");
        for l in &self.log_lines {
            t.push_str(l);
            t.push('\n');
        }
        sc.scrub(&t).replace('`', "'")
    }

    pub fn text(&self, sc: &Scrubber) -> String {
        format!("{}```text\n{}\n```\n", self.header(sc), self.content(sc))
    }

    pub fn title(&self, sc: &Scrubber) -> String {
        let ty = self.exc_type.rsplit(['.', ':']).next().unwrap_or(&self.exc_type);
        let title = sc.scrub(&format!("[{APP_NAME} {}] {ty}: {}", self.version, self.exc_message)).replace(['\r', '\n'], " ");
        prefix(&title, 180).to_string()
    }

    /// Önceden doldurulmuş issue bağlantısı; uzunluk 7000 karakterin altında kalacak biçimde kısaltılır.
    pub fn url(&self, sc: &Scrubber) -> String {
        let start = format!("https://github.com/{RELEASES_REPO}/issues/new?title={}&body=", escape_data(&self.title(sc)));
        let mut header = self.header(sc);
        if escape_data(&header).len() > 2000 {
            header = "# Crash report\n\nEnvironment omitted (too large); see full preview.\n\n".into();
        }
        let content = self.content(sc);
        let total = content.chars().count();
        let build = |n: usize| {
            let cut = prefix(&content, n);
            let note = if n < total { "\n[truncated; full report available via Copy]" } else { "" };
            format!("{start}{}", escape_data(&format!("{header}```text\n{cut}{note}\n```\n")))
        };
        let (mut low, mut high) = (0usize, total);
        while low < high {
            let mid = low + (high - low).div_ceil(2);
            if build(mid).len() < URL_LIMIT {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        build(low)
    }
}

fn fmt_uptime(s: u64) -> String {
    format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}

/// Unix saniyesi → "2026-11-10T12:34:56Z".
pub fn iso_utc(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    // Howard Hinnant "civil_from_days".
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, (rem / 60) % 60, rem % 60)
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ---------------------------------------------------------------------------------------------------------------------
// Günlük dosyası + panik kancası
// ---------------------------------------------------------------------------------------------------------------------

static AUTO: AtomicBool = AtomicBool::new(true);
static START: OnceLock<Instant> = OnceLock::new();
static VERSION: OnceLock<String> = OnceLock::new();
static LOCK: Mutex<()> = Mutex::new(());

/// `%LOCALAPPDATA%\T-Suite\logs\t-bridge`
pub fn log_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("T-Suite").join("logs").join("t-bridge")
}

fn log_path(n: usize) -> PathBuf {
    let name = if n == 0 { "t-bridge.log".to_string() } else { format!("t-bridge.{n}.log") };
    log_dir().join(name)
}

fn pending_path() -> PathBuf {
    log_dir().join("pending-crash.json")
}

fn rotate() {
    if fs::metadata(log_path(0)).map(|m| m.len()).unwrap_or(0) < LOG_MAX {
        return;
    }
    let _ = fs::remove_file(log_path(LOG_KEEP - 1));
    for i in (0..LOG_KEEP - 1).rev() {
        let _ = fs::rename(log_path(i), log_path(i + 1));
    }
}

pub fn write_log(level: &str, msg: &str) {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ = fs::create_dir_all(log_dir());
    rotate();
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(log_path(0)) {
        let _ = writeln!(f, "{} {level:<5} {}", iso_utc(now_secs()), msg.replace(['\r', '\n'], " ⏎ "));
    }
}

struct FileLogger;
impl log::Log for FileLogger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Info
    }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            write_log(r.level().as_str(), &r.args().to_string());
        }
    }
    fn flush(&self) {}
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pending {
    utc: String,
    version: String,
    os: String,
    culture: String,
    uptime_secs: u64,
    exc_type: String,
    exc_message: String,
    stack: String,
}

fn os_string() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

/// Günlüğü ve panik kancasını kurar. `auto`: çökme raporu otomatik hazırlansın mı.
pub fn init(version: &str, auto: bool) {
    let _ = START.set(Instant::now());
    let _ = VERSION.set(version.to_string());
    AUTO.store(auto, Ordering::Relaxed);
    let _ = log::set_logger(&FileLogger).map(|()| log::set_max_level(log::LevelFilter::Info));
    write_log("INFO", &format!("T-Bridge {version} başladı"));
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg = match info.payload().downcast_ref::<&str>() {
            Some(s) => (*s).to_string(),
            None => info.payload().downcast_ref::<String>().cloned().unwrap_or_else(|| "panic".into()),
        };
        let loc = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let stack = format!("   at {loc}\n{}", std::backtrace::Backtrace::force_capture());
        write_log("FATAL", &format!("panic: {msg} ({loc})"));
        if AUTO.load(Ordering::Relaxed) {
            let p = Pending {
                utc: iso_utc(now_secs()),
                version: VERSION.get().cloned().unwrap_or_default(),
                os: os_string(),
                culture: String::new(),
                uptime_secs: START.get().map(|s| s.elapsed().as_secs()).unwrap_or(0),
                exc_type: "panic".into(),
                exc_message: msg,
                stack,
            };
            if let Ok(j) = serde_json::to_string(&p) {
                let _ = fs::create_dir_all(log_dir());
                let _ = fs::write(pending_path(), j);
            }
        }
        prev(info);
    }));
}

pub fn set_auto(on: bool) {
    AUTO.store(on, Ordering::Relaxed);
}

fn recent_log() -> Vec<String> {
    let mut lines = Vec::new();
    for n in (0..LOG_KEEP).rev() {
        if let Ok(t) = fs::read_to_string(log_path(n)) {
            lines.extend(t.lines().map(str::to_string));
        }
    }
    let skip = lines.len().saturating_sub(LOG_LINES);
    lines.split_off(skip)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    /// Arındırılmış tam rapor (önizleme + "Kopyala").
    pub text: String,
    /// Önceden doldurulmuş GitHub issue bağlantısı.
    pub url: String,
    pub title: String,
}

fn preview(r: &CrashReport) -> Preview {
    let sc = Scrubber::from_env();
    Preview { text: r.text(&sc), url: r.url(&sc), title: r.title(&sc) }
}

// ---------------------------------------------------------------------------------------------------------------------
// Komutlar
// ---------------------------------------------------------------------------------------------------------------------

/// Önceki oturumdan kalan çökme raporu varsa önizlemesi.
#[tauri::command]
pub fn crash_pending(s: tauri::State<'_, std::sync::Arc<crate::app::Shared>>) -> Option<Preview> {
    let p: Pending = serde_json::from_str(&fs::read_to_string(pending_path()).ok()?).ok()?;
    let culture = crate::i18n::resolve(&s.settings.lock().unwrap_or_else(|e| e.into_inner()).language).to_string();
    let r = CrashReport {
        version: p.version,
        utc: p.utc,
        os: p.os,
        culture,
        uptime_secs: p.uptime_secs,
        exc_type: p.exc_type,
        exc_message: p.exc_message,
        stack: p.stack,
        log_lines: recent_log(),
    };
    Some(preview(&r))
}

#[tauri::command]
pub fn crash_dismiss() {
    let _ = fs::remove_file(pending_path());
}

/// "Hata raporu gönder…": çökme olmadan, son günlük satırlarından rapor hazırlar.
#[tauri::command]
pub fn report_manual(s: tauri::State<'_, std::sync::Arc<crate::app::Shared>>) -> Preview {
    let culture = crate::i18n::resolve(&s.settings.lock().unwrap_or_else(|e| e.into_inner()).language).to_string();
    let r = CrashReport {
        version: VERSION.get().cloned().unwrap_or_default(),
        utc: iso_utc(now_secs()),
        os: os_string(),
        culture,
        uptime_secs: START.get().map(|s| s.elapsed().as_secs()).unwrap_or(0),
        exc_type: "UserReport".into(),
        exc_message: "Manual report".into(),
        stack: String::new(),
        log_lines: recent_log(),
    };
    preview(&r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sc() -> Scrubber {
        Scrubber::new("taha", "DESKTOP-ABC", false)
    }

    #[test]
    fn url_query_and_credentials_removed() {
        assert_eq!(sc().scrub("GET https://example.com/a/b?token=1&x=2 ok"), "GET https://example.com/a/b[query] ok");
        assert_eq!(sc().scrub("https://u:p@host.io/x#frag"), "https://[credentials]@host.io/x[query]");
    }

    #[test]
    fn email_user_path_identity() {
        let s = sc().scrub(r"mail a.b@c.com in C:\Users\hp\Desktop and user taha on DESKTOP-ABC, tahaX stays");
        assert!(s.contains("[email]"), "{s}");
        assert!(s.contains(r"C:\Users\[user]\Desktop"), "{s}");
        assert!(s.contains("user [identity] on [identity],"), "{s}");
        assert!(s.contains("tahaX stays"), "{s}");
    }

    #[test]
    fn ips_masked() {
        let s = sc().scrub("peer 192.168.1.20 and fe80::1ff:fe23:4567:890a end ::1");
        assert!(!s.contains("192.168") && !s.contains("fe80"), "{s}");
        assert_eq!(s.matches("[ip]").count(), 3, "{s}");
        // IPv6 olmayan zaman damgası bozulmamalı.
        assert_eq!(sc().scrub("at 12:34:56 done"), "at 12:34:56 done");
    }

    #[test]
    fn secrets_masked() {
        assert_eq!(sc().scrub("sent Bearer abc.def-123 now"), "sent Bearer [secret] now");
        assert_eq!(sc().scrub(r#"{"apiKey": "s3cr3t", "n": 1}"#), r#"{"apiKey": [secret], "n": 1}"#);
        assert_eq!(sc().scrub("password=hunter2;x=1"), "password=[secret];x=1");
    }

    #[test]
    fn long_tokens_and_guids() {
        let tok = "A".repeat(45);
        assert_eq!(sc().scrub(&format!("id {tok} end")), "id [token] end");
        assert_eq!(sc().scrub(&format!("h {} z", "ab12".repeat(8))), "h [token] z");
        let g = "123e4567-e89b-12d3-a456-426614174000";
        assert!(sc().scrub(g).contains(g));
        assert_eq!(Scrubber::new("", "", true).scrub(g), "[id]");
        assert_eq!(sc().scrub("short abc123 fine"), "short abc123 fine");
    }

    fn report(stack: String) -> CrashReport {
        CrashReport {
            version: "0.3.1".into(),
            utc: iso_utc(1_700_000_000),
            os: "windows x86_64".into(),
            culture: "tr".into(),
            uptime_secs: 3725,
            exc_type: "panic".into(),
            exc_message: "boom at C:\\Users\\hp\\x.rs".into(),
            stack,
            log_lines: vec!["line one".into(); 5],
        }
    }

    #[test]
    fn iso_and_uptime() {
        assert_eq!(iso_utc(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(fmt_uptime(3725), "01:02:05");
    }

    #[test]
    fn issue_url_format_and_bound() {
        let r = report("frame\n".repeat(5000));
        let u = r.url(&sc());
        assert!(u.starts_with("https://github.com/taha-sa/t-bridge-releases/issues/new?title="), "{u}");
        assert!(u.contains("&body="));
        assert!(u.len() < URL_LIMIT, "{}", u.len());
        assert!(u.contains(&escape_data("[truncated; full report available via Copy]")));
        assert!(!u.contains("hp"), "kullanıcı adı sızdı");
        let small = report(String::new()).url(&sc());
        assert!(!small.contains("truncated"));
    }

    #[test]
    fn escape_matches_uri_escape_data() {
        assert_eq!(escape_data("a b/ç~-_."), "a%20b%2F%C3%A7~-_.");
    }

    #[test]
    fn text_has_header_and_fence() {
        let t = report("x".into()).text(&sc());
        assert!(t.starts_with("# Crash report\n\n| Environment | Value |"));
        assert!(t.contains("| Version | 0.3.1 |") && t.contains("```text") && t.ends_with("```\n"));
        assert!(t.contains(r"C:\Users\[user]"));
    }
}
