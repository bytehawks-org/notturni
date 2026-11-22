//! Contributo del modulo all'esportazione completa di un blog.

use async_trait::async_trait;
use noct_core::{AdminSection, ApiError, AppState};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

pub fn all() -> Vec<Arc<dyn AdminSection>> {
    vec![Arc::new(Export)]
}

struct Export;

fn pretty(v: &Value) -> Result<Vec<u8>, ApiError> {
    serde_json::to_vec_pretty(v).map_err(ApiError::internal)
}

#[async_trait]
impl AdminSection for Export {
    fn module(&self) -> &'static str {
        "notes"
    }

    /// La libreria delle note del blog e le note citate da ogni post.
    async fn blog_export(&self, st: &AppState, blog_id: Uuid) -> Result<Vec<(String, Vec<u8>)>, ApiError> {
        type Library = (Uuid, String, String, Option<String>, Option<String>, Option<String>);
        let library: Vec<Library> = sqlx::query_as(
            "SELECT id, content, kind, url, title, author FROM blog_notes WHERE blog_id = $1 ORDER BY created_at, id",
        )
        .bind(blog_id)
        .fetch_all(&st.db)
        .await?;
        let cited: Vec<(Uuid, i32, String)> = sqlx::query_as(
            "SELECT n.post_id, n.idx, n.content FROM post_notes n JOIN posts p ON p.id = n.post_id
             WHERE p.blog_id = $1 ORDER BY n.post_id, n.idx",
        )
        .bind(blog_id)
        .fetch_all(&st.db)
        .await?;
        let doc = json!({
            "library": library.into_iter().map(|(id, content, kind, url, title, author)| json!({
                "id": id, "content": content, "kind": kind, "url": url, "title": title, "author": author,
            })).collect::<Vec<_>>(),
            "post_notes": cited.into_iter().map(|(post, idx, content)| json!({"post_id": post, "index": idx, "content": content})).collect::<Vec<_>>(),
        });
        Ok(vec![("notes.json".into(), pretty(&doc)?)])
    }
}
