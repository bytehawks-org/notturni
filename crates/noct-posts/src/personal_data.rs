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
        "posts"
    }

    async fn export(&self, st: &AppState, user: Uuid) -> Result<JsonMap, ApiError> {
        #[allow(clippy::type_complexity)]
        let rows: Vec<(
            Uuid,
            Uuid,
            String,
            String,
            String,
            String,
            String,
            chrono::DateTime<chrono::Utc>,
            Option<chrono::DateTime<chrono::Utc>>,
        )> = sqlx::query_as(
            "SELECT id, blog_id, title, slug, locale, status::text, content, created_at, published_at
                 FROM posts WHERE author_id = $1 ORDER BY created_at",
        )
        .bind(user)
        .fetch_all(&st.db)
        .await?;
        Ok(sections(json!({
            "posts_authored": rows.into_iter().map(|(id, blog_id, title, slug, locale, status, content, created_at, published_at)| {
                json!({"id": id, "blog_id": blog_id, "title": title, "slug": slug, "locale": locale, "status": status,
                       "content": content, "created_at": created_at, "published_at": published_at})
            }).collect::<Vec<_>>(),
        })))
    }

    /// I post restano: sono contenuto condiviso con i lettori e i collaboratori.
    async fn erase(&self, _: &mut PgConnection, _: &AppState, _: Uuid) -> Result<Vec<StoredObject>, ApiError> {
        Ok(Vec::new())
    }
}
