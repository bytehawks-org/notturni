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
        "newsletter"
    }

    async fn export(&self, st: &AppState, user: Uuid) -> Result<JsonMap, ApiError> {
        let rows: Vec<(Option<Uuid>, String, String, Option<chrono::DateTime<chrono::Utc>>)> = sqlx::query_as(
            "SELECT blog_id, email, status::text, confirmed_at FROM newsletter_subscribers WHERE user_id = $1 ORDER BY created_at",
        )
        .bind(user)
        .fetch_all(&st.db)
        .await?;
        Ok(sections(json!({
            "newsletter_subscriptions": rows.into_iter().map(|(blog_id, email, status, confirmed_at)| {
                json!({"blog_id": blog_id, "email": email, "status": status, "confirmed_at": confirmed_at})
            }).collect::<Vec<_>>(),
        })))
    }

    /// Le iscrizioni collegate all'account: chi vuole restare iscritto può farlo di nuovo, senza account.
    async fn erase(&self, conn: &mut PgConnection, _: &AppState, user: Uuid) -> Result<Vec<StoredObject>, ApiError> {
        sqlx::query("DELETE FROM newsletter_subscribers WHERE user_id = $1").bind(user).execute(&mut *conn).await?;
        Ok(Vec::new())
    }
}
