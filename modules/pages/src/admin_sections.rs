//! Contributo del modulo all'esportazione completa di un blog.

use async_trait::async_trait;
use noct_core::{AdminSection, ApiError, AppState};
use std::sync::Arc;
use uuid::Uuid;

pub fn all() -> Vec<Arc<dyn AdminSection>> {
    vec![Arc::new(Export)]
}

struct Export;

fn safe(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '_' }).collect()
}

#[async_trait]
impl AdminSection for Export {
    fn module(&self) -> &'static str {
        "pages"
    }

    /// Una pagina per file Markdown con front matter, come i post.
    async fn blog_export(&self, st: &AppState, blog_id: Uuid) -> Result<Vec<(String, Vec<u8>)>, ApiError> {
        let rows: Vec<(String, String, String, String, bool)> = sqlx::query_as(
            "SELECT title, slug, locale, content, is_published FROM blog_pages WHERE blog_id = $1 ORDER BY created_at, id",
        )
        .bind(blog_id)
        .fetch_all(&st.db)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(title, slug, locale, content, published)| {
                let head = |k: &str, v: &str| {
                    if v.chars().any(|c| matches!(c, ':' | '#' | '"' | '\n')) || v.is_empty() {
                        format!("{k}: {}\n", serde_json::to_string(v).unwrap_or_default())
                    } else {
                        format!("{k}: {v}\n")
                    }
                };
                let body = format!(
                    "---\n{}{}{}is_published: {published}\n---\n\n{}\n",
                    head("title", &title),
                    head("slug", &slug),
                    head("locale", &locale),
                    content.trim_end()
                );
                (format!("pages/{}-{}.md", safe(&locale), safe(&slug)), body.into_bytes())
            })
            .collect())
    }
}
