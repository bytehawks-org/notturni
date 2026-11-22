//! Handler dei job dichiarati in `[jobs.*]`: il nome della funzione è quello del job.

use noct_core::{
    AppState,
    bus::{HandlerError, Message},
};
use uuid::Uuid;

/// Elimina tutte le note di un blog. Payload: `{"blog_id": "<uuid>"}`.
pub async fn cleanup(st: AppState, msg: Message) -> Result<(), HandlerError> {
    let blog_id = msg
        .payload
        .get("blog_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| HandlerError::Permanent("payload senza `blog_id` valido".into()))?;
    sqlx::query("DELETE FROM sandbox_notes WHERE blog_id = $1")
        .bind(blog_id)
        .execute(&st.db)
        .await
        .map(|_| ())
        .map_err(|e| HandlerError::Retry(e.to_string()))
}
