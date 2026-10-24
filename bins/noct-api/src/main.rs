use anyhow::{Context, Result};
use noct_core::{AppState, Config, Registry};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let env = |k: &str| std::env::var(k).ok();
    let config = Config::from_env(&env).map_err(anyhow::Error::msg)?;
    let registry = Registry::new(noct_bundle::all()).map_err(|e| anyhow::anyhow!("manifest non validi:\n{e}"))?;
    let enabled = registry
        .resolve_enabled(config.modules.as_deref())
        .map_err(|e| anyhow::anyhow!("moduli non attivabili:\n{e}"))?;
    let module_cfg = registry
        .module_configs(&enabled, &env)
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

    let names: Vec<_> = registry
        .manifests()
        .into_iter()
        .enumerate()
        .filter(|(i, _)| enabled.contains(i))
        .map(|(_, m)| m.name().to_string())
        .collect();
    tracing::info!(modules = ?names, "moduli attivi");

    let bind = config.bind_addr.clone();
    let app = noct_api::app(&registry, &enabled, AppState::new(pool, config, module_cfg));
    let listener = tokio::net::TcpListener::bind(&bind).await.with_context(|| format!("bind su {bind}"))?;
    tracing::info!(%bind, "in ascolto");
    axum::serve(listener, app).with_graceful_shutdown(shutdown()).await?;
    Ok(())
}

async fn shutdown() {
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
