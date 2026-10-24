//! Cache e primitive di coordinamento (contatori a finestra per il rate limit,
//! lock distribuiti, chiavi con scadenza) dietro un trait. Redis per più
//! repliche, memoria per un singolo processo (installazioni `solo`, test).

mod memory;
mod redis_cache;

pub use memory::MemoryCache;
pub use redis_cache::RedisCache;

use async_trait::async_trait;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("errore della cache: {0}")]
    Backend(String),
}

/// Contatore di una finestra fissa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowHit {
    /// Richieste viste nella finestra, questa inclusa.
    pub count: u64,
    /// Tempo residuo prima che la finestra si azzeri.
    pub remaining: Duration,
}

/// Prova di possesso di un lock: serve per rilasciarlo senza toccare quello di altri.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockToken(pub String);

#[async_trait]
pub trait Cache: Send + Sync {
    fn backend(&self) -> &'static str;

    /// Incrementa il contatore `key`; la finestra parte al primo incremento e dura `window`.
    async fn incr_window(&self, key: &str, window: Duration) -> Result<WindowHit, CacheError>;

    async fn get(&self, key: &str) -> Result<Option<String>, CacheError>;
    async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<(), CacheError>;
    async fn delete(&self, key: &str) -> Result<(), CacheError>;

    /// Acquisisce il lock `key` per al più `ttl` (scade da solo se il processo muore).
    /// `None` se è già tenuto da altri.
    async fn try_lock(&self, key: &str, ttl: Duration) -> Result<Option<LockToken>, CacheError>;
    /// Rilascia il lock solo se è ancora il nostro; `false` se scaduto o di altri.
    async fn unlock(&self, key: &str, token: &LockToken) -> Result<bool, CacheError>;

    async fn health(&self) -> Result<(), CacheError>;
}

/// Prefisso comune delle chiavi, per convivere con altre applicazioni sullo stesso Redis.
pub(crate) const NAMESPACE: &str = "noct:";
