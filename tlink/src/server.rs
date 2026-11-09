//! T-Link sunucusu (Windows named pipe `\\.\pipe\TSuite.<id>`): çok istemcili, abonelik önekleri, komut işleyicileri,
//! yalnızca geçerli kullanıcıya açık pipe (DACL: kullanıcı SID'i + SYSTEM). Olay kuyruğu istemci başına sınırlıdır (dolunca eski olay atılmaz,
//! YENİ olay atılır; UI/uygulamayı asla bloklamaz).
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ServerOptions;
use tokio::sync::mpsc;

use crate::{Envelope, MAX_LINE};

pub type HandlerFuture = Pin<Box<dyn Future<Output = Result<Value, String>> + Send>>;
pub type Handler = Arc<dyn Fn(Option<Value>) -> HandlerFuture + Send + Sync>;

struct Session {
    tx: mpsc::Sender<Vec<u8>>,
    prefixes: Mutex<Vec<String>>,
}

struct Inner {
    app_id: String,
    version: String,
    events: Vec<String>,
    commands: Vec<String>,
    handlers: Mutex<HashMap<String, Handler>>,
    sessions: Mutex<Vec<Arc<Session>>>,
}

#[derive(Clone)]
pub struct Server {
    inner: Arc<Inner>,
}

impl Server {
    pub fn new(app_id: &str, version: &str, events: &[&str], commands: &[&str]) -> Self {
        Self {
            inner: Arc::new(Inner {
                app_id: app_id.into(),
                version: version.into(),
                events: events.iter().map(|s| s.to_string()).collect(),
                commands: commands.iter().map(|s| s.to_string()).collect(),
                handlers: Mutex::new(HashMap::new()),
                sessions: Mutex::new(Vec::new()),
            }),
        }
    }

    pub fn on_command<F, Fut>(&self, name: &str, f: F)
    where
        F: Fn(Option<Value>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, String>> + Send + 'static,
    {
        let h: Handler = Arc::new(move |d| Box::pin(f(d)));
        self.inner.handlers.lock().unwrap().insert(name.into(), h);
    }

    pub fn has_subscriber(&self, event: &str) -> bool {
        self.inner.sessions.lock().unwrap().iter().any(|s| s.prefixes.lock().unwrap().iter().any(|p| event.starts_with(p.as_str())))
    }

    /// Olayı abone istemcilere yollar (bloklamaz; kuyruğu dolu istemcide atılır).
    pub fn publish(&self, name: &str, data: Option<Value>) {
        let mut e = Envelope::new("event");
        e.name = Some(name.into());
        e.data = data;
        let line = e.to_line();
        for s in self.inner.sessions.lock().unwrap().iter() {
            if s.prefixes.lock().unwrap().iter().any(|p| name.starts_with(p.as_str())) {
                let _ = s.tx.try_send(line.clone());
            }
        }
    }

    /// Kabul döngüsünü başlatır (tokio çalışma zamanı içinden çağrılmalı). İlk pipe oluşturulamazsa hata.
    pub fn start(&self) -> std::io::Result<tokio::task::JoinHandle<()>> {
        let name = crate::pipe_path(&self.inner.app_id);
        let sddl = match crate::winsid::current_user_sid() {
            Some(sid) => format!("D:P(A;;GA;;;{sid})(A;;GA;;;SY)"),
            None => "D:P(A;;GA;;;SY)".to_string(),
        };
        let first = create_pipe(&name, true, &sddl)?;
        let inner = self.inner.clone();
        Ok(tokio::spawn(async move {
            let mut next = Some(first);
            loop {
                let pipe = match next.take() {
                    Some(p) => p,
                    None => match create_pipe(&name, false, &sddl) {
                        Ok(p) => p,
                        Err(e) => {
                            log::warn!("tlink pipe oluşturulamadı: {e}");
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            continue;
                        }
                    },
                };
                if pipe.connect().await.is_err() {
                    continue;
                }
                let inner = inner.clone();
                tokio::spawn(session(inner, pipe));
            }
        }))
    }
}

fn create_pipe(name: &str, first: bool, sddl: &str) -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeServer> {
    use windows::Win32::Security::{Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1}, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
    use windows::core::PCWSTR;
    let wide: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut sd = PSECURITY_DESCRIPTOR::default();
        ConvertStringSecurityDescriptorToSecurityDescriptorW(PCWSTR(wide.as_ptr()), SDDL_REVISION_1, &mut sd, None)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let mut sa = SECURITY_ATTRIBUTES { nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32, lpSecurityDescriptor: sd.0, bInheritHandle: false.into() };
        let mut opts = ServerOptions::new();
        opts.first_pipe_instance(first).reject_remote_clients(true);
        opts.create_with_security_attributes_raw(name, &mut sa as *mut _ as *mut std::ffi::c_void)
    }
}

async fn session(inner: Arc<Inner>, pipe: tokio::net::windows::named_pipe::NamedPipeServer) {
    let (rd, mut wr) = tokio::io::split(pipe);
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(256);
    let me = Arc::new(Session { tx: tx.clone(), prefixes: Mutex::new(Vec::new()) });
    inner.sessions.lock().unwrap().push(me.clone());
    let writer = tokio::spawn(async move {
        while let Some(b) = rx.recv().await {
            if wr.write_all(&b).await.is_err() || wr.flush().await.is_err() {
                break;
            }
        }
    });
    let mut hello = Envelope::new("hello");
    hello.name = Some("hello".into());
    hello.data = Some(json!({ "appId": inner.app_id, "version": inner.version, "protocol": crate::PROTOCOL_VERSION, "events": inner.events, "commands": inner.commands }));
    let _ = tx.send(hello.to_line()).await;

    let mut rd = BufReader::new(rd);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match rd.read_until(b'\n', &mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) if buf.len() > MAX_LINE => break,
            Ok(_) => {}
        }
        let Some(e) = Envelope::parse(&buf) else { continue };
        if e.v < 1 {
            continue;
        }
        match e.typ.as_str() {
            "subscribe" => {
                let p: Vec<String> = e
                    .data
                    .as_ref()
                    .and_then(|d| d.get("prefixes"))
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                *me.prefixes.lock().unwrap() = p;
            }
            "cmd" => {
                let handler = e.name.as_ref().and_then(|n| inner.handlers.lock().unwrap().get(n).cloned());
                let tx = tx.clone();
                tokio::spawn(async move {
                    let result = match handler {
                        Some(h) => h(e.data.clone()).await,
                        None => Err("unknown_command".into()),
                    };
                    let mut r = Envelope::new("reply");
                    r.id = e.id.clone();
                    r.name = e.name.clone();
                    r.data = Some(match result {
                        Ok(v) => json!({ "ok": true, "result": v }),
                        Err(m) => json!({ "ok": false, "error": m }),
                    });
                    let _ = tx.send(r.to_line()).await;
                });
            }
            _ => {} // bilinmeyen tür yok sayılır (ileri uyumluluk)
        }
    }
    inner.sessions.lock().unwrap().retain(|s| !Arc::ptr_eq(s, &me));
    writer.abort();
}
