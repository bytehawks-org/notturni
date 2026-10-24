use anyhow::{Context, Result};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let rt = noct_api::bootstrap(&|k| std::env::var(k).ok()).await?;
    // con NATS i consumer girano nei processi `noct-worker`; altrimenti qui, nel processo API
    let background = if rt.state.config.worker_inline {
        Some(rt.registry.start_background(&rt.enabled, &rt.state).await.context("avvio dei consumer")?)
    } else {
        None
    };

    let bind = rt.state.config.bind_addr.clone();
    let app = noct_api::app(&rt.registry, &rt.enabled, rt.state.clone());
    let listener = tokio::net::TcpListener::bind(&bind).await.with_context(|| format!("bind su {bind}"))?;
    tracing::info!(%bind, inline_workers = background.is_some(), "in ascolto");
    axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .with_graceful_shutdown(noct_api::shutdown_signal())
        .await?;
    if let Some(b) = background {
        b.stop().await;
    }
    Ok(())
}
