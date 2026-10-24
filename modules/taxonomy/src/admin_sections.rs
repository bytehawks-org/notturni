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
        "taxonomy"
    }

    async fn blog_export(&self, st: &AppState, blog_id: Uuid) -> Result<Vec<(String, Vec<u8>)>, ApiError> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT name, slug FROM categories WHERE blog_id = $1 ORDER BY name")
                .bind(blog_id)
                .fetch_all(&st.db)
                .await?;
        let list: Vec<Value> = rows.into_iter().map(|(name, slug)| json!({"name": name, "slug": slug})).collect();
        Ok(vec![("categories.json".into(), pretty(&Value::Array(list))?)])
    }
}
