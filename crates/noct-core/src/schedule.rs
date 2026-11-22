//! Job periodici: i job con `schedule` in `module.toml` vengono accodati a intervalli regolari.
//!
//! Ogni replica (API o worker) controlla periodicamente; l'`UPDATE` condizionale su
//! `noct_schedules` assegna l'intervallo a una sola, che accoda il job nella stessa transazione
//! (outbox). Se il processo muore tra la decisione e il commit non resta niente: l'intervallo
//! viene riassegnato al giro dopo. Un job periodico deve comunque essere idempotente.

use noct_bus::{Kind, outbox};
use serde_json::json;
use sqlx::PgPool;

/// Un job da accodare ogni `every_secs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    /// `modulo.job`: chiave in `noct_schedules`.
    pub name: String,
    pub subject: String,
    pub every_secs: u64,
}

/// Accoda i job arrivati a scadenza; ritorna i nomi di quelli accodati da questa chiamata.
pub async fn run_due(db: &PgPool, schedules: &[Schedule]) -> Result<Vec<String>, sqlx::Error> {
    let mut fired = Vec::new();
    for s in schedules {
        let mut tx = db.begin().await?;
        let due: Option<String> = sqlx::query_scalar(
            "INSERT INTO noct_schedules (name, last_run_at) VALUES ($1, now())
             ON CONFLICT (name) DO UPDATE SET last_run_at = now()
               WHERE noct_schedules.last_run_at <= now() - make_interval(secs => $2)
             RETURNING name",
        )
        .bind(&s.name)
        .bind(s.every_secs as f64)
        .fetch_optional(&mut *tx)
        .await?;
        if due.is_some() {
            outbox::enqueue(&mut tx, Kind::Job, &s.subject, &json!({"scheduled": s.name}))
                .await
                .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
            fired.push(s.name.clone());
        }
        tx.commit().await?;
    }
    Ok(fired)
}
