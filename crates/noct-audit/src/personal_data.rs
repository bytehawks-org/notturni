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
        "audit"
    }

    /// Gli ultimi eventi di cui l'utente è l'attore; un account molto vecchio non produce un'esportazione senza limite.
    async fn export(&self, st: &AppState, user: Uuid) -> Result<JsonMap, ApiError> {
        let rows: Vec<(chrono::DateTime<chrono::Utc>, String, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT occurred_at, action, ip, user_agent FROM audit_log WHERE actor_id = $1 ORDER BY occurred_at DESC LIMIT 1000",
        )
        .bind(user)
        .fetch_all(&st.db)
        .await?;
        Ok(sections(json!({
            "audit_log": rows.into_iter().map(|(occurred_at, action, ip, user_agent)| {
                json!({"occurred_at": occurred_at, "action": action, "ip": ip, "user_agent": user_agent})
            }).collect::<Vec<_>>(),
        })))
    }

    /// Il registro di audit si conserva (obbligo di sicurezza e di responsabilità): non si cancella.
    async fn erase(&self, _: &mut PgConnection, _: &AppState, _: Uuid) -> Result<Vec<StoredObject>, ApiError> {
        Ok(Vec::new())
    }
}
