//! Rust istemci <-> C# LinkServer (t-suite/tools/testhost). TLINK_CS_HOST (tsuite-testhost.exe yolu) ayarlı değilse atlanır.
#![cfg(windows)]
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::json;

#[tokio::test]
async fn rust_client_talks_to_csharp_server() {
    let Ok(exe) = std::env::var("TLINK_CS_HOST") else { return };
    if !std::path::Path::new(&exe).exists() {
        return;
    }
    let id = format!("csint{}", std::process::id());
    let mut child = Command::new(&exe).arg(&id).stdout(Stdio::piped()).spawn().unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    assert_eq!(line.trim(), "READY");

    tokio::time::sleep(Duration::from_millis(500)).await; // C# kabul döngüsü ilk pipe örneğini yaratsın
    let (c, mut ev) = tlink::client::Client::connect(&id).await.expect("C# sunucusuna bağlanılamadı");
    assert_eq!(c.hello["appId"], id);
    c.subscribe(&["tick."]);
    let r = c.cmd("echo", Some(json!({"merhaba": "dünya"})), Duration::from_secs(5)).await.unwrap();
    assert_eq!(r["echo"]["merhaba"], "dünya");
    assert_eq!(c.cmd("yok", None, Duration::from_secs(5)).await.unwrap_err(), "unknown_command");
    let e = tokio::time::timeout(Duration::from_secs(5), ev.recv()).await.unwrap().unwrap();
    assert_eq!(e.name.as_deref(), Some("tick.n"));
    let _ = child.kill();
}
