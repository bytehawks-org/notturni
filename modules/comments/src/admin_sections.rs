//! Coda dei commenti in attesa nella panoramica e interruttore dei commenti anonimi nelle
//! impostazioni di piattaforma.

use async_trait::async_trait;
use noct_core::{AdminSection, ApiError, AppState, JsonMap};
use serde_json::{Value, json};
use sqlx::PgConnection;
use std::sync::Arc;

pub fn all() -> Vec<Arc<dyn AdminSection>> {
    vec![Arc::new(CommentsAdmin)]
}

struct CommentsAdmin;

#[async_trait]
impl AdminSection for CommentsAdmin {
    fn module(&self) -> &'static str {
        "comments"
    }

    async fn overview(&self, st: &AppState) -> Result<JsonMap, ApiError> {
        let pending: i64 =
            sqlx::query_scalar("SELECT count(*) FROM comments WHERE status = 'pending'").fetch_one(&st.db).await?;
        let mut out = JsonMap::new();
        out.insert("queue_pending_comments".into(), json!(pending));
        Ok(out)
    }

    async fn blog_export(&self, st: &AppState, blog_id: uuid::Uuid) -> Result<Vec<(String, Vec<u8>)>, ApiError> {
        type Row = (uuid::Uuid, uuid::Uuid, Option<uuid::Uuid>, String, String, String, chrono::DateTime<chrono::Utc>);
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT c.id, c.post_id, c.parent_id, c.author_display_name, c.status::text, c.content, c.created_at
             FROM comments c JOIN posts p ON p.id = c.post_id WHERE p.blog_id = $1 ORDER BY c.created_at, c.id",
        )
        .bind(blog_id)
        .fetch_all(&st.db)
        .await?;
        let list: Vec<Value> = rows
            .into_iter()
            .map(|(id, post, parent, author, status, content, at)| {
                json!({"id": id, "post_id": post, "parent_id": parent, "author": author, "status": status,
                       "content": content, "created_at": at.to_rfc3339()})
            })
            .collect();
        let bytes = serde_json::to_vec_pretty(&list).map_err(ApiError::internal)?;
        Ok(vec![("comments.json".into(), bytes)])
    }

    fn settings_keys(&self) -> &'static [&'static str] {
        &["anonymous_comments_allowed"]
    }

    async fn settings(&self, st: &AppState) -> Result<JsonMap, ApiError> {
        let allowed: Option<bool> =
            sqlx::query_scalar("SELECT anonymous_comments_allowed FROM comments_platform_settings WHERE id = 1")
                .fetch_optional(&st.db)
                .await?;
        let mut out = JsonMap::new();
        out.insert("anonymous_comments_allowed".into(), json!(allowed.unwrap_or(true)));
        Ok(out)
    }

    async fn apply_settings(
        &self,
        conn: &mut PgConnection,
        _st: &AppState,
        patch: &JsonMap,
    ) -> Result<JsonMap, ApiError> {
        let Some(Value::Bool(new)) = patch.get("anonymous_comments_allowed") else {
            return Err(ApiError::bad_request("`anonymous_comments_allowed` deve essere booleano."));
        };
        let old: bool = sqlx::query_scalar(
            "SELECT anonymous_comments_allowed FROM comments_platform_settings WHERE id = 1 FOR UPDATE",
        )
        .fetch_optional(&mut *conn)
        .await?
        .unwrap_or(true);
        sqlx::query(
            "INSERT INTO comments_platform_settings (id, anonymous_comments_allowed) VALUES (1, $1)
             ON CONFLICT (id) DO UPDATE SET anonymous_comments_allowed = EXCLUDED.anonymous_comments_allowed",
        )
        .bind(new)
        .execute(&mut *conn)
        .await?;
        let mut changes = JsonMap::new();
        if old != *new {
            changes.insert("anonymous_comments_allowed".into(), json!({"from": old, "to": new}));
        }
        Ok(changes)
    }
}
