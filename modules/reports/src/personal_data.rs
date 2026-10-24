//! Dati personali del modulo: sezioni dell'esportazione e cancellazione (modulo `gdpr`).

use async_trait::async_trait;
use noct_core::{ApiError, AppState, JsonMap, PersonalData, StoredObject};
use serde_json::{Value, json};
use sqlx::PgConnection;
use std::sync::Arc;
use uuid::Uuid;

pub fn all() -> Vec<Arc<dyn PersonalData>> {
    vec![Arc::new(Provider)]
}

struct Provider;

fn sections(v: Value) -> JsonMap {
    v.as_object().cloned().unwrap_or_default()
}

#[async_trait]
impl PersonalData for Provider {
    fn module(&self) -> &'static str {
        "reports"
    }

    async fn export(&self, st: &AppState, user: Uuid) -> Result<JsonMap, ApiError> {
        let rows: Vec<(String, Uuid, String, Option<String>, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
            "SELECT target_type::text, target_id, reason::text, note, status::text, created_at
             FROM content_reports WHERE reporter_id = $1 ORDER BY created_at",
        )
        .bind(user)
        .fetch_all(&st.db)
        .await?;
        Ok(sections(json!({
            "reports_made": rows.into_iter().map(|(target_type, target_id, reason, note, status, created_at)| {
                json!({"target_type": target_type, "target_id": target_id, "reason": reason, "note": note, "status": status, "created_at": created_at})
            }).collect::<Vec<_>>(),
        })))
    }

    /// Le segnalazioni restano ai moderatori (tutela di terzi); il segnalante risulta anonimizzato.
    async fn erase(&self, _: &mut PgConnection, _: &AppState, _: Uuid) -> Result<Vec<StoredObject>, ApiError> {
        Ok(Vec::new())
    }
}
