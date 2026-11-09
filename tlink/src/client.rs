//! T-Link istemcisi (Windows named pipe): bağlan, abone ol, zaman aşımlı komut, otomatik yeniden bağlanma.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};

use crate::{Envelope, MAX_LINE};

type Pending = Arc<Mutex<HashMap<String, oneshot::Sender<Envelope>>>>;

/// Tek bağlantı. `events()` abonelikten gelen olayları verir; bağlantı kopunca kanal kapanır.
pub struct Client {
    tx: mpsc::UnboundedSender<Vec<u8>>,
    pending: Pending,
    next_id: Mutex<u64>,
    pub hello: Value,
}

impl Client {
    #[cfg(windows)]
    pub async fn connect(app_id: &str) -> std::io::Result<(Client, mpsc::UnboundedReceiver<Envelope>)> {
        use tokio::net::windows::named_pipe::ClientOptions;
        let pipe = ClientOptions::new().open(crate::pipe_path(app_id))?;
        let (rd, mut wr) = tokio::io::split(pipe);
        let mut rd = BufReader::new(rd);
        let (tx, mut wrx) = mpsc::unbounded_channel::<Vec<u8>>();
        tokio::spawn(async move {
            while let Some(b) = wrx.recv().await {
                if wr.write_all(&b).await.is_err() || wr.flush().await.is_err() {
                    break;
                }
            }
        });
        // hello (en çok 5 sn)
        let mut hello = Value::Null;
        let mut line = Vec::new();
        let first = tokio::time::timeout(Duration::from_secs(5), rd.read_until(b'\n', &mut line)).await;
        if let Ok(Ok(n)) = first {
            if n > 0 {
                if let Some(e) = Envelope::parse(&line) {
                    if e.typ == "hello" {
                        hello = e.data.unwrap_or(Value::Null);
                    }
                }
            }
        }
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let (etx, erx) = mpsc::unbounded_channel::<Envelope>();
        let p2 = pending.clone();
        tokio::spawn(async move {
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match rd.read_until(b'\n', &mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) if buf.len() > MAX_LINE => break,
                    Ok(_) => {}
                }
                let Some(e) = Envelope::parse(&buf) else { continue };
                match e.typ.as_str() {
                    "reply" => {
                        if let Some(id) = e.id.clone() {
                            if let Some(s) = p2.lock().unwrap().remove(&id) {
                                let _ = s.send(e);
                            }
                        }
                    }
                    "event" => {
                        let _ = etx.send(e);
                    }
                    _ => {} // bilinmeyen tür yok sayılır
                }
            }
            p2.lock().unwrap().clear(); // bekleyen komutlar düşer (alıcı kapanır)
        });
        Ok((Client { tx, pending, next_id: Mutex::new(0), hello }, erx))
    }

    #[cfg(not(windows))]
    pub async fn connect(_app_id: &str) -> std::io::Result<(Client, mpsc::UnboundedReceiver<Envelope>)> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "yalnızca Windows"))
    }

    pub fn subscribe(&self, prefixes: &[&str]) {
        let mut e = Envelope::new("subscribe");
        e.data = Some(json!({ "prefixes": prefixes }));
        let _ = self.tx.send(e.to_line());
    }

    /// Komut gönderir; `{"ok":true,"result":..}` -> Ok(result), `{"ok":false,"error":..}` -> Err(error). Zaman aşımı -> Err("zaman aşımı").
    pub async fn cmd(&self, name: &str, data: Option<Value>, timeout: Duration) -> Result<Value, String> {
        let id = {
            let mut n = self.next_id.lock().unwrap();
            *n += 1;
            n.to_string()
        };
        let (s, r) = oneshot::channel();
        self.pending.lock().unwrap().insert(id.clone(), s);
        let mut e = Envelope::new("cmd");
        e.id = Some(id.clone());
        e.name = Some(name.into());
        e.data = data;
        if self.tx.send(e.to_line()).is_err() {
            self.pending.lock().unwrap().remove(&id);
            return Err("bağlantı kapalı".into());
        }
        match tokio::time::timeout(timeout, r).await {
            Ok(Ok(reply)) => {
                let d = reply.data.unwrap_or(Value::Null);
                if d.get("ok").and_then(Value::as_bool) == Some(true) {
                    Ok(d.get("result").cloned().unwrap_or(Value::Null))
                } else {
                    Err(d.get("error").and_then(Value::as_str).unwrap_or("hata").to_string())
                }
            }
            Ok(Err(_)) => Err("bağlantı koptu".into()),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err("zaman aşımı".into())
            }
        }
    }

    pub fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }
}

/// Otomatik yeniden bağlanan istemci: olaylar `(app_id, Envelope)` olarak tek kanala akar.
pub struct ReconnectingClient {
    pub app_id: String,
    current: Arc<Mutex<Option<Arc<Client>>>>,
    task: tokio::task::JoinHandle<()>,
}

impl ReconnectingClient {
    pub fn spawn(app_id: &str, prefixes: Vec<String>, events: mpsc::UnboundedSender<(String, Envelope)>, retry: Duration) -> Self {
        let current: Arc<Mutex<Option<Arc<Client>>>> = Arc::new(Mutex::new(None));
        let (cur, id) = (current.clone(), app_id.to_string());
        let task = tokio::spawn(async move {
            loop {
                if let Ok((c, mut rx)) = Client::connect(&id).await {
                    let c = Arc::new(c);
                    let p: Vec<&str> = prefixes.iter().map(String::as_str).collect();
                    c.subscribe(&p);
                    *cur.lock().unwrap() = Some(c);
                    while let Some(e) = rx.recv().await {
                        if events.send((id.clone(), e)).is_err() {
                            return;
                        }
                    }
                    *cur.lock().unwrap() = None;
                }
                tokio::time::sleep(retry).await;
            }
        });
        Self { app_id: app_id.into(), current, task }
    }

    /// Geçerli bağlantı (komutu kilit dışında beklemek için).
    pub fn handle(&self) -> Option<Arc<Client>> {
        self.current.lock().unwrap().clone().filter(|c| !c.is_closed())
    }

    pub fn is_connected(&self) -> bool {
        self.current.lock().unwrap().as_ref().is_some_and(|c| !c.is_closed())
    }

    pub async fn cmd(&self, name: &str, data: Option<Value>, timeout: Duration) -> Result<Value, String> {
        let c = self.current.lock().unwrap().clone().ok_or_else(|| "bağlı değil".to_string())?;
        c.cmd(name, data, timeout).await
    }
}

impl Drop for ReconnectingClient {
    fn drop(&mut self) {
        self.task.abort();
    }
}
