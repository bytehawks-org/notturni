//! Processo di sola elaborazione in background: relay dell'outbox e consumer di
//! eventi e job dei moduli attivi. Stesso binario di configurazione dell'API;
//! se ne possono avviare più repliche (NATS distribuisce i job fra loro).

use anyhow::{Context, Result};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let rt = noct_api::bootstrap(&|k| std::env::var(k).ok()).await?;
    let background = rt.registry.start_background(&rt.enabled, &rt.state).await.context("avvio dei consumer")?;
    tracing::info!(consumers = background.consumer_count(), "worker in esecuzione");
    noct_api::shutdown_signal().await;
    background.stop().await;
    Ok(())
}
