use anyhow::{Context, Result};
use noct_api::LiveRouter;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let rt = noct_api::bootstrap(Arc::new(|k| std::env::var(k).ok())).await?;
    // con NATS i consumer girano nei processi `noct-worker`; altrimenti qui, nel processo API
    let inline_workers = rt.state.config.worker_inline;
    let bind = rt.state.config.bind_addr.clone();
    let live = LiveRouter::new(noct_api::app(&rt.registry, &rt.enabled, rt.state.clone()));
    let service = live.service();
    let (stop, stopped) = tokio::sync::watch::channel(false);

    let listener = tokio::net::TcpListener::bind(&bind).await.with_context(|| format!("bind su {bind}"))?;
    tracing::info!(%bind, inline_workers, "in ascolto");
    let server = async move {
        axum::serve(listener, service.into_make_service_with_connect_info::<std::net::SocketAddr>())
            .with_graceful_shutdown(async move {
                noct_api::shutdown_signal().await;
                let _ = stop.send(true);
            })
            .await
            .map_err(anyhow::Error::from)
    };
    // nello stesso task: il supervisore tiene allineati router e consumer ai moduli attivi
    tokio::try_join!(server, noct_api::supervise(rt, Some(live), inline_workers, stopped))?;
    Ok(())
}
