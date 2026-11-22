use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{ConnectInfo, Request, State},
    http::{HeaderValue, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::get,
};
use noct_core::{ApiError, AppState, Config, Registry, Services, StorageBackend};
use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower_http::services::ServeDir;

/// Lettura dell'ambiente, riusata a ogni ricostruzione dopo l'attivazione di un modulo.
pub type EnvFn = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// Ogni quanto un processo controlla se lo stato desiderato dei moduli è cambiato (oltre al risveglio
/// immediato dell'API che ha ricevuto la richiesta).
pub const MODULES_POLL: std::time::Duration = std::time::Duration::from_secs(5);
/// Dopo un cambio non applicabile (es. migrazione fallita) si riprova più di rado.
const RELOAD_RETRY: std::time::Duration = std::time::Duration::from_secs(30);

/// Tutto ciò che serve a un processo (API o worker) per partire: moduli validati
/// e attivi, database migrato, servizi di infrastruttura connessi.
pub struct Runtime {
    pub registry: Arc<Registry>,
    pub enabled: Vec<usize>,
    pub state: AppState,
    env: EnvFn,
    /// Versione di `noct_module_state` applicata da questo processo.
    version: i64,
    retry_at: Option<std::time::Instant>,
}

/// Moduli da attivare secondo lo stato desiderato: i core più gli opzionali con `enabled`. Alla prima
/// installazione la tabella si semina da `NOCT_MODULES` (o dai default dei manifest).
async fn desired_modules(
    registry: &Registry,
    pool: &sqlx::PgPool,
    seed: Option<&[String]>,
) -> Result<(Vec<usize>, i64)> {
    for m in registry.catalog().iter().filter(|m| !m.is_core()) {
        let on = seed.map_or(m.default_enabled, |s| s.iter().any(|n| n == &m.name));
        sqlx::query("INSERT INTO noct_modules (name, enabled) VALUES ($1, $2) ON CONFLICT (name) DO NOTHING")
            .bind(&m.name)
            .bind(on)
            .execute(pool)
            .await
            .context("stato dei moduli")?;
    }
    sqlx::query("INSERT INTO noct_module_state (id) VALUES (1) ON CONFLICT (id) DO NOTHING").execute(pool).await?;
    let version: i64 = sqlx::query_scalar("SELECT version FROM noct_module_state WHERE id = 1").fetch_one(pool).await?;
    let names: Vec<String> = sqlx::query_scalar("SELECT name FROM noct_modules WHERE enabled").fetch_all(pool).await?;
    let known: std::collections::BTreeSet<String> = registry.catalog().into_iter().map(|m| m.name).collect();
    // un modulo che non è più compilato nel binario viene ignorato (e segnalato), non blocca l'avvio
    let (names, unknown): (Vec<_>, Vec<_>) = names.into_iter().partition(|n| known.contains(n));
    if !unknown.is_empty() {
        tracing::warn!(modules = ?unknown, "moduli attivi nel database ma non compilati in questo binario: ignorati");
    }
    let enabled = registry.resolve_enabled(Some(&names)).map_err(|e| anyhow::anyhow!("moduli non attivabili:\n{e}"))?;
    Ok((enabled, version))
}

pub async fn bootstrap(env: EnvFn) -> Result<Runtime> {
    let config = Config::from_env(&|k| env(k)).map_err(anyhow::Error::msg)?;
    let registry =
        Arc::new(Registry::new(noct_bundle::all()).map_err(|e| anyhow::anyhow!("manifest non validi:\n{e}"))?);
    // validazione anticipata della richiesta dell'operatore (nomi sconosciuti, dipendenze)
    if config.modules.is_some() {
        registry
            .resolve_enabled(config.modules.as_deref())
            .map_err(|e| anyhow::anyhow!("moduli non attivabili:\n{e}"))?;
    }

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await
        .context("connessione a Postgres")?;
    let auto_migrate = env("NOCT_AUTO_MIGRATE").as_deref() != Some("false");
    // prima il core (contiene la tabella dello stato dei moduli), poi quanto è voluto
    let core = registry.resolve_enabled(Some(&[])).map_err(|e| anyhow::anyhow!("{e}"))?;
    if auto_migrate {
        registry.migrate(&pool, &core).await.context("migrazioni del core")?;
    }
    let (enabled, version) = desired_modules(&registry, &pool, config.modules.as_deref()).await?;
    if auto_migrate {
        // le migrazioni dei moduli attivi si applicano all'avvio (advisory lock: sicuro con più repliche)
        for m in registry.migrate(&pool, &enabled).await.context("migrazioni")? {
            tracing::info!(module = %m.module, version = m.version, name = %m.name, "migrazione applicata");
        }
    }
    let module_cfg = registry
        .module_configs(&enabled, &|k| env(k))
        .map_err(|e| anyhow::anyhow!("configurazione dei moduli non valida:\n{e}"))?;
    let services = Services::connect(&config).await.map_err(anyhow::Error::msg).context("servizi di infrastruttura")?;
    tracing::info!(
        storage = services.storage.backend(),
        cache = services.cache.backend(),
        bus = services.bus.backend(),
        "servizi connessi"
    );
    tracing::info!(modules = ?registry.names(&enabled), "moduli attivi");
    let state = registry
        .wire(&enabled, AppState::new(pool, config, services, module_cfg))
        .map_err(anyhow::Error::msg)
        .context("collegamento dei moduli")?;
    // hook di avvio dei moduli (validazione della configurazione, provisioning del Super Admin...)
    registry.run_startup(&enabled, &state).await.map_err(anyhow::Error::msg)?;
    Ok(Runtime { registry, enabled, state, env, version, retry_at: None })
}

impl Runtime {
    /// Se l'amministratore ha cambiato i moduli attivi, applica il nuovo stato: migra lo schema,
    /// ricostruisce lo `AppState` e lo restituisce (con l'elenco dei moduli) perché il chiamante
    /// scambi router e consumer. `None` se non è cambiato niente o il cambio non è applicabile
    /// (l'errore finisce in `noct_module_state.last_error` e nei log; si riprova dopo un po').
    pub async fn reload_if_changed(&mut self) -> Result<Option<AppState>> {
        if self.retry_at.is_some_and(|t| t > std::time::Instant::now()) {
            return Ok(None);
        }
        let db = self.state.db.clone();
        let version: i64 =
            sqlx::query_scalar("SELECT version FROM noct_module_state WHERE id = 1").fetch_one(&db).await?;
        if version == self.version {
            return Ok(None);
        }
        match self.apply(&db).await {
            Ok(state) => {
                self.version = version;
                self.retry_at = None;
                Ok(Some(state))
            }
            Err(e) => {
                tracing::error!(error = %format!("{e:#}"), "attivazione dei moduli non riuscita");
                let _ = sqlx::query("UPDATE noct_module_state SET last_error = $1 WHERE id = 1")
                    .bind(format!("{e:#}"))
                    .execute(&db)
                    .await;
                self.retry_at = Some(std::time::Instant::now() + RELOAD_RETRY);
                Ok(None)
            }
        }
    }

    async fn apply(&mut self, db: &sqlx::PgPool) -> Result<AppState> {
        let names: Vec<String> =
            sqlx::query_scalar("SELECT name FROM noct_modules WHERE enabled").fetch_all(db).await?;
        let known: std::collections::BTreeSet<String> = self.registry.catalog().into_iter().map(|m| m.name).collect();
        let names: Vec<String> = names.into_iter().filter(|n| known.contains(n)).collect();
        let enabled =
            self.registry.resolve_enabled(Some(&names)).map_err(|e| anyhow::anyhow!("moduli non attivabili:\n{e}"))?;
        if enabled != self.enabled {
            let env = self.env.clone();
            self.registry.migrate(db, &enabled).await.context("migrazioni")?;
            let module_cfg = self
                .registry
                .module_configs(&enabled, &|k| env(k))
                .map_err(|e| anyhow::anyhow!("configurazione dei moduli non valida:\n{e}"))?;
            let mut base = self.state.clone();
            base.module_config = Arc::new(module_cfg);
            let state = self.registry.wire(&enabled, base).map_err(anyhow::Error::msg)?;
            // gli hook di avvio dei soli moduli nuovi
            let added: Vec<usize> = enabled.iter().copied().filter(|i| !self.enabled.contains(i)).collect();
            self.registry.run_startup(&added, &state).await.map_err(anyhow::Error::msg)?;
            tracing::info!(modules = ?self.registry.names(&enabled), "moduli attivi aggiornati");
            self.enabled = enabled;
            self.state = state;
        }
        sqlx::query("UPDATE noct_module_state SET last_error = NULL WHERE id = 1 AND last_error IS NOT NULL")
            .execute(db)
            .await?;
        Ok(self.state.clone())
    }
}

/// Router sostituibile a caldo: le richieste in corso terminano sul router vecchio, le nuove vanno
/// sul nuovo, senza fermare il listener.
#[derive(Clone)]
pub struct LiveRouter {
    current: Arc<std::sync::RwLock<Router>>,
}

impl LiveRouter {
    pub fn new(router: Router) -> Self {
        Self { current: Arc::new(std::sync::RwLock::new(router)) }
    }

    pub fn swap(&self, router: Router) {
        *self.current.write().unwrap_or_else(|e| e.into_inner()) = router;
    }

    /// Il router da montare nel server: delega al router corrente a ogni richiesta.
    pub fn service(&self) -> Router {
        let live = self.clone();
        Router::new().fallback_service(tower::service_fn(move |req: Request| {
            let router = live.current.read().unwrap_or_else(|e| e.into_inner()).clone();
            async move { tower::ServiceExt::oneshot(router, req).await }
        }))
    }
}

/// Router completo: rotte di piattaforma (`/health`, `/health/ready`, `/openapi.json`)
/// più quelle dei moduli attivi, tutte sotto `/api/v1`; con lo storage locale anche
/// `/storage/<bucket>` per i soli bucket pubblici.
pub fn app(registry: &Registry, enabled: &[usize], state: AppState) -> Router {
    let openapi = Arc::new(registry.openapi(enabled));
    let platform = Router::new()
        .route("/health", get(health))
        .route("/health/ready", get(ready))
        .route("/openapi.json", get(move || async move { Json(Value::clone(&openapi)) }));
    let mut app = Router::new().nest("/api/v1", platform.merge(registry.router(enabled)));
    let storage = &state.config.storage;
    if storage.backend == StorageBackend::Local {
        // il bucket di audit resta privato: non viene mai servito
        for bucket in [&storage.bucket_avatars, &storage.bucket_content] {
            app = app.nest_service(&format!("/storage/{bucket}"), ServeDir::new(storage.local_base_path.join(bucket)));
        }
    }
    let cors = cors_layer(&state.config);
    let inner = app.layer(middleware::from_fn(peer_ip)).layer(cors).with_state(state);
    // `/api/v1/spaces...` è l'alias di `/api/v1/blogs...`: la riscrittura avviene prima del routing
    Router::new().fallback_service(tower::service_fn(move |mut req: Request| {
        rewrite_spaces_alias(&mut req);
        let router = inner.clone();
        async move { tower::ServiceExt::oneshot(router, req).await }
    }))
}

/// Riscrive il prefisso `/api/v1/spaces` in `/api/v1/blogs` (path esatto o seguito da `/`),
/// conservando la query.
fn rewrite_spaces_alias(req: &mut Request) {
    const FROM: &str = "/api/v1/spaces";
    const TO: &str = "/api/v1/blogs";
    let uri = req.uri();
    let Some(rest) = uri.path().strip_prefix(FROM).filter(|r| r.is_empty() || r.starts_with('/')) else {
        return;
    };
    let mut pq = format!("{TO}{rest}");
    if let Some(q) = uri.query() {
        pq.push('?');
        pq.push_str(q);
    }
    let mut parts = uri.clone().into_parts();
    if let Ok(pq) = pq.parse() {
        parts.path_and_query = Some(pq);
        if let Ok(new) = axum::http::Uri::from_parts(parts) {
            *req.uri_mut() = new;
        }
    }
}

/// CORS per il frontend nel browser: origini esatte più l'eventuale regex dei sottodomini, con
/// credenziali (il cookie di sessione viaggia con le richieste). Metodi e header si specchiano dalla
/// richiesta di preflight: con le credenziali il `*` non è ammesso.
fn cors_layer(cfg: &Config) -> tower_http::cors::CorsLayer {
    use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer};
    let exact = cfg.cors_origins.clone();
    let pattern = cfg.cors_origin_regex.as_ref().and_then(|r| regex::Regex::new(&format!("^(?:{r})$")).ok());
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(move |origin: &HeaderValue, _| {
            let Ok(origin) = origin.to_str() else { return false };
            exact.iter().any(|o| o == origin) || pattern.as_ref().is_some_and(|p| p.is_match(origin))
        }))
        .allow_credentials(true)
        .allow_methods(AllowMethods::mirror_request())
        .allow_headers(AllowHeaders::mirror_request())
}

/// Scrive l'IP del peer TCP in un header interno, scartando sempre quello eventualmente
/// inviato dal client: è la base del rate limit e dell'audit quando non c'è un proxy fidato.
async fn peer_ip(mut req: Request, next: Next) -> Response {
    req.headers_mut().remove(noct_core::PEER_IP_HEADER);
    let peer = req.extensions().get::<ConnectInfo<std::net::SocketAddr>>().map(|c| c.0.ip().to_string());
    if let Some(ip) = peer.and_then(|ip| HeaderValue::from_str(&ip).ok()) {
        req.headers_mut().insert(noct_core::PEER_IP_HEADER, ip);
    }
    next.run(req).await
}

/// Liveness: il processo risponde e il database è raggiungibile.
async fn health(State(st): State<AppState>) -> Result<Json<Value>, ApiError> {
    sqlx::query("SELECT 1").execute(&st.db).await?;
    Ok(Json(json!({ "status": "ok" })))
}

/// Readiness: tutti i servizi da cui dipende il traffico. 503 se uno è giù; i
/// dettagli dell'errore vanno nei log, non nella risposta pubblica.
async fn ready(State(st): State<AppState>) -> (StatusCode, Json<Value>) {
    let mut services = serde_json::Map::new();
    let mut all_ok = true;
    let mut record = |name: &str, backend: &str, result: Result<(), String>| {
        if let Err(e) = &result {
            tracing::warn!(service = name, error = %e, "readiness: servizio non disponibile");
            all_ok = false;
        }
        services.insert(
            name.to_string(),
            json!({ "backend": backend, "status": if result.is_ok() { "ok" } else { "error" } }),
        );
    };
    record(
        "database",
        "postgres",
        sqlx::query("SELECT 1").execute(&st.db).await.map(|_| ()).map_err(|e| e.to_string()),
    );
    for (name, backend, result) in st.services.check().await {
        record(name, backend, result);
    }
    let status = if all_ok { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };
    (status, Json(json!({ "status": if all_ok { "ok" } else { "degraded" }, "services": services })))
}

/// Ciclo di vita di un processo dopo l'avvio: tiene allineati router e consumer allo stato desiderato
/// dei moduli finché `shutdown` non scatta, poi ferma i consumer. Lo usano l'API (con un router da
/// scambiare) e il worker (senza).
pub async fn supervise(
    mut rt: Runtime,
    router: Option<LiveRouter>,
    run_background: bool,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<()> {
    let mut background = if run_background {
        Some(rt.registry.start_background(&rt.enabled, &rt.state).await.context("avvio dei consumer")?)
    } else {
        None
    };
    loop {
        let changed = rt.state.modules_changed.clone();
        tokio::select! {
            _ = shutdown.changed() => break,
            _ = changed.notified() => {}
            _ = tokio::time::sleep(MODULES_POLL) => {}
        }
        match rt.reload_if_changed().await {
            Ok(Some(state)) => {
                if let Some(r) = &router {
                    r.swap(app(&rt.registry, &rt.enabled, state.clone()));
                }
                if let Some(old) = background.take() {
                    old.stop().await;
                    background =
                        Some(rt.registry.start_background(&rt.enabled, &state).await.context("riavvio dei consumer")?);
                }
            }
            Ok(None) => {}
            Err(e) => tracing::warn!(error = %format!("{e:#}"), "controllo dei moduli attivi"),
        }
    }
    if let Some(b) = background {
        b.stop().await;
    }
    Ok(())
}

/// Si risolve su SIGINT o SIGTERM (Kubernetes ferma i pod con SIGTERM).
pub async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
    tracing::info!("arresto");
}
