//! Uzaktan ekran görüntüleme oturumu (PC tarafı): izin, ardışık düzen yaşam döngüsü, gösterge (tepsi + açılır hap), mutlak işaretçi.
use std::sync::Arc;

use kopru_core::{MonitorInfo, Node, ScreenCmd};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogResult};

use crate::app::{Shared, lk};
use crate::win::screen::{self, Params, Pipeline, Sink};

pub const PILL: &str = "pill";

#[derive(Default)]
pub struct ScreenState {
    pub pipeline: Option<Pipeline>,
    pub asking: bool,
    pub sharing: bool,
    pub monitor: u32,
    /// Başlatma oturum sayacı (bayat durum bildirimlerini ayıklar).
    pub session: u64,
    /// Hızlı hata bildiren son oturum (pipeline kaydedilmeden önce gelmiş olabilir).
    pub failed: u64,
    /// Onay penceresi dönemi: Stop/kopma artırır; bayat onay yok sayılır.
    pub ask_epoch: u64,
}

impl ScreenState {
    pub fn begin_session(&mut self) -> u64 {
        self.session += 1;
        self.session
    }
    /// Başlatma hatası: true ise çağıran `stop` yapmalı (pipeline zaten kayıtlı).
    pub fn report_failure(&mut self, id: u64) -> bool {
        if self.session != id {
            return false;
        }
        self.failed = id;
        self.pipeline.is_some()
    }
    /// Pipeline kaydedilebilir mi (hata gelmedi ve oturum bayatlamadı)?
    pub fn can_commit(&self, id: u64) -> bool {
        self.session == id && self.failed != id
    }
}

fn shared(app: &AppHandle) -> Arc<Shared> {
    app.state::<Arc<Shared>>().inner().clone()
}

fn monitor_infos() -> Vec<MonitorInfo> {
    screen::monitors()
        .into_iter()
        .map(|m| MonitorInfo { index: m.index, name: m.name, width: m.width, height: m.height, primary: m.primary })
        .collect()
}

fn send_info(sh: &Shared, node: &Node, active: bool, error: Option<String>) {
    let cur = lk(&sh.screen).monitor;
    node.send_screen_info(monitor_infos(), cur, active, error);
}

/// Bağlanınca telefona monitör listesini bildir (telefon seçici için).
pub fn on_connected(app: &AppHandle) {
    let sh = shared(app);
    let Some(node) = sh.node().cloned() else { return };
    if node.peer_supports_screen() && lk(&sh.settings).screen_share {
        send_info(&sh, &node, false, None);
    }
}

pub fn on_disconnected(app: &AppHandle) {
    lk(&shared(app).screen).ask_epoch += 1;
    stop(app, "bağlantı koptu");
}

pub fn on_cmd(app: &AppHandle, cmd: ScreenCmd) {
    let sh = shared(app);
    let Some(node) = sh.node().cloned() else { return };
    match cmd {
        ScreenCmd::Start { monitor, max_w, max_h, fps, bitrate_kbps } => {
            let p = Params {
                monitor,
                max_w: max_w.clamp(320, 3840),
                max_h: max_h.clamp(240, 2160),
                fps: fps.clamp(5, 60),
                bitrate_kbps: bitrate_kbps.clamp(300, 20_000),
            };
            if !lk(&sh.settings).screen_share {
                send_info(&sh, &node, false, Some("Bu bilgisayarda uzaktan ekran görüntüleme kapalı".into()));
                return;
            }
            let Some(peer) = node.current_peer() else { return };
            if lk(&sh.settings).screen_allowed.contains(&peer.id) {
                start_pipeline(app, p);
                return;
            }
            let epoch = {
                let mut st = lk(&sh.screen);
                if st.asking {
                    return;
                }
                st.asking = true;
                st.ask_epoch
            };
            // İlk kez: PC kullanıcısına sor.
            let (app2, peer_id) = (app.clone(), peer.id.clone());
            let peer_id_chk = peer.id.clone();
            app.dialog()
                .message(format!("{} ekranı görmek istiyor.\n\nİzin verirsen telefon bu bilgisayarın ekranını görür ve fareyle klavyeyi kullanabilir.", peer.name))
                .title("T-Bridge · Ekran paylaşımı")
                .buttons(MessageDialogButtons::YesNoCancelCustom("İzin ver".into(), "Reddet".into(), "Her zaman izin ver".into()))
                .show_with_result(move |res| {
                    let sh = shared(&app2);
                    lk(&sh.screen).asking = false;
                    let (ok, always) = match res {
                        MessageDialogResult::Yes => (true, false),
                        MessageDialogResult::Custom(s) => (s == "İzin ver" || s == "Her zaman izin ver", s == "Her zaman izin ver"),
                        _ => (false, false),
                    };
                    if always {
                        let mut s = lk(&sh.settings);
                        if !s.screen_allowed.contains(&peer_id) {
                            s.screen_allowed.push(peer_id);
                        }
                        drop(s);
                        sh.save_settings();
                    }
                    // Onay beklenirken Stop/kopma oldu ya da başka cihaz bağlandı: bayat, başlatma.
                    let stale = lk(&sh.screen).ask_epoch != epoch
                        || sh.node().and_then(|n| n.current_peer()).map(|c| c.id) != Some(peer_id_chk);
                    if ok && stale {
                        log::info!("ekran paylaşımı onayı bayat, yok sayıldı");
                    } else if ok {
                        start_pipeline(&app2, p);
                    } else if let Some(n) = sh.node() {
                        send_info(&sh, n, false, Some("Bilgisayar kullanıcısı reddetti".into()));
                    }
                });
        }
        ScreenCmd::Stop => {
            lk(&sh.screen).ask_epoch += 1;
            stop(app, "telefon durdurdu")
        }
        ScreenCmd::Keyframe => {
            if let Some(p) = lk(&sh.screen).pipeline.as_ref() {
                p.request_key();
            }
        }
        ScreenCmd::Config { max_w, max_h, fps, bitrate_kbps } => {
            if let Some(p) = lk(&sh.screen).pipeline.as_ref() {
                let cur = p.current();
                p.reconfigure(Params {
                    monitor: cur.monitor,
                    max_w: max_w.clamp(320, 3840),
                    max_h: max_h.clamp(240, 2160),
                    fps: fps.clamp(5, 60),
                    bitrate_kbps: bitrate_kbps.clamp(300, 20_000),
                });
            }
        }
        ScreenCmd::Pointer { x, y, monitor } => {
            // Yalnızca paylaşım sürerken (yetkisiz fare kontrolü yok).
            if lk(&sh.screen).sharing {
                crate::win::input::move_abs(monitor, x, y);
            }
        }
    }
}

fn start_pipeline(app: &AppHandle, p: Params) {
    let sh = shared(app);
    let Some(node) = sh.node().cloned() else { return };
    // Önceki oturum varsa kapat.
    let old = lk(&sh.screen).pipeline.take();
    if let Some(mut o) = old {
        o.stop();
    }
    node.end_screen_stream();
    let sid = {
        let mut st = lk(&sh.screen);
        st.monitor = p.monitor;
        st.begin_session()
    };

    let (n1, n2, n3) = (node.clone(), node.clone(), node.clone());
    let app3 = app.clone();
    let sink = Sink {
        send: Box::new(move |pts, key, data| n1.send_screen_frame(pts, key, data)),
        feedback: Box::new(move |pts| {
            let f = n2.screen_feedback(pts);
            (f.want_key, f.backlog_ms)
        }),
        on_status: Box::new(move |r| {
            let sh = shared(&app3);
            match r {
                Ok(desc) => {
                    log::info!("ekran paylaşımı: {desc}");
                    send_info(&sh, &n3, true, None);
                }
                Err(e) => {
                    log::warn!("ekran paylaşımı başlatılamadı: {e}");
                    send_info(&sh, &n3, false, Some(format!("Ekran yakalanamadı: {e}")));
                    // Pipeline henüz kaydedilmediyse start_pipeline `failed` işaretini görüp kaydetmez.
                    if lk(&sh.screen).report_failure(sid) {
                        let app4 = app3.clone();
                        std::thread::spawn(move || stop(&app4, "hata"));
                    }
                }
            }
        }),
    };
    let pipeline = Pipeline::start(p, sink);
    let rejected = {
        let mut st = lk(&sh.screen);
        if st.can_commit(sid) {
            st.pipeline = Some(pipeline);
            st.sharing = true;
            None
        } else {
            Some(pipeline)
        }
    };
    if let Some(mut dead) = rejected {
        // Hızlı hata veya yarışta bayatlama: durumu temiz bırak.
        dead.stop();
        node.end_screen_stream();
        return;
    }
    set_indicator(app, true);
}

pub fn stop(app: &AppHandle, why: &str) {
    let sh = shared(app);
    let (pipeline, was) = {
        let mut st = lk(&sh.screen);
        let was = st.sharing;
        st.sharing = false;
        (st.pipeline.take(), was)
    };
    if !was && pipeline.is_none() {
        return;
    }
    log::info!("ekran paylaşımı durdu: {why}");
    if let Some(mut p) = pipeline {
        p.stop();
    }
    if let Some(n) = sh.node() {
        n.end_screen_stream();
        send_info(&sh, n, false, None);
    }
    set_indicator(app, false);
}

/// Tepsi simgesi rengi + üstte açılır "Ekran paylaşılıyor · Durdur" hapı.
fn set_indicator(app: &AppHandle, on: bool) {
    let sh = shared(app);
    sh.update_tray();
    sh.set_tray_tint(on);
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        if on {
            if app2.get_webview_window(PILL).is_some() {
                return;
            }
            let (w, h) = (264.0, 44.0);
            let x = app2
                .primary_monitor()
                .ok()
                .flatten()
                .map(|m| {
                    let sf = m.scale_factor();
                    m.position().x as f64 / sf + (m.size().width as f64 / sf - w) / 2.0
                })
                .unwrap_or(400.0);
            let _ = WebviewWindowBuilder::new(&app2, PILL, WebviewUrl::App("index.html#pill".into()))
                .title("T-Bridge · Ekran paylaşılıyor")
                .inner_size(w, h)
                .position(x, 12.0)
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .resizable(false)
                .maximizable(false)
                .minimizable(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .focused(false)
                .build();
        } else if let Some(w) = app2.get_webview_window(PILL) {
            let _ = w.close();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::ScreenState;

    #[test]
    fn fast_failure_before_commit_is_not_committed() {
        let mut st = ScreenState::default();
        let id = st.begin_session();
        assert!(!st.report_failure(id)); // pipeline yok -> stop thread gerekmez
        assert!(!st.can_commit(id)); // kaydedilmez, sharing=true olmaz
    }

    #[test]
    fn stale_session_failure_ignored() {
        let mut st = ScreenState::default();
        let old = st.begin_session();
        let new = st.begin_session();
        assert!(!st.report_failure(old));
        assert!(st.can_commit(new));
        assert!(!st.can_commit(old));
    }
}
