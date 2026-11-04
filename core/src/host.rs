//! Platformun (Kotlin veya masaüstü Rust) çekirdeğe sağladığı arayüz.
use crate::types::{Event, OpenedFd};

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum KopruError {
    #[error("{msg}")]
    Failed { msg: String },
}

impl KopruError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self::Failed { msg: msg.into() }
    }
}

impl From<anyhow::Error> for KopruError {
    fn from(e: anyhow::Error) -> Self {
        Self::new(format!("{e:#}"))
    }
}

impl From<uniffi::UnexpectedUniFFICallbackError> for KopruError {
    fn from(e: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::new(e.reason)
    }
}

/// Platform geri çağrıları. Tüm metotlar çekirdeğin iş parçacıklarından çağrılır;
/// uygulama tarafı hızlı dönmeli (ağır işi kendi iş parçacığına atmalı).
#[uniffi::export(with_foreign)]
pub trait Host: Send + Sync {
    fn on_event(&self, event: Event);

    /// Android: gelen dosya için hedef aç ("rw" fd). `key` aynı aktarım için devam ederken aynıdır.
    fn open_incoming(&self, key: String, name: String, size: u64) -> Result<OpenedFd, KopruError>;

    /// Android: gelen dosya bitti (ok) veya iptal edildi (!ok). Son konumu (görünen) döndürür.
    fn complete_incoming(&self, key: String, ok: bool) -> Option<String>;

    /// Android: giden dosyayı okumak için fd aç ("r").
    fn open_outgoing(&self, source: String) -> Result<i32, KopruError>;
}
