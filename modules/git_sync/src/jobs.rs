//! Consumer dei job del modulo (`[jobs.*]` di module.toml).

use crate::{SUBJECT_RUN, sync};
use noct_core::{
    AppState,
    bus::{HandlerError, Kind, Message, outbox},
};
use serde_json::json;
use uuid::Uuid;

fn retry(e: impl ToString) -> HandlerError {
    HandlerError::Retry(e.to_string())
}

/// Accoda una sincronizzazione per ogni configurazione attiva. Quelle senza modifiche si fermano subito
/// (impronta invariata) senza toccare la rete.
pub async fn dispatch(st: AppState, _msg: Message) -> Result<(), HandlerError> {
    let ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM git_sync_configs WHERE enabled").fetch_all(&st.db).await.map_err(retry)?;
    let mut tx = st.db.begin().await.map_err(retry)?;
    for id in &ids {
        outbox::enqueue(&mut tx, Kind::Job, SUBJECT_RUN, &json!({"config_id": id, "force": false}))
            .await
            .map_err(retry)?;
    }
    tx.commit().await.map_err(retry)?;
    Ok(())
}

pub async fn run(st: AppState, msg: Message) -> Result<(), HandlerError> {
    let id = msg
        .payload
        .get("config_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| HandlerError::Permanent("payload senza `config_id` valido".into()))?;
    let force = msg.payload.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
    sync::run(&st, id, force).await.map_err(retry)
}
