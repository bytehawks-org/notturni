use crate::{AppState, Background, ConsumerDef, StartupFuture};
use axum::Router;
use noct_manifest::{
    ConfigValue, Issues, Kind, Manifest, Sources, openapi, resolve_config, topological_order, validate_registry,
};
use noct_migrate::{AppliedMigration, Migration, MigrationError, ModuleMigrations};
use serde_json::Value;
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet};

/// Un modulo compilato nel binario, come lo emette `noct-codegen`.
#[derive(Clone, Copy)]
pub struct ModuleDef {
    pub name: &'static str,
    pub module_toml: &'static str,
    pub api_toml: Option<&'static str>,
    pub db_toml: Option<&'static str>,
    pub lock_json: Option<&'static str>,
    pub migrations: &'static [Migration<'static>],
    pub router: fn() -> Router<AppState>,
    /// Consumer di eventi e job dichiarati in `module.toml`, legati ai loro handler.
    pub consumers: fn(AppState) -> Vec<ConsumerDef>,
    /// `startup::run` del modulo (se dichiarato in `[lifecycle]`), altrimenti un no-op.
    pub startup: fn(AppState) -> StartupFuture,
    /// Directory dei sorgenti del modulo (solo per gli strumenti di sviluppo).
    pub source_dir: &'static str,
}

struct Entry {
    def: ModuleDef,
    manifest: Manifest,
}

/// Insieme dei moduli compilati, già validati tra loro, in ordine di dipendenza.
pub struct Registry {
    entries: Vec<Entry>,
}

impl Registry {
    /// Parsa e valida ogni manifest e le relazioni tra moduli; segnala come errore
    /// anche il drift tra `db.toml` e `schema.lock.json`.
    pub fn new(defs: Vec<ModuleDef>) -> Result<Registry, Issues> {
        let mut issues = Issues::default();
        let mut entries = Vec::new();
        for def in defs {
            match Manifest::parse(Sources {
                module: def.module_toml,
                api: def.api_toml,
                db: def.db_toml,
                lock: def.lock_json,
            }) {
                Ok(manifest) => {
                    if let Some(drift) = manifest.drift() {
                        issues.push(
                            manifest.name(),
                            "schema.lock.json",
                            format!("{drift}: eseguire `noct-ctl migrate generate`"),
                        );
                    }
                    entries.push(Entry { def, manifest });
                }
                Err(e) => issues.0.extend(e.0),
            }
        }
        if !issues.is_empty() {
            return Err(issues);
        }
        let refs: Vec<&Manifest> = entries.iter().map(|e| &e.manifest).collect();
        validate_registry(&refs)?;
        let order = topological_order(&refs).expect("cicli già esclusi da validate_registry");
        let mut slots: Vec<Option<Entry>> = entries.into_iter().map(Some).collect();
        let entries = order.into_iter().map(|i| slots[i].take().expect("indice unico")).collect();
        Ok(Registry { entries })
    }

    /// Moduli compilati, in ordine di dipendenza.
    pub fn manifests(&self) -> Vec<&Manifest> {
        self.entries.iter().map(|e| &e.manifest).collect()
    }

    pub fn defs(&self) -> Vec<&ModuleDef> {
        self.entries.iter().map(|e| &e.def).collect()
    }

    /// Indici dei moduli da attivare: i core, più `requested` (o, se `None`, gli
    /// opzionali con `default_enabled`). Ogni dipendenza deve essere attiva.
    pub fn resolve_enabled(&self, requested: Option<&[String]>) -> Result<Vec<usize>, Issues> {
        let mut issues = Issues::default();
        let known: BTreeSet<&str> = self.entries.iter().map(|e| e.manifest.name()).collect();
        if let Some(req) = requested {
            for name in req {
                if !known.contains(name.as_str()) {
                    issues.push(
                        name,
                        "NOCT_MODULES",
                        format!(
                            "modulo sconosciuto (disponibili: {})",
                            known.iter().copied().collect::<Vec<_>>().join(", ")
                        ),
                    );
                }
            }
        }
        let enabled: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                let m = &e.manifest.module.module;
                m.kind == Kind::Core
                    || match requested {
                        Some(req) => req.iter().any(|r| r == &m.name),
                        None => m.default_enabled,
                    }
            })
            .map(|(i, _)| i)
            .collect();
        let names: BTreeSet<&str> = enabled.iter().map(|&i| self.entries[i].manifest.name()).collect();
        for &i in &enabled {
            let m = &self.entries[i].manifest;
            for req in &m.module.module.requires {
                if !names.contains(req.as_str()) {
                    issues.push(m.name(), "module.toml", format!("richiede il modulo `{req}`, che non è attivo"));
                }
            }
        }
        issues.into_result(enabled)
    }

    /// Configurazione risolta (env → default) dei moduli attivi.
    pub fn module_configs(
        &self,
        enabled: &[usize],
        env: &dyn Fn(&str) -> Option<String>,
    ) -> Result<BTreeMap<String, BTreeMap<String, ConfigValue>>, Issues> {
        let mut issues = Issues::default();
        let mut out = BTreeMap::new();
        for &i in enabled {
            let m = &self.entries[i].manifest;
            match resolve_config(&m.module, env) {
                Ok(cfg) => {
                    out.insert(m.name().to_string(), cfg);
                }
                Err(e) => issues.0.extend(e.0),
            }
        }
        issues.into_result(out)
    }

    pub fn migration_sets(&self, enabled: &[usize]) -> Vec<ModuleMigrations<'_>> {
        enabled
            .iter()
            .map(|&i| ModuleMigrations { module: self.entries[i].def.name, migrations: self.entries[i].def.migrations })
            .collect()
    }

    pub async fn migrate(&self, pool: &PgPool, enabled: &[usize]) -> Result<Vec<AppliedMigration>, MigrationError> {
        noct_migrate::apply(pool, &self.migration_sets(enabled)).await
    }

    /// Router dei moduli attivi (senza prefisso `/api/v1`).
    pub fn router(&self, enabled: &[usize]) -> Router<AppState> {
        enabled.iter().fold(Router::new(), |r, &i| r.merge((self.entries[i].def.router)()))
    }

    /// Esegue gli hook di avvio dei moduli attivi, in ordine di dipendenza. Va chiamato
    /// dopo le migrazioni e una sola volta per processo API.
    pub async fn run_startup(&self, enabled: &[usize], state: &AppState) -> Result<(), String> {
        for &i in enabled {
            let def = &self.entries[i].def;
            (def.startup)(state.clone()).await.map_err(|e| format!("avvio del modulo `{}`: {e}", def.name))?;
        }
        Ok(())
    }

    /// Avvia relay dell'outbox e consumer dei moduli attivi (nel processo API o in un worker).
    pub async fn start_background(
        &self,
        enabled: &[usize],
        state: &AppState,
    ) -> Result<Background, noct_bus::BusError> {
        let mut consumers = Vec::new();
        for &i in enabled {
            for def in (self.entries[i].def.consumers)(state.clone()) {
                tracing::info!(module = def.module, consumer = %def.spec.name, subject = %def.spec.subject, "consumer avviato");
                consumers.push(state.services.bus.consume(def.spec, def.handler).await?);
            }
        }
        Ok(Background::start(state, consumers))
    }

    pub fn openapi(&self, enabled: &[usize]) -> Value {
        let manifests: Vec<&Manifest> = enabled.iter().map(|&i| &self.entries[i].manifest).collect();
        openapi(&manifests, "Notturni API", env!("CARGO_PKG_VERSION"))
    }
}
