use crate::{
    Bus, BusError, Consumer, ConsumerSpec, DeadLetter, Decision, Handler, Kind, Message, decide, subject_matches,
};
use crate::{validate_filter, validate_subject};
use async_trait::async_trait;
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc, watch};

struct Registered {
    spec: ConsumerSpec,
    tx: mpsc::UnboundedSender<Message>,
}

#[derive(Default)]
struct Inner {
    consumers: Vec<Registered>,
    dead: Vec<DeadLetter>,
    /// Job pubblicati quando nessun consumer era ancora registrato (come una coda persistente).
    pending: Vec<Message>,
    seen: HashSet<(Kind, String)>,
}

/// Bus in-process: stessa semantica (retry con backoff, dead letter, dedup per id)
/// ma senza persistenza né più processi. Per test e installazioni `solo`.
#[derive(Clone, Default)]
pub struct MemoryBus {
    inner: Arc<Mutex<Inner>>,
}

#[async_trait]
impl Bus for MemoryBus {
    fn backend(&self) -> &'static str {
        "memory"
    }

    async fn publish(&self, kind: Kind, subject: &str, id: &str, payload: &serde_json::Value) -> Result<(), BusError> {
        validate_subject(subject)?;
        let mut inner = self.inner.lock().expect("lock avvelenato");
        if inner.seen.len() > 10_000 {
            inner.seen.clear();
        }
        if !inner.seen.insert((kind, id.to_string())) {
            return Ok(()); // duplicato
        }
        let msg =
            Message { kind, subject: subject.to_string(), id: id.to_string(), payload: payload.clone(), delivered: 1 };
        let targets =
            inner.consumers.iter().filter(|c| c.spec.kind == kind && subject_matches(&c.spec.subject, subject));
        match kind {
            Kind::Event => targets.for_each(|c| {
                let _ = c.tx.send(msg.clone());
            }),
            // una coda di lavoro: un solo consumer per messaggio
            Kind::Job => match targets.into_iter().next() {
                Some(c) => {
                    let _ = c.tx.send(msg);
                }
                None => inner.pending.push(msg),
            },
        }
        Ok(())
    }

    async fn consume(&self, spec: ConsumerSpec, handler: Arc<dyn Handler>) -> Result<Consumer, BusError> {
        validate_filter(&spec.subject)?;
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        {
            let mut inner = self.inner.lock().expect("lock avvelenato");
            if spec.kind == Kind::Job {
                let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut inner.pending)
                    .into_iter()
                    .partition(|m| subject_matches(&spec.subject, &m.subject));
                inner.pending = rest;
                mine.into_iter().for_each(|m| {
                    let _ = tx.send(m);
                });
            }
            inner.consumers.push(Registered { spec: spec.clone(), tx: tx.clone() });
        }
        let (stop, mut stopped) = watch::channel(false);
        let inner = self.inner.clone();
        let task = tokio::spawn(async move {
            loop {
                let msg = tokio::select! {
                    _ = stopped.changed() => break,
                    m = rx.recv() => match m { Some(m) => m, None => break },
                };
                let delivered = msg.delivered;
                match decide(&spec, delivered, handler.handle(msg.clone()).await) {
                    Decision::Ack => {}
                    Decision::Retry(delay) => {
                        // il ritardo non blocca gli altri messaggi del consumer
                        let tx = tx.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(delay).await;
                            let _ = tx.send(Message { delivered: delivered + 1, ..msg });
                        });
                    }
                    Decision::Dead(reason) => {
                        inner.lock().expect("lock avvelenato").dead.push(DeadLetter {
                            kind: msg.kind,
                            subject: msg.subject,
                            id: msg.id,
                            payload: msg.payload,
                            consumer: spec.name.clone(),
                            reason,
                            delivered,
                        });
                    }
                }
            }
        });
        Ok(Consumer { stop, task })
    }

    async fn dead_letters(&self) -> Result<Vec<DeadLetter>, BusError> {
        Ok(self.inner.lock().expect("lock avvelenato").dead.iter().take(1000).cloned().collect())
    }

    async fn health(&self) -> Result<(), BusError> {
        Ok(())
    }
}
