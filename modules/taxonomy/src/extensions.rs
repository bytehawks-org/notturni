//! Estensione di `posts`: `category_id` (tri-stato: assente non tocca, `null` rimuove) in
//! ingresso, `category_id` e `category` in uscita.

use async_trait::async_trait;
use noct_core::{ApiError, Entity, EntityExtension, ExtCtx, JsonMap, Op};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

pub fn all() -> Vec<Arc<dyn EntityExtension>> {
    vec![Arc::new(PostCategory)]
}

struct PostCategory;

#[async_trait]
impl EntityExtension for PostCategory {
    fn module(&self) -> &'static str {
        "taxonomy"
    }
    fn entity(&self) -> Entity {
        Entity::Post
    }
    fn fields(&self) -> &'static [&'static str] {
        &["category_id"]
    }

    async fn apply(&self, conn: &mut PgConnection, ctx: &ExtCtx<'_>, input: &JsonMap) -> Result<(), ApiError> {
        match input.get("category_id") {
            // una traduzione senza categoria propria eredita quella dell'originale
            None if ctx.op == Op::Translate => {
                if let Some(source) = ctx.source_id {
                    sqlx::query(
                        "INSERT INTO post_categories (post_id, category_id)
                         SELECT $1, category_id FROM post_categories WHERE post_id = $2",
                    )
                    .bind(ctx.entity_id)
                    .bind(source)
                    .execute(conn)
                    .await?;
                }
            }
            None => {}
            Some(Value::Null) => {
                sqlx::query("DELETE FROM post_categories WHERE post_id = $1").bind(ctx.entity_id).execute(conn).await?;
            }
            Some(Value::String(raw)) => {
                let invalid = || ApiError::bad_request("Categoria non valida per questo blog.");
                let id: Uuid = raw.parse().map_err(|_| invalid())?;
                let belongs: Option<(Uuid,)> =
                    sqlx::query_as("SELECT id FROM categories WHERE id = $1 AND blog_id = $2")
                        .bind(id)
                        .bind(ctx.blog.id)
                        .fetch_optional(&mut *conn)
                        .await?;
                if belongs.is_none() {
                    return Err(invalid());
                }
                sqlx::query(
                    "INSERT INTO post_categories (post_id, category_id) VALUES ($1, $2)
                     ON CONFLICT (post_id) DO UPDATE SET category_id = EXCLUDED.category_id",
                )
                .bind(ctx.entity_id)
                .bind(id)
                .execute(conn)
                .await?;
            }
            Some(_) => return Err(ApiError::bad_request("`category_id` deve essere un UUID oppure null.")),
        }
        Ok(())
    }

    async fn enrich(&self, db: &PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, JsonMap>, ApiError> {
        let rows: Vec<(Uuid, Uuid, String, String)> = sqlx::query_as(
            "SELECT pc.post_id, c.id, c.name, c.slug FROM post_categories pc
             JOIN categories c ON c.id = pc.category_id WHERE pc.post_id = ANY($1)",
        )
        .bind(ids)
        .fetch_all(db)
        .await?;
        let mut by_post: HashMap<Uuid, (Uuid, String, String)> =
            rows.into_iter().map(|(p, id, name, slug)| (p, (id, name, slug))).collect();
        Ok(ids
            .iter()
            .map(|post| {
                let (category_id, category) = match by_post.remove(post) {
                    Some((id, name, slug)) => (json!(id), json!({"id": id, "name": name, "slug": slug})),
                    None => (Value::Null, Value::Null),
                };
                let map =
                    json!({"category_id": category_id, "category": category}).as_object().cloned().unwrap_or_default();
                (*post, map)
            })
            .collect())
    }
}
