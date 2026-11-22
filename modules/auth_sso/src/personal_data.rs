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
        "auth_sso"
    }

    async fn export(&self, st: &AppState, user: Uuid) -> Result<JsonMap, ApiError> {
        let rows: Vec<(String, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
            "SELECT provider::text, email, created_at FROM sso_identities WHERE user_id = $1 ORDER BY created_at",
        )
        .bind(user)
        .fetch_all(&st.db)
        .await?;
        Ok(sections(json!({
            "sso_identities": rows.into_iter().map(|(provider, email, created_at)| json!({"provider": provider, "email": email, "linked_at": created_at})).collect::<Vec<_>>(),
        })))
    }

    async fn erase(&self, conn: &mut PgConnection, _: &AppState, user: Uuid) -> Result<Vec<StoredObject>, ApiError> {
        sqlx::query("DELETE FROM sso_identities WHERE user_id = $1").bind(user).execute(&mut *conn).await?;
        Ok(Vec::new())
    }
}
