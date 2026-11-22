//! Outbox transazionale: l'evento (o il job) viene scritto nella *stessa*
//! transazione del dato di dominio e solo dopo il commit un relay lo pubblica
//! sul bus. Niente eventi persi se il bus è giù, niente eventi per transazioni
//! annullate. La consegna è at-least-once; l'id della riga è la chiave di
//! deduplicazione sul bus.
//!
//! La tabella `noct_outbox` è dichiarata dal modulo core `events` (crate `noct-events`).

use crate::{Bus, BusError, Kind, validate_subject};
use serde_json::Value;
use sqlx::{PgConnection, PgPool, Row, postgres::PgListener};
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;
use uuid::Uuid;

const CHANNEL: &str = "noct_outbox";

#[derive(Debug, thiserror::Error)]
pub enum OutboxError {
    #[error(transparent)]
    Bus(#[from] BusError),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// Accoda un messaggio nella transazione corrente (`conn` è la transazione del chiamante).
pub async fn enqueue(conn: &mut PgConnection, kind: Kind, subject: &str, payload: &Value) -> Result<Uuid, OutboxError> {
    validate_subject(subject)?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO noct_outbox (kind, subject, payload) VALUES ($1::outbox_kind, $2, $3) RETURNING id",
    )
    .bind(kind.as_str())
    .bind(subject)
    .bind(payload)
    .fetch_one(&mut *conn)
    .await?;
    // sveglia i relay; la notifica viene recapitata solo al commit
    sqlx::query("SELECT pg_notify($1, '')").bind(CHANNEL).execute(&mut *conn).await?;
    Ok(id)
}

/// Elimina i messaggi già pubblicati più vecchi di `older_than`.
pub async fn purge(pool: &PgPool, older_than: Duration) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "DELETE FROM noct_outbox WHERE published_at IS NOT NULL AND published_at < now() - make_interval(secs => $1)",
    )
    .bind(older_than.as_secs_f64())
    .execute(pool)
    .await?
    .rows_affected())
}

/// Pubblica sul bus i messaggi dell'outbox, in ordine. Più relay in parallelo
/// sono sicuri (`FOR UPDATE SKIP LOCKED`).
pub struct Relay {
    pool: PgPool,
    bus: Arc<dyn Bus>,
    batch: i64,
    poll: Duration,
}

impl Relay {
    pub fn new(pool: PgPool, bus: Arc<dyn Bus>) -> Self {
        Self { pool, bus, batch: 100, poll: Duration::from_secs(2) }
    }

    pub fn poll_interval(mut self, poll: Duration) -> Self {
        self.poll = poll;
        self
    }

    /// Un giro: pubblica fino a `batch` messaggi. Restituisce quanti ne ha pubblicati;
    /// al primo errore del bus si ferma (per non rompere l'ordine) e lo registra sulla riga.
    pub async fn run_once(&self) -> Result<usize, OutboxError> {
        let mut tx = self.pool.begin().await?;
        let rows = sqlx::query(
            "SELECT id, kind::text AS kind, subject, payload FROM noct_outbox
             WHERE published_at IS NULL ORDER BY created_at, id LIMIT $1 FOR UPDATE SKIP LOCKED",
        )
        .bind(self.batch)
        .fetch_all(&mut *tx)
        .await?;
        let mut done = 0;
        for row in rows {
            let id: Uuid = row.get("id");
            let kind = if row.get::<String, _>("kind") == "job" { Kind::Job } else { Kind::Event };
            let payload: Value = row.get("payload");
            match self.bus.publish(kind, row.get("subject"), &id.to_string(), &payload).await {
                Ok(()) => {
                    sqlx::query("UPDATE noct_outbox SET published_at = now(), last_error = NULL WHERE id = $1")
                        .bind(id)
                        .execute(&mut *tx)
                        .await?;
                    done += 1;
                }
                Err(e) => {
                    tracing::warn!(error = %e, %id, "pubblicazione dall'outbox fallita, si riprova");
                    sqlx::query("UPDATE noct_outbox SET attempts = attempts + 1, last_error = $2 WHERE id = $1")
                        .bind(id)
                        .bind(e.to_string())
                        .execute(&mut *tx)
                        .await?;
                    break;
                }
            }
        }
        tx.commit().await?;
        Ok(done)
    }

    /// Ciclo continuo fino a `shutdown`: reagisce alle notifiche di Postgres e,
    /// in ogni caso, controlla ogni `poll` (le notifiche possono andare perse).
    pub async fn run(self, mut shutdown: watch::Receiver<bool>) {
        let mut listener = match PgListener::connect_with(&self.pool).await {
            Ok(mut l) => l.listen(CHANNEL).await.map(|_| Some(l)).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "LISTEN non attivo, solo polling");
                None
            }),
            Err(e) => {
                tracing::warn!(error = %e, "connessione LISTEN fallita, solo polling");
                None
            }
        };
        loop {
            loop {
                match self.run_once().await {
                    Ok(0) => break,
                    Ok(_) => continue,
                    Err(e) => {
                        tracing::error!(error = %e, "relay dell'outbox");
                        break;
                    }
                }
            }
            tokio::select! {
                _ = shutdown.changed() => return,
                _ = tokio::time::sleep(self.poll) => {}
                _ = async { match listener.as_mut() { Some(l) => { let _ = l.recv().await; } None => std::future::pending().await } } => {}
            }
        }
    }
}
