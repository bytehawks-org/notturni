use crate::{Config, RateLimiter};
use noct_manifest::ConfigValue;
use sqlx::PgPool;
use std::{collections::BTreeMap, sync::Arc};

/// Stato condiviso da tutti gli handler.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub limiter: Arc<RateLimiter>,
    /// Configurazione risolta di ogni modulo attivo: modulo → chiave → valore.
    pub module_config: Arc<BTreeMap<String, BTreeMap<String, ConfigValue>>>,
}

impl AppState {
    pub fn new(db: PgPool, config: Config, module_config: BTreeMap<String, BTreeMap<String, ConfigValue>>) -> Self {
        Self {
            db,
            config: Arc::new(config),
            limiter: Arc::new(RateLimiter::default()),
            module_config: Arc::new(module_config),
        }
    }
}
