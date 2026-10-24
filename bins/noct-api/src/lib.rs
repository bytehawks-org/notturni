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

/// Tutto ciò che serve a un processo (API o worker) per partire: moduli validati
/// e attivi, database migrato, servizi di infrastruttura connessi.
pub struct Runtime {
    pub registry: Registry,
    pub enabled: Vec<usize>,
    pub state: AppState,
}

pub async fn bootstrap(env: &dyn Fn(&str) -> Option<String>) -> Result<Runtime> {
    let config = Config::from_env(env).map_err(anyhow::Error::msg)?;
    let registry = Registry::new(noct_bundle::all()).map_err(|e| anyhow::anyhow!("manifest non validi:\n{e}"))?;
    let enabled = registry
        .resolve_enabled(config.modules.as_deref())
        .map_err(|e| anyhow::anyhow!("moduli non attivabili:\n{e}"))?;
    let module_cfg = registry
        .module_configs(&enabled, env)
        .map_err(|e| anyhow::anyhow!("configurazione dei moduli non valida:\n{e}"))?;

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await
        .context("connessione a Postgres")?;
    // le migrazioni dei moduli attivi si applicano all'avvio (advisory lock: sicuro con più repliche)
    if env("NOCT_AUTO_MIGRATE").as_deref() != Some("false") {
        for m in registry.migrate(&pool, &enabled).await.context("migrazioni")? {
            tracing::info!(module = %m.module, version = m.version, name = %m.name, "migrazione applicata");
        }
    }
    let services = Services::connect(&config).await.map_err(anyhow::Error::msg).context("servizi di infrastruttura")?;
    tracing::info!(
        storage = services.storage.backend(),
        cache = services.cache.backend(),
        bus = services.bus.backend(),
        "servizi connessi"
    );
    let names: Vec<_> = registry
        .manifests()
        .into_iter()
        .enumerate()
        .filter(|(i, _)| enabled.contains(i))
        .map(|(_, m)| m.name().to_string())
        .collect();
    tracing::info!(modules = ?names, "moduli attivi");
    let extensions = registry.extensions(&enabled).map_err(anyhow::Error::msg).context("estensioni dei moduli")?;
    let state = AppState::new(pool, config, services, module_cfg)
        .with_extensions(extensions)
        .with_login_gates(registry.login_gates(&enabled))
        .with_personal_data(registry.personal_data(&enabled))
        .with_modules(names.clone());
    // hook di avvio dei moduli (validazione della configurazione, provisioning del Super Admin...)
    registry.run_startup(&enabled, &state).await.map_err(anyhow::Error::msg)?;
    Ok(Runtime { registry, enabled, state })
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
    app.layer(middleware::from_fn(peer_ip)).with_state(state)
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
