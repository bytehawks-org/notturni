use crate::{Config, Extensions, RateLimiter, Services};
use noct_manifest::ConfigValue;
use sqlx::PgPool;
use std::{collections::BTreeMap, sync::Arc};

/// Stato condiviso da tutti gli handler.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub services: Services,
    pub limiter: RateLimiter,
    /// Configurazione risolta di ogni modulo attivo: modulo → chiave → valore.
    pub module_config: Arc<BTreeMap<String, BTreeMap<String, ConfigValue>>>,
    /// Estensioni di blog e post dei moduli attivi.
    pub extensions: Extensions,
}

impl AppState {
    pub fn new(
        db: PgPool,
        config: Config,
        services: Services,
        module_config: BTreeMap<String, BTreeMap<String, ConfigValue>>,
    ) -> Self {
        let limiter = RateLimiter::new(services.cache.clone());
        Self {
            db,
            config: Arc::new(config),
            services,
            limiter,
            module_config: Arc::new(module_config),
            extensions: Extensions::default(),
        }
    }

    /// Registra le estensioni dei moduli attivi (vedi [`Registry::extensions`](crate::Registry::extensions)).
    pub fn with_extensions(mut self, extensions: Extensions) -> Self {
        self.extensions = extensions;
        self
    }
}
