//! Dati personali del modulo: le revisioni scritte dall'utente (esportazione) e il loro scollegamento
//! dall'account (cancellazione, modulo `gdpr`).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use noct_core::{ApiError, AppState, JsonMap, PersonalData, StoredObject};
use serde_json::json;
use sqlx::PgConnection;
use std::sync::Arc;
use uuid::Uuid;

pub fn all() -> Vec<Arc<dyn PersonalData>> {
    vec![Arc::new(Provider)]
}

struct Provider;

#[async_trait]
impl PersonalData for Provider {
    fn module(&self) -> &'static str {
        "wiki"
    }

    async fn export(&self, st: &AppState, user: Uuid) -> Result<JsonMap, ApiError> {
        let rows: Vec<(String, String, i32, String, String, Option<String>, DateTime<Utc>)> = sqlx::query_as(
            "SELECT b.slug, p.slug, r.revision, r.title, r.content, r.summary, r.created_at
             FROM post_revisions r JOIN posts p ON p.id = r.post_id JOIN blogs b ON b.id = p.blog_id
             WHERE r.editor_id = $1 ORDER BY r.created_at, r.revision",
        )
        .bind(user)
        .fetch_all(&st.db)
        .await?;
        let revisions: Vec<_> = rows
            .into_iter()
            .map(|(blog, post, revision, title, content, summary, created_at)| {
                json!({"blog_slug": blog, "post_slug": post, "revision": revision, "title": title,
                       "content": content, "summary": summary, "created_at": created_at})
            })
            .collect();
        Ok(json!({"wiki_revisions": revisions}).as_object().cloned().unwrap_or_default())
    }

    /// Le revisioni restano (fanno parte della storia di contenuti di terzi, art. 17.3) ma perdono il
    /// collegamento con l'account.
    async fn erase(&self, conn: &mut PgConnection, _: &AppState, user: Uuid) -> Result<Vec<StoredObject>, ApiError> {
        sqlx::query("UPDATE post_revisions SET editor_id = NULL WHERE editor_id = $1").bind(user).execute(conn).await?;
        Ok(Vec::new())
    }
}
