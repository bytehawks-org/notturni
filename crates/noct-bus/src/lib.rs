//! Bus di eventi e code di lavoro dietro un trait.
//!
//! * **Eventi** (`post.published`): fan-out, ogni consumer riceve i messaggi
//!   pubblicati dopo la sua creazione.
//! * **Job** (`newsletter.send`): ogni messaggio è elaborato da un solo consumer.
//!
//! Implementazioni: NATS JetStream (produzione, più repliche) e in memoria
//! (singolo processo, test). Lo scambio affidabile con il database passa
//! dall'[`outbox`] transazionale.

mod memory;
mod nats;
pub mod outbox;
mod subject;

pub use memory::MemoryBus;
pub use nats::NatsBus;
pub use subject::{subject_matches, validate_filter, validate_subject};

use async_trait::async_trait;
use std::{sync::Arc, time::Duration};
use tokio::{sync::watch, task::JoinHandle};

#[derive(Debug, thiserror::Error)]
pub enum BusError {
    #[error("subject non valido: {0}")]
    InvalidSubject(String),
    #[error("errore del bus: {0}")]
    Backend(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Event,
    Job,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Event => "event",
            Kind::Job => "job",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub kind: Kind,
    pub subject: String,
    /// Identificativo stabile (id dell'outbox): lo stesso a ogni consegna.
    pub id: String,
    pub payload: serde_json::Value,
    /// Numero della consegna, da 1.
    pub delivered: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandlerError {
    /// Errore transitorio: si riprova secondo il backoff.
    Retry(String),
    /// Errore definitivo: va subito in dead letter.
    Permanent(String),
}

#[async_trait]
pub trait Handler: Send + Sync {
    async fn handle(&self, msg: Message) -> Result<(), HandlerError>;
}

#[derive(Debug, Clone)]
pub struct ConsumerSpec {
    /// Nome stabile (durable): identifica la posizione del consumer fra i riavvii.
    pub name: String,
    pub kind: Kind,
    /// Subject o filtro con `*` / `>`.
    pub subject: String,
    /// Consegne totali prima della dead letter.
    pub max_deliver: u32,
    /// Attesa prima del tentativo n+1 (l'ultimo valore vale per i successivi).
    pub backoff: Vec<Duration>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeadLetter {
    pub kind: Kind,
    pub subject: String,
    pub id: String,
    pub payload: serde_json::Value,
    pub consumer: String,
    pub reason: String,
    pub delivered: u32,
}

/// Esito deciso dopo un tentativo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Ack,
    Retry(Duration),
    Dead(String),
}

const DEFAULT_BACKOFF: Duration = Duration::from_secs(5);

/// Politica comune a tutte le implementazioni: dato l'esito del tentativo
/// `delivered`, che cosa fare del messaggio.
pub fn decide(spec: &ConsumerSpec, delivered: u32, outcome: Result<(), HandlerError>) -> Decision {
    match outcome {
        Ok(()) => Decision::Ack,
        Err(HandlerError::Permanent(reason)) => Decision::Dead(reason),
        Err(HandlerError::Retry(reason)) if delivered >= spec.max_deliver => {
            Decision::Dead(format!("tentativi esauriti ({delivered}): {reason}"))
        }
        Err(HandlerError::Retry(_)) => {
            let idx = (delivered as usize).saturating_sub(1).min(spec.backoff.len().saturating_sub(1));
            Decision::Retry(spec.backoff.get(idx).copied().unwrap_or(DEFAULT_BACKOFF))
        }
    }
}

/// Un consumer in esecuzione. `stop()` lo ferma in modo ordinato; se viene
/// lasciato cadere il task si interrompe.
pub struct Consumer {
    pub(crate) stop: watch::Sender<bool>,
    pub(crate) task: JoinHandle<()>,
}

impl Consumer {
    pub async fn stop(mut self) {
        let _ = self.stop.send(true);
        let _ = (&mut self.task).await;
    }
}

impl Drop for Consumer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[async_trait]
pub trait Bus: Send + Sync {
    fn backend(&self) -> &'static str;
    /// `id` rende la pubblicazione idempotente: lo stesso id ripubblicato entro la
    /// finestra di deduplicazione non produce un secondo messaggio.
    async fn publish(&self, kind: Kind, subject: &str, id: &str, payload: &serde_json::Value) -> Result<(), BusError>;
    async fn consume(&self, spec: ConsumerSpec, handler: Arc<dyn Handler>) -> Result<Consumer, BusError>;
    /// Messaggi che hanno esaurito i tentativi o sono falliti in modo definitivo (max 1000).
    async fn dead_letters(&self) -> Result<Vec<DeadLetter>, BusError>;
    async fn health(&self) -> Result<(), BusError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(max: u32, backoff: &[u64]) -> ConsumerSpec {
        ConsumerSpec {
            name: "c".into(),
            kind: Kind::Job,
            subject: "a.b".into(),
            max_deliver: max,
            backoff: backoff.iter().map(|s| Duration::from_secs(*s)).collect(),
        }
    }

    #[test]
    fn decision_policy() {
        let s = spec(3, &[1, 10]);
        let retry = || Err(HandlerError::Retry("x".into()));
        assert_eq!(decide(&s, 1, Ok(())), Decision::Ack);
        assert_eq!(decide(&s, 1, retry()), Decision::Retry(Duration::from_secs(1)));
        assert_eq!(decide(&s, 2, retry()), Decision::Retry(Duration::from_secs(10)));
        assert!(matches!(decide(&s, 3, retry()), Decision::Dead(r) if r.contains("tentativi esauriti")));
        assert_eq!(decide(&s, 1, Err(HandlerError::Permanent("no".into()))), Decision::Dead("no".into()));
        // senza backoff dichiarato: attesa di default
        assert_eq!(decide(&spec(5, &[]), 4, retry()), Decision::Retry(DEFAULT_BACKOFF));
        // l'ultimo valore vale per i tentativi successivi
        assert_eq!(decide(&spec(9, &[2]), 7, retry()), Decision::Retry(Duration::from_secs(2)));
    }
}
