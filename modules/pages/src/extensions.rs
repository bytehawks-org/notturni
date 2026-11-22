//! Estensione di `blogs`: `static_pages_enabled` (default `false`) in ingresso e in uscita.

use async_trait::async_trait;
use noct_core::{ApiError, Entity, EntityExtension, ExtCtx, JsonMap};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

pub fn all() -> Vec<Arc<dyn EntityExtension>> {
    vec![Arc::new(BlogPages)]
}

struct BlogPages;

#[async_trait]
impl EntityExtension for BlogPages {
    fn module(&self) -> &'static str {
        "pages"
    }
    fn entity(&self) -> Entity {
        Entity::Blog
    }
    fn fields(&self) -> &'static [&'static str] {
        &["static_pages_enabled"]
    }

    async fn apply(&self, conn: &mut PgConnection, ctx: &ExtCtx<'_>, input: &JsonMap) -> Result<(), ApiError> {
        match input.get("static_pages_enabled") {
            None => {}
            Some(Value::Bool(enabled)) => {
                sqlx::query(
                    "INSERT INTO blog_pages_settings (blog_id, static_pages_enabled) VALUES ($1, $2)
                     ON CONFLICT (blog_id) DO UPDATE SET static_pages_enabled = EXCLUDED.static_pages_enabled",
                )
                .bind(ctx.entity_id)
                .bind(enabled)
                .execute(conn)
                .await?;
            }
            Some(_) => return Err(ApiError::bad_request("`static_pages_enabled` deve essere un booleano.")),
        }
        Ok(())
    }

    async fn enrich(&self, db: &PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, JsonMap>, ApiError> {
        let rows: Vec<(Uuid, bool)> =
            sqlx::query_as("SELECT blog_id, static_pages_enabled FROM blog_pages_settings WHERE blog_id = ANY($1)")
                .bind(ids)
                .fetch_all(db)
                .await?;
        let mut by_id: HashMap<Uuid, bool> = rows.into_iter().collect();
        Ok(ids
            .iter()
            .map(|id| {
                let enabled = by_id.remove(id).unwrap_or(false);
                (*id, json!({"static_pages_enabled": enabled}).as_object().cloned().unwrap_or_default())
            })
            .collect())
    }
}
