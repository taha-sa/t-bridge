//! Rust istemci <-> Rust sunucu (gerçek named pipe) uçtan uca.
#![cfg(windows)]
use std::time::Duration;

use serde_json::json;
use tlink::{client::Client, server::Server};

fn uniq() -> String {
    format!("tltest{}", std::process::id() as u64 * 1000 + (tlink::now_ms() as u64 % 1000))
}

#[tokio::test]
async fn command_reply_subscription_and_unknown() {
    let id = uniq();
    let srv = Server::new(&id, "1.0.0", &["a.x", "b.y"], &["echo"]);
    srv.on_command("echo", |d| async move { Ok(json!({ "got": d })) });
    srv.on_command("fail", |_| async { Err("kötü".to_string()) });
    let _h = srv.start().unwrap();

    let (c, mut ev) = Client::connect(&id).await.unwrap();
    assert_eq!(c.hello["appId"], id);
    c.subscribe(&["a."]);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let r = c.cmd("echo", Some(json!({"n": 1})), Duration::from_secs(3)).await.unwrap();
    assert_eq!(r["got"]["n"], 1);
    assert_eq!(c.cmd("fail", None, Duration::from_secs(3)).await.unwrap_err(), "kötü");
    assert_eq!(c.cmd("yok", None, Duration::from_secs(3)).await.unwrap_err(), "unknown_command");

    assert!(srv.has_subscriber("a.x") && !srv.has_subscriber("b.y"));
    srv.publish("b.y", Some(json!(1))); // abone değil: gelmemeli
    srv.publish("a.x", Some(json!({"v": 2})));
    let e = tokio::time::timeout(Duration::from_secs(3), ev.recv()).await.unwrap().unwrap();
    assert_eq!(e.name.as_deref(), Some("a.x"));
    assert_eq!(e.data.unwrap()["v"], 2);
}

#[tokio::test]
async fn command_times_out_when_handler_hangs() {
    let id = format!("{}h", uniq());
    let srv = Server::new(&id, "1.0.0", &[], &["slow"]);
    srv.on_command("slow", |_| async {
        tokio::time::sleep(Duration::from_secs(5)).await;
        Ok(json!(null))
    });
    let _h = srv.start().unwrap();
    let (c, _ev) = Client::connect(&id).await.unwrap();
    assert_eq!(c.cmd("slow", None, Duration::from_millis(300)).await.unwrap_err(), "zaman aşımı");
}
