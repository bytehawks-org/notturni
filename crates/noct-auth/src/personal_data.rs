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
        "auth"
    }

    async fn export(&self, st: &AppState, user: Uuid) -> Result<JsonMap, ApiError> {
        // mai il valore del token né il suo hash: solo i metadati
        let rows: Vec<(String, String, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>, Option<chrono::DateTime<chrono::Utc>>)> =
            sqlx::query_as(
                "SELECT name, token_prefix, created_at, last_used_at, revoked_at FROM api_tokens WHERE user_id = $1 ORDER BY created_at",
            )
            .bind(user)
            .fetch_all(&st.db)
            .await?;
        let tokens: Vec<Value> = rows
            .into_iter()
            .map(|(name, prefix, created, used, revoked)| {
                json!({"name": name, "token_prefix": prefix, "created_at": created, "last_used_at": used, "revoked_at": revoked})
            })
            .collect();
        Ok(sections(json!({"api_tokens": tokens})))
    }

    async fn erase(&self, conn: &mut PgConnection, _: &AppState, user: Uuid) -> Result<Vec<StoredObject>, ApiError> {
        for table in ["user_sessions", "api_tokens", "password_reset_codes"] {
            sqlx::query(&format!("DELETE FROM {table} WHERE user_id = $1")).bind(user).execute(&mut *conn).await?;
        }
        Ok(Vec::new())
    }
}
