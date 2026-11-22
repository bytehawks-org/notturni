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
        "fragments"
    }

    async fn export(&self, st: &AppState, user: Uuid) -> Result<JsonMap, ApiError> {
        let rows: Vec<(Uuid, String, bool, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
            "SELECT post_id, text, is_public, created_at FROM post_fragments WHERE user_id = $1 ORDER BY created_at",
        )
        .bind(user)
        .fetch_all(&st.db)
        .await?;
        Ok(sections(json!({
            "fragments": rows.into_iter().map(|(post_id, text, is_public, captured_at)| {
                json!({"post_id": post_id, "text": text, "is_public": is_public, "captured_at": captured_at})
            }).collect::<Vec<_>>(),
        })))
    }

    async fn erase(&self, conn: &mut PgConnection, _: &AppState, user: Uuid) -> Result<Vec<StoredObject>, ApiError> {
        sqlx::query("DELETE FROM post_fragments WHERE user_id = $1").bind(user).execute(&mut *conn).await?;
        Ok(Vec::new())
    }
}
