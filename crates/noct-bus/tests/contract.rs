//! Contratto comune a MemoryBus e NatsBus (`NOCT_TEST_NATS_URL`, server con JetStream).

use async_trait::async_trait;
use noct_bus::{Bus, ConsumerSpec, Handler, HandlerError, Kind, MemoryBus, Message, NatsBus};
use serde_json::json;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

/// Registra ciò che riceve; fallisce le prime `fail_first` consegne.
#[derive(Default)]
struct Recorder {
    seen: Mutex<Vec<Message>>,
    fail_first: AtomicU32,
    permanent: bool,
}

impl Recorder {
    fn failing(n: u32) -> Arc<Self> {
        Arc::new(Self { fail_first: AtomicU32::new(n), ..Default::default() })
    }
    fn ids(&self) -> Vec<String> {
        self.seen.lock().unwrap().iter().map(|m| m.id.clone()).collect()
    }
    fn count(&self) -> usize {
        self.seen.lock().unwrap().len()
    }
}

#[async_trait]
impl Handler for Recorder {
    async fn handle(&self, msg: Message) -> Result<(), HandlerError> {
        self.seen.lock().unwrap().push(msg);
        if self.permanent {
            return Err(HandlerError::Permanent("rifiutato".into()));
        }
        if self.fail_first.load(Ordering::SeqCst) > 0 {
            self.fail_first.fetch_sub(1, Ordering::SeqCst);
            return Err(HandlerError::Retry("transitorio".into()));
        }
        Ok(())
    }
}

fn spec(name: &str, kind: Kind, subject: &str, max: u32) -> ConsumerSpec {
    ConsumerSpec {
        name: name.into(),
        kind,
        subject: subject.into(),
        max_deliver: max,
        backoff: vec![Duration::from_millis(100)],
    }
}

async fn eventually(what: &str, mut cond: impl FnMut() -> bool) {
    for _ in 0..100 {
        if cond() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("condizione non raggiunta entro 10 s: {what}");
}

async fn contract(bus: &dyn Bus) {
    bus.health().await.unwrap();

    // eventi: fan-out a più consumer, filtri con wildcard, nessuna consegna a chi non corrisponde
    let a = Arc::new(Recorder::default());
    let b = Arc::new(Recorder::default());
    let other = Arc::new(Recorder::default());
    let ca = bus.consume(spec("a", Kind::Event, "post.published", 3), a.clone()).await.unwrap();
    let cb = bus.consume(spec("b", Kind::Event, "post.*", 3), b.clone()).await.unwrap();
    let co = bus.consume(spec("o", Kind::Event, "comment.>", 3), other.clone()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await; // i consumer di eventi partono da "ora"
    bus.publish(Kind::Event, "post.published", "e1", &json!({"n": 1})).await.unwrap();
    bus.publish(Kind::Event, "post.updated", "e2", &json!({"n": 2})).await.unwrap();
    eventually("fan-out", || a.count() == 1 && b.count() == 2).await;
    assert_eq!(a.ids(), ["e1"]);
    assert_eq!(b.seen.lock().unwrap()[0].payload, json!({"n": 1}));
    assert_eq!(b.seen.lock().unwrap()[0].subject, "post.published");
    assert_eq!(other.count(), 0);

    // pubblicazione idempotente: stesso id, un solo messaggio
    bus.publish(Kind::Event, "post.published", "e1", &json!({"n": 1})).await.unwrap();
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(a.count(), 1, "il duplicato non va consegnato");

    // subject non valido rifiutato
    assert!(bus.publish(Kind::Event, "post.*", "x", &json!({})).await.is_err());

    // job: ogni messaggio a un solo consumer, anche se pubblicato prima che parta
    bus.publish(Kind::Job, "mail.send", "j1", &json!({"to": "a"})).await.unwrap();
    let w = Arc::new(Recorder::default());
    let cw = bus.consume(spec("worker", Kind::Job, "mail.send", 3), w.clone()).await.unwrap();
    eventually("job", || w.count() == 1).await;
    assert_eq!(w.ids(), ["j1"]);

    // retry con backoff: fallisce 2 volte, riesce alla terza, con id e payload invariati e `delivered` crescente
    let flaky = Recorder::failing(2);
    let cf = bus.consume(spec("flaky", Kind::Job, "flaky.job", 5), flaky.clone()).await.unwrap();
    bus.publish(Kind::Job, "flaky.job", "j2", &json!({"x": 1})).await.unwrap();
    eventually("retry", || flaky.count() == 3).await;
    let delivered: Vec<u32> = flaky.seen.lock().unwrap().iter().map(|m| m.delivered).collect();
    assert_eq!(delivered, [1, 2, 3]);
    assert!(flaky.seen.lock().unwrap().iter().all(|m| m.id == "j2" && m.payload == json!({"x": 1})));

    // tentativi esauriti → dead letter con il motivo
    let broken = Recorder::failing(99);
    let cx = bus.consume(spec("broken", Kind::Job, "broken.job", 3), broken.clone()).await.unwrap();
    bus.publish(Kind::Job, "broken.job", "j3", &json!({"y": 2})).await.unwrap();
    eventually("esaurimento", || broken.count() == 3).await;
    // errore permanente → dead letter subito, nessun retry
    let perm = Arc::new(Recorder { permanent: true, ..Default::default() });
    let cp = bus.consume(spec("perm", Kind::Job, "perm.job", 5), perm.clone()).await.unwrap();
    bus.publish(Kind::Job, "perm.job", "j4", &json!({})).await.unwrap();
    eventually("dead letters", || futures_block(bus).len() >= 2).await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(broken.count(), 3, "nessuna consegna oltre il limite");
    assert_eq!(perm.count(), 1);

    let dead = bus.dead_letters().await.unwrap();
    let d3 = dead.iter().find(|d| d.id == "j3").expect("j3 in dead letter");
    assert_eq!((d3.consumer.as_str(), d3.delivered, d3.payload.clone()), ("broken", 3, json!({"y": 2})));
    assert!(d3.reason.contains("tentativi esauriti"), "{}", d3.reason);
    let d4 = dead.iter().find(|d| d.id == "j4").expect("j4 in dead letter");
    assert_eq!((d4.reason.as_str(), d4.delivered), ("rifiutato", 1));

    // stop ordinato: dopo lo stop non arrivano più messaggi
    ca.stop().await;
    bus.publish(Kind::Event, "post.published", "e9", &json!({})).await.unwrap();
    eventually("b riceve e9", || b.count() == 3).await;
    assert_eq!(a.count(), 1);
    for c in [cb, co, cw, cf, cx, cp] {
        c.stop().await;
    }
}

/// `eventually` è sincrono: legge le dead letter con un runtime dedicato.
fn futures_block(bus: &dyn Bus) -> Vec<noct_bus::DeadLetter> {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(bus.dead_letters())).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn memory_bus_honours_the_contract() {
    contract(&MemoryBus::default()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn nats_bus_honours_the_contract() {
    let url =
        std::env::var("NOCT_TEST_NATS_URL").expect("NOCT_TEST_NATS_URL non impostata (es. nats://127.0.0.1:4222)");
    let prefix = format!("t{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
    contract(&NatsBus::connect(&url, &prefix).await.unwrap()).await;
}

#[tokio::test]
async fn nats_rejects_bad_prefix_and_unreachable_server() {
    assert!(NatsBus::connect("nats://127.0.0.1:1", "ok").await.is_err());
    assert!(NatsBus::connect("nats://127.0.0.1:1", "Bad-Prefix").await.is_err());
}
