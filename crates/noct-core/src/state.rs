use crate::{AdminSections, Config, Extensions, LoginGates, ModuleInfo, PersonalDataProviders, RateLimiter, Services};
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
    /// Fornitori di dati personali dei moduli attivi (esportazione e cancellazione dell'account).
    pub personal_data: PersonalDataProviders,
    /// Sezioni di amministrazione dei moduli attivi (panoramica e impostazioni di piattaforma).
    pub admin_sections: AdminSections,
    /// Tutti i moduli compilati nel binario (attivi o no).
    pub catalog: Arc<Vec<ModuleInfo>>,
    /// Svegliato quando lo stato desiderato dei moduli cambia: il supervisore del processo lo legge
    /// subito invece di aspettare il prossimo giro.
    pub modules_changed: Arc<tokio::sync::Notify>,
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
            personal_data: PersonalDataProviders::default(),
            admin_sections: AdminSections::default(),
            catalog: Arc::new(Vec::new()),
            modules_changed: Arc::new(tokio::sync::Notify::new()),
            modules: Arc::new(BTreeSet::new()),
        }
    }

    pub fn with_login_gates(mut self, gates: LoginGates) -> Self {
        self.login_gates = gates;
        self
    }

    pub fn with_personal_data(mut self, providers: PersonalDataProviders) -> Self {
        self.personal_data = providers;
        self
    }

    pub fn with_admin_sections(mut self, sections: AdminSections) -> Self {
        self.admin_sections = sections;
        self
    }

    pub fn with_catalog(mut self, catalog: Vec<ModuleInfo>) -> Self {
        self.catalog = Arc::new(catalog);
        self
    }

    pub fn with_modules(mut self, names: impl IntoIterator<Item = String>) -> Self {
        self.modules = Arc::new(names.into_iter().collect());
        self
    }

    /// Valore testuale della configurazione di un modulo attivo (vuoto = non impostato).
    pub fn module_config_str(&self, module: &str, key: &str) -> Option<&str> {
        match self.module_config.get(module)?.get(key)? {
            ConfigValue::String(s) if !s.is_empty() => Some(s),
            _ => None,
        }
    }

    /// Valore booleano della configurazione di un modulo.
    pub fn module_flag(&self, module: &str, key: &str) -> Option<bool> {
        match self.module_config.get(module)?.get(key)? {
            ConfigValue::Bool(b) => Some(*b),
            _ => None,
        }
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
