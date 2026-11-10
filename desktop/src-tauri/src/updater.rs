//! Otomatik güncelleme: GitHub Releases'teki `latest.json` denetlenir, imzası (minisign) doğrulanan
//! kurulum dosyası arka planda iner; aktarım yoksa sessiz kurulur ve uygulama kendini yeniden başlatır.
//! Kullanıcı başına kurulum olduğu için Windows yönetici onayı istenmez.
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use kopru_core::TransferState;
use serde::Serialize;
use tauri::{Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::app::{Shared, lk};

const FIRST_CHECK: Duration = Duration::from_secs(30);
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateState {
    /// idle | checking | latest | downloading | ready | installing | error
    pub phase: &'static str,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub progress: f32,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct Updater {
    pub state: Mutex<UpdateState>,
    /// İndirilmiş, kurulmayı bekleyen paket.
    ready: Mutex<Option<(Update, Vec<u8>)>>,
    busy: tokio::sync::Mutex<()>,
}

impl Updater {
    pub fn new() -> Self {
        Self { state: Mutex::new(UpdateState { phase: "idle", ..Default::default() }), ..Default::default() }
    }
}

fn set(sh: &Shared, f: impl FnOnce(&mut UpdateState)) {
    let st = {
        let mut g = lk(&sh.updater.state);
        f(&mut g);
        g.clone()
    };
    let _ = sh.app.emit("kopru", serde_json::json!({ "type": "update", "state": st }));
}

fn transfers_active(sh: &Shared) -> bool {
    sh.node()
        .map(|n| n.transfers().iter().any(|t| matches!(t.state, TransferState::Active | TransferState::Queued)))
        .unwrap_or(false)
}

/// Denetle ve varsa indir. `auto` ise indirilen paket uygun anda kendiliğinden kurulur.
pub async fn check(sh: Arc<Shared>, auto: bool) -> Result<bool, String> {
    let _guard = sh.updater.busy.lock().await;
    if lk(&sh.updater.ready).is_some() {
        if auto {
            install_when_idle(sh.clone()).await;
        }
        return Ok(true);
    }
    set(&sh, |s| {
        s.phase = "checking";
        s.error = None;
    });
    let updater = match sh.app.updater_builder().on_before_exit({
        let sh = sh.clone();
        move || {
            if let Some(n) = sh.node() {
                n.shutdown();
            }
        }
    })
    .build()
    {
        Ok(u) => u,
        Err(e) => return fail(&sh, e.to_string()),
    };
    let update = match updater.check().await {
        Ok(Some(u)) => u,
        Ok(None) => {
            set(&sh, |s| {
                s.phase = "latest";
                s.version = None;
            });
            return Ok(false);
        }
        Err(e) => return fail(&sh, e.to_string()),
    };
    let version = update.version.clone();
    set(&sh, |s| {
        s.phase = "downloading";
        s.version = Some(version.clone());
        s.notes = update.body.clone();
        s.progress = 0.0;
    });
    let mut got = 0usize;
    let mut last = std::time::Instant::now();
    let bytes = match update
        .download(
            |n, total| {
                got += n;
                if last.elapsed() >= Duration::from_millis(300) {
                    last = std::time::Instant::now();
                    let p = total.map(|t| got as f32 / t as f32).unwrap_or(0.0);
                    set(&sh, |s| s.progress = p.min(1.0));
                }
            },
            || {},
        )
        .await
    {
        Ok(b) => b,
        Err(e) => return fail(&sh, e.to_string()),
    };
    *lk(&sh.updater.ready) = Some((update, bytes));
    set(&sh, |s| {
        s.phase = "ready";
        s.progress = 1.0;
    });
    if auto {
        drop(_guard);
        install_when_idle(sh.clone()).await;
    }
    Ok(true)
}

fn fail(sh: &Shared, e: String) -> Result<bool, String> {
    log::warn!("güncelleme: {e}");
    set(sh, |s| {
        s.phase = "error";
        s.error = Some(e.clone());
    });
    Err(e)
}

/// Aktarım bitene kadar bekler, sonra kurar (uygulama kapanıp yeni sürümle açılır).
async fn install_when_idle(sh: Arc<Shared>) {
    while transfers_active(&sh) {
        tokio::time::sleep(Duration::from_secs(20)).await;
    }
    let _ = install_now(&sh);
}

/// İndirilmiş paketi hemen kurar. Başarılı olursa süreç çıkar ve kurulum sonrası yeniden açılır.
pub fn install_now(sh: &Shared) -> Result<(), String> {
    let Some((update, bytes)) = lk(&sh.updater.ready).take() else { return Err(crate::i18n::t("err.update.none")) };
    set(sh, |s| s.phase = "installing");
    // Windows'ta install() kurulum dosyasını başlatıp süreci sonlandırır.
    update.install(&bytes).map_err(|e| {
        let msg = e.to_string();
        let _ = fail(sh, msg.clone());
        msg
    })
}

/// Arka plan döngüsü: açılıştan kısa süre sonra ve 6 saatte bir.
pub fn spawn(sh: Arc<Shared>) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            if lk(&sh.settings).auto_update {
                let _ = check(sh.clone(), true).await;
            }
            tokio::time::sleep(EVERY).await;
        }
    });
}

pub fn state(app: &tauri::AppHandle) -> UpdateState {
    lk(&app.state::<Arc<Shared>>().updater.state).clone()
}
