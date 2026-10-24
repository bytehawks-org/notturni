//! Estensione di `posts`: `publication_id` (tri-stato: assente non tocca, `null` rimuove) in
//! ingresso; `publication_id`, `publication` e `chapter_order` in uscita.

use async_trait::async_trait;
use noct_core::{ApiError, Entity, EntityExtension, ExtCtx, JsonMap, Op};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

pub fn all() -> Vec<Arc<dyn EntityExtension>> {
    vec![Arc::new(PostPublication)]
}

struct PostPublication;

#[async_trait]
impl EntityExtension for PostPublication {
    fn module(&self) -> &'static str {
        "publications"
    }
    fn entity(&self) -> Entity {
        Entity::Post
    }
    fn fields(&self) -> &'static [&'static str] {
        &["publication_id"]
    }

    async fn apply(&self, conn: &mut PgConnection, ctx: &ExtCtx<'_>, input: &JsonMap) -> Result<(), ApiError> {
        match input.get("publication_id") {
            // una traduzione senza pubblicazione propria resta nella stessa dell'originale
            None if ctx.op == Op::Translate => {
                if let Some(source) = ctx.source_id {
                    sqlx::query(
                        "INSERT INTO post_publications (post_id, publication_id)
                         SELECT $1, publication_id FROM post_publications WHERE post_id = $2",
                    )
                    .bind(ctx.entity_id)
                    .bind(source)
                    .execute(conn)
                    .await?;
                }
            }
            None => {}
            Some(Value::Null) => {
                sqlx::query("DELETE FROM post_publications WHERE post_id = $1")
                    .bind(ctx.entity_id)
                    .execute(conn)
                    .await?;
            }
            Some(Value::String(raw)) => {
                let invalid = || ApiError::bad_request("Pubblicazione non valida per questo blog.");
                let id: Uuid = raw.parse().map_err(|_| invalid())?;
                let belongs: Option<(Uuid,)> =
                    sqlx::query_as("SELECT id FROM publications WHERE id = $1 AND blog_id = $2")
                        .bind(id)
                        .bind(ctx.blog.id)
                        .fetch_optional(&mut *conn)
                        .await?;
                if belongs.is_none() {
                    return Err(invalid());
                }
                // cambiare pubblicazione azzera la posizione esplicita del capitolo
                sqlx::query(
                    "INSERT INTO post_publications (post_id, publication_id) VALUES ($1, $2)
                     ON CONFLICT (post_id) DO UPDATE SET
                       chapter_order = CASE WHEN post_publications.publication_id = EXCLUDED.publication_id
                                            THEN post_publications.chapter_order END,
                       publication_id = EXCLUDED.publication_id",
                )
                .bind(ctx.entity_id)
                .bind(id)
                .execute(conn)
                .await?;
            }
            Some(_) => return Err(ApiError::bad_request("`publication_id` deve essere un UUID oppure null.")),
        }
        Ok(())
    }

    async fn enrich(&self, db: &PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, JsonMap>, ApiError> {
        let rows: Vec<(Uuid, Uuid, String, String, Option<i32>)> = sqlx::query_as(
            "SELECT pp.post_id, u.id, u.name, u.title, pp.chapter_order FROM post_publications pp
             JOIN publications u ON u.id = pp.publication_id WHERE pp.post_id = ANY($1)",
        )
        .bind(ids)
        .fetch_all(db)
        .await?;
        let mut by_post: HashMap<Uuid, (Uuid, String, String, Option<i32>)> =
            rows.into_iter().map(|(p, id, name, title, order)| (p, (id, name, title, order))).collect();
        Ok(ids
            .iter()
            .map(|post| {
                let map = match by_post.remove(post) {
                    Some((id, name, title, order)) => json!({
                        "publication_id": id,
                        "publication": {"id": id, "name": name, "title": title},
                        "chapter_order": order,
                    }),
                    None => json!({"publication_id": null, "publication": null, "chapter_order": null}),
                };
                (*post, map.as_object().cloned().unwrap_or_default())
            })
            .collect())
    }
}
