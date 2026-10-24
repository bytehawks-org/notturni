//! Processo di sola elaborazione in background: relay dell'outbox, pianificatore dei job periodici e
//! consumer di eventi e job dei moduli attivi. Stesso binario di configurazione dell'API; se ne
//! possono avviare più repliche (NATS distribuisce i job fra loro). Segue anche l'attivazione a caldo
//! dei moduli decisa dall'amministrazione.

use anyhow::Result;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let rt = noct_api::bootstrap(Arc::new(|k| std::env::var(k).ok())).await?;
    tracing::info!(modules = rt.enabled.len(), "worker in esecuzione");
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let signal = async {
        noct_api::shutdown_signal().await;
        let _ = stop.send(true);
    };
    let (_, result) = tokio::join!(signal, noct_api::supervise(rt, None, true, stopped));
    result
}
