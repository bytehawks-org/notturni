use crate::{Access, Actor, ApiError, AppState, BlogRef};
use async_trait::async_trait;
use noct_bus::{Consumer, ConsumerSpec, Handler, HandlerError, Kind, Message, outbox::Relay};
use serde_json::{Value, json};
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};
use tokio::{sync::watch, task::JoinHandle};

/// Esito di un hook di avvio: un errore interrompe l'avvio del processo.
pub type StartupFuture = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
pub type HandlerFuture = Pin<Box<dyn Future<Output = Result<(), HandlerError>> + Send>>;
/// Firma degli handler di eventi e job scritti dai moduli.
pub type HandlerFn = fn(AppState, Message) -> HandlerFuture;

struct StateHandler {
    state: AppState,
    f: HandlerFn,
}

#[async_trait]
impl Handler for StateHandler {
    async fn handle(&self, msg: Message) -> Result<(), HandlerError> {
        (self.f)(self.state.clone(), msg).await
    }
}

/// Un consumer dichiarato nel `module.toml` di un modulo, con il suo handler.
pub struct ConsumerDef {
    pub module: &'static str,
    pub spec: ConsumerSpec,
    pub handler: Arc<dyn Handler>,
}

/// Costruttore usato dal codice generato.
#[allow(clippy::too_many_arguments)]
pub fn consumer(
    module: &'static str,
    name: &str,
    kind: Kind,
    subject: &str,
    max_deliver: u32,
    backoff_secs: &[u64],
    state: AppState,
    f: HandlerFn,
) -> ConsumerDef {
    ConsumerDef {
        module,
        spec: ConsumerSpec {
            name: format!("{module}_{name}"),
            kind,
            subject: subject.to_string(),
            max_deliver,
            backoff: backoff_secs.iter().map(|s| Duration::from_secs(*s)).collect(),
        },
        handler: Arc::new(StateHandler { state, f }),
    }
}

/// Busta comune dei messaggi pubblicati dalle rotte con `emits`: contesto
/// (modulo, blog, autore) più il dato.
pub fn event_envelope(module: &str, blog: Option<&BlogRef>, actor: Option<&Actor>, data: Value) -> Value {
    json!({
        "module": module,
        "blog_id": blog.map(|b| b.id),
        "actor_id": actor.map(|a| a.user_id),
        "data": data,
    })
}

/// Ogni quanto il pianificatore controlla se un job periodico è a scadenza.
const SCHEDULER_TICK: Duration = Duration::from_secs(15);

fn tx_clone(shutdown: &watch::Sender<bool>) -> watch::Receiver<bool> {
    shutdown.subscribe()
}

/// Relay dell'outbox e consumer dei moduli attivi, in esecuzione.
pub struct Background {
    shutdown: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
    consumers: Vec<Consumer>,
}

impl Background {
    pub(crate) fn start(state: &AppState, consumers: Vec<Consumer>, schedules: Vec<crate::Schedule>) -> Background {
        let (shutdown, rx) = watch::channel(false);
        let relay = Relay::new(state.db.clone(), state.services.bus.clone());
        let mut tasks = vec![tokio::spawn(relay.run(rx.clone()))];
        // pulizia oraria dell'outbox: i messaggi pubblicati servono un giorno per il debug
        let (pool, mut rx2) = (state.db.clone(), rx);
        tasks.push(tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = rx2.changed() => return,
                    _ = tokio::time::sleep(Duration::from_secs(3600)) => {
                        if let Err(e) = noct_bus::outbox::purge(&pool, Duration::from_secs(86_400)).await {
                            tracing::warn!(error = %e, "pulizia dell'outbox fallita");
                        }
                    }
                }
            }
        }));
        if !schedules.is_empty() {
            let (pool, mut rx3) = (state.db.clone(), tx_clone(&shutdown));
            tasks.push(tokio::spawn(async move {
                loop {
                    match crate::schedule::run_due(&pool, &schedules).await {
                        Ok(fired) if !fired.is_empty() => tracing::info!(jobs = ?fired, "job periodici accodati"),
                        Ok(_) => {}
                        Err(e) => tracing::warn!(error = %e, "pianificatore dei job periodici"),
                    }
                    tokio::select! {
                        _ = rx3.changed() => return,
                        _ = tokio::time::sleep(SCHEDULER_TICK) => {}
                    }
                }
            }));
        }
        Background { shutdown, tasks, consumers }
    }

    pub fn consumer_count(&self) -> usize {
        self.consumers.len()
    }

    pub async fn stop(self) {
        let _ = self.shutdown.send(true);
        for c in self.consumers {
            c.stop().await;
        }
        for t in self.tasks {
            let _ = t.await;
        }
    }
}

/// Pubblica un evento tramite l'outbox, nella transazione `conn` della rotta
/// (usato dal codice generato per `emits`). Il messaggio parte solo al commit.
pub async fn emit_event(
    conn: &mut sqlx::PgConnection,
    module: &str,
    subject: &str,
    access: &Access,
    data: Value,
) -> Result<(), ApiError> {
    let envelope = event_envelope(module, access.blog.as_ref(), access.actor.as_ref(), data);
    noct_bus::outbox::enqueue(conn, Kind::Event, subject, &envelope).await.map(|_| ()).map_err(ApiError::internal)
}
