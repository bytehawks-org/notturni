use crate::{Bus, BusError, Consumer, ConsumerSpec, DeadLetter, Decision, Handler, Kind, Message, decide};
use crate::{validate_filter, validate_subject};
use async_nats::{
    HeaderMap,
    jetstream::{
        self, AckKind,
        consumer::{AckPolicy, DeliverPolicy, pull},
        stream::{Config as StreamConfig, RetentionPolicy, StorageType},
    },
};
use async_trait::async_trait;
use futures::StreamExt;
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;

const MSG_ID: &str = "Nats-Msg-Id";
const H_CONSUMER: &str = "Noct-Consumer";
const H_REASON: &str = "Noct-Reason";
const H_DELIVERED: &str = "Noct-Delivered";
const H_SUBJECT: &str = "Noct-Subject";
const H_KIND: &str = "Noct-Kind";
const H_ID: &str = "Noct-Id";

fn be<E: std::fmt::Display>(e: E) -> BusError {
    BusError::Backend(e.to_string())
}

/// Bus su NATS JetStream. Tre stream (eventi, job, dead letter), tutti con il
/// prefisso indicato: istanze diverse sullo stesso server non si toccano.
pub struct NatsBus {
    client: async_nats::Client,
    js: jetstream::Context,
    prefix: String,
}

impl NatsBus {
    pub async fn connect(url: &str, prefix: &str) -> Result<Self, BusError> {
        if prefix.is_empty() || !prefix.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
            return Err(BusError::Backend(format!("prefisso NATS non valido: `{prefix}` (minuscole, cifre, `_`)")));
        }
        let client = tokio::time::timeout(Duration::from_secs(5), async_nats::connect(url))
            .await
            .map_err(|_| BusError::Backend("timeout di connessione a NATS".into()))?
            .map_err(be)?;
        let js = jetstream::new(client.clone());
        let bus = Self { client, js, prefix: prefix.to_string() };
        bus.ensure_streams().await?;
        Ok(bus)
    }

    fn stream_name(&self, which: &str) -> String {
        format!("{}_{which}", self.prefix.to_uppercase())
    }

    fn subject(&self, which: &str, subject: &str) -> String {
        format!("{}.{which}.{subject}", self.prefix)
    }

    fn which(kind: Kind) -> &'static str {
        match kind {
            Kind::Event => "evt",
            Kind::Job => "job",
        }
    }

    async fn ensure_streams(&self) -> Result<(), BusError> {
        let day = Duration::from_secs(86_400);
        let defs = [
            ("EVT", "evt", RetentionPolicy::Limits, 7 * day),
            // una coda di lavoro: il messaggio sparisce quando è confermato
            ("JOB", "job", RetentionPolicy::WorkQueue, 14 * day),
            ("DLQ", "dlq", RetentionPolicy::Limits, 30 * day),
        ];
        for (name, which, retention, max_age) in defs {
            self.js
                .get_or_create_stream(StreamConfig {
                    name: self.stream_name(name),
                    subjects: vec![format!("{}.{which}.>", self.prefix)],
                    retention,
                    storage: StorageType::File,
                    max_age,
                    ..Default::default()
                })
                .await
                .map_err(be)?;
        }
        Ok(())
    }

    async fn dead_letter(&self, msg: &Message, consumer: &str, reason: &str) {
        let mut headers = HeaderMap::new();
        headers.insert(MSG_ID, format!("dlq-{}-{consumer}", msg.id).as_str());
        headers.insert(H_ID, msg.id.as_str());
        headers.insert(H_CONSUMER, consumer);
        headers.insert(H_REASON, reason.replace(['\r', '\n'], " ").as_str());
        headers.insert(H_DELIVERED, msg.delivered.to_string().as_str());
        headers.insert(H_SUBJECT, msg.subject.as_str());
        headers.insert(H_KIND, msg.kind.as_str());
        let subject = self.subject("dlq", &format!("{}.{}", msg.kind.as_str(), msg.subject));
        let body = serde_json::to_vec(&msg.payload).unwrap_or_default();
        match self.js.publish_with_headers(subject, headers, body.into()).await {
            Ok(ack) => {
                if let Err(e) = ack.await {
                    tracing::error!(error = %e, id = %msg.id, "dead letter non confermata");
                }
            }
            Err(e) => tracing::error!(error = %e, id = %msg.id, "dead letter non pubblicata"),
        }
    }
}

fn durable(spec: &ConsumerSpec) -> String {
    let clean: String =
        spec.name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
    format!("{}_{clean}", spec.kind.as_str())
}

#[async_trait]
impl Bus for NatsBus {
    fn backend(&self) -> &'static str {
        "nats"
    }

    async fn publish(&self, kind: Kind, subject: &str, id: &str, payload: &serde_json::Value) -> Result<(), BusError> {
        validate_subject(subject)?;
        let mut headers = HeaderMap::new();
        headers.insert(MSG_ID, id);
        let body = serde_json::to_vec(payload).map_err(be)?;
        // la conferma del server (`await` sull'ack) è ciò che rende l'outbox sicuro
        self.js
            .publish_with_headers(self.subject(Self::which(kind), subject), headers, body.into())
            .await
            .map_err(be)?
            .await
            .map_err(be)?;
        Ok(())
    }

    async fn consume(&self, spec: ConsumerSpec, handler: Arc<dyn Handler>) -> Result<Consumer, BusError> {
        validate_filter(&spec.subject)?;
        let which = Self::which(spec.kind);
        let stream = self.js.get_stream(self.stream_name(&which.to_uppercase())).await.map_err(be)?;
        let name = durable(&spec);
        let consumer = stream
            .get_or_create_consumer(
                &name,
                pull::Config {
                    durable_name: Some(name.clone()),
                    filter_subject: self.subject(which, &spec.subject),
                    ack_policy: AckPolicy::Explicit,
                    // gli eventi interessano da quando il consumer esiste; i job anche quelli già in coda
                    deliver_policy: if spec.kind == Kind::Event { DeliverPolicy::New } else { DeliverPolicy::All },
                    // una consegna in più del limite: se il processo muore all'ultimo tentativo
                    // il messaggio arriva comunque alla dead letter
                    max_deliver: i64::from(spec.max_deliver) + 1,
                    ack_wait: Duration::from_secs(60),
                    ..Default::default()
                },
            )
            .await
            .map_err(be)?;
        let mut messages = consumer.messages().await.map_err(be)?;

        let (stop, mut stopped) = watch::channel(false);
        let bus = Self { client: self.client.clone(), js: self.js.clone(), prefix: self.prefix.clone() };
        let task = tokio::spawn(async move {
            loop {
                let raw = tokio::select! {
                    _ = stopped.changed() => break,
                    next = messages.next() => match next {
                        Some(Ok(m)) => m,
                        Some(Err(e)) => {
                            tracing::warn!(error = %e, consumer = %spec.name, "errore di lettura dal bus");
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            continue;
                        }
                        None => break,
                    },
                };
                let delivered = raw.info().map(|i| i.delivered as u32).unwrap_or(1);
                let id = raw
                    .headers
                    .as_ref()
                    .and_then(|h| h.get(MSG_ID))
                    .map(|v| v.as_str().to_string())
                    .unwrap_or_else(|| format!("seq-{delivered}"));
                let subject = raw
                    .subject
                    .as_str()
                    .strip_prefix(&format!("{}.{which}.", bus.prefix))
                    .unwrap_or(raw.subject.as_str())
                    .to_string();
                let (payload, parse_err) = match serde_json::from_slice(&raw.payload) {
                    Ok(v) => (v, None),
                    Err(e) => (serde_json::Value::Null, Some(format!("payload non JSON: {e}"))),
                };
                let msg = Message { kind: spec.kind, subject, id, payload, delivered };

                let decision = if let Some(reason) = parse_err {
                    Decision::Dead(reason)
                } else if delivered > spec.max_deliver {
                    Decision::Dead("tentativi esauriti (consegne oltre il limite)".into())
                } else {
                    decide(&spec, delivered, handler.handle(msg.clone()).await)
                };
                let ack = match decision {
                    Decision::Ack => raw.ack().await,
                    Decision::Retry(delay) => raw.ack_with(AckKind::Nak(Some(delay))).await,
                    Decision::Dead(reason) => {
                        tracing::error!(consumer = %spec.name, id = %msg.id, %reason, "messaggio in dead letter");
                        bus.dead_letter(&msg, &spec.name, &reason).await;
                        raw.ack_with(AckKind::Term).await
                    }
                };
                if let Err(e) = ack {
                    tracing::warn!(error = %e, consumer = %spec.name, "conferma non inviata");
                }
            }
        });
        Ok(Consumer { stop, task })
    }

    async fn dead_letters(&self) -> Result<Vec<DeadLetter>, BusError> {
        let stream = self.js.get_stream(self.stream_name("DLQ")).await.map_err(be)?;
        if stream.cached_info().state.messages == 0 {
            return Ok(Vec::new());
        }
        // consumer effimero, sola lettura: non altera lo stream
        let consumer = stream
            .create_consumer(pull::Config {
                ack_policy: AckPolicy::None,
                deliver_policy: DeliverPolicy::All,
                ..Default::default()
            })
            .await
            .map_err(be)?;
        let mut batch =
            consumer.fetch().max_messages(1000).expires(Duration::from_millis(500)).messages().await.map_err(be)?;
        let mut out = Vec::new();
        while let Some(m) = batch.next().await {
            let m = m.map_err(be)?;
            let h = m.headers.clone().unwrap_or_default();
            let get = |k: &str| h.get(k).map(|v| v.as_str().to_string()).unwrap_or_default();
            out.push(DeadLetter {
                kind: if get(H_KIND) == "job" { Kind::Job } else { Kind::Event },
                subject: get(H_SUBJECT),
                id: get(H_ID),
                payload: serde_json::from_slice(&m.payload).unwrap_or(serde_json::Value::Null),
                consumer: get(H_CONSUMER),
                reason: get(H_REASON),
                delivered: get(H_DELIVERED).parse().unwrap_or(0),
            });
            if out.len() >= 1000 {
                break;
            }
        }
        Ok(out)
    }

    async fn health(&self) -> Result<(), BusError> {
        match self.client.connection_state() {
            async_nats::connection::State::Connected => Ok(()),
            other => Err(BusError::Backend(format!("NATS non connesso ({other})"))),
        }
    }
}
