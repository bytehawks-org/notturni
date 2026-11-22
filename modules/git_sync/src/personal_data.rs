//! Dati personali del modulo: le sincronizzazioni impostate dall'utente (senza token) e la loro rimozione.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use noct_core::{ApiError, AppState, JsonMap, PersonalData, StoredObject};
use serde_json::json;
use sqlx::{FromRow, PgConnection};
use std::sync::Arc;
use uuid::Uuid;

pub fn all() -> Vec<Arc<dyn PersonalData>> {
    vec![Arc::new(Provider)]
}

struct Provider;

#[derive(FromRow)]
struct Row {
    blog_slug: String,
    remote_url: String,
    branch: String,
    username: String,
    auth_method: String,
    include_drafts: bool,
    enabled: bool,
    last_status: Option<String>,
    last_run_at: Option<DateTime<Utc>>,
}

#[async_trait]
impl PersonalData for Provider {
    fn module(&self) -> &'static str {
        "git_sync"
    }

    async fn export(&self, st: &AppState, user: Uuid) -> Result<JsonMap, ApiError> {
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT b.slug AS blog_slug, c.remote_url, c.branch, c.username, c.auth_method, c.include_drafts, c.enabled, c.last_status, c.last_run_at
             FROM git_sync_configs c JOIN blogs b ON b.id = c.blog_id WHERE c.owner_id = $1 ORDER BY b.slug",
        )
        .bind(user)
        .fetch_all(&st.db)
        .await?;
        let list: Vec<_> = rows
            .into_iter()
            .map(|r| {
                json!({"blog_slug": r.blog_slug, "remote_url": r.remote_url, "branch": r.branch, "username": r.username, "auth_method": r.auth_method,
                       "include_drafts": r.include_drafts, "enabled": r.enabled, "last_status": r.last_status,
                       "last_run_at": r.last_run_at})
            })
            .collect();
        Ok(json!({"git_sync": list}).as_object().cloned().unwrap_or_default())
    }

    /// Via le configurazioni, e con loro il token salvato. Il repository remoto non è nostro da toccare:
    /// i testi già inviati restano dove l'utente li ha mandati.
    async fn erase(&self, conn: &mut PgConnection, _: &AppState, user: Uuid) -> Result<Vec<StoredObject>, ApiError> {
        sqlx::query("DELETE FROM git_sync_configs WHERE owner_id = $1").bind(user).execute(conn).await?;
        Ok(Vec::new())
    }
}
