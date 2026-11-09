//! Birlikte çalışma testi için: `echo_server <id>`; komut "echo" veriyi geri döner, abonelere 1 sn'de bir "tick.n" olayı yollar.
#[cfg(windows)]
#[tokio::main]
async fn main() {
    use serde_json::json;
    let id = std::env::args().nth(1).unwrap_or_else(|| "tlink-echo".into());
    let srv = tlink::server::Server::new(&id, "1.0.0", &["tick.n"], &["echo"]);
    srv.on_command("echo", |d| async move { Ok(json!({ "echo": d })) });
    let _h = srv.start().expect("pipe");
    println!("READY");
    let mut n = 0;
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        n += 1;
        srv.publish("tick.n", Some(json!({ "n": n })));
    }
}
#[cfg(not(windows))]
fn main() {}
