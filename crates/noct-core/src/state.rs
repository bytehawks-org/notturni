use crate::{Config, Extensions, LoginGates, RateLimiter, Services};
use noct_manifest::ConfigValue;
use sqlx::PgPool;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

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
    /// Gate del login dei moduli attivi (sfida MFA...).
    pub login_gates: LoginGates,
    /// Nomi dei moduli attivi: un modulo che legge dati di un altro opzionale (conteggi dei
    /// commenti, filtro per categoria) controlla qui che sia attivo.
    pub modules: Arc<BTreeSet<String>>,
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
            login_gates: LoginGates::default(),
            modules: Arc::new(BTreeSet::new()),
        }
    }

    pub fn with_login_gates(mut self, gates: LoginGates) -> Self {
        self.login_gates = gates;
        self
    }

    pub fn with_modules(mut self, names: impl IntoIterator<Item = String>) -> Self {
        self.modules = Arc::new(names.into_iter().collect());
        self
    }

    pub fn module_enabled(&self, name: &str) -> bool {
        self.modules.contains(name)
    }

    /// Registra le estensioni dei moduli attivi (vedi [`Registry::extensions`](crate::Registry::extensions)).
    pub fn with_extensions(mut self, extensions: Extensions) -> Self {
        self.extensions = extensions;
        self
    }
}
