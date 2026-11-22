//! Estensioni di esempio: aggiungono un campo a blog (`sandbox_theme`) e a post
//! (`sandbox_mood`), scritti nella stessa transazione del core e restituiti nelle risposte.
//! Mostrano il contratto completo di `EntityExtension`: valori validati (un errore annulla
//! l'intera richiesta), default nelle risposte, ereditarietà alla traduzione di un post.

use async_trait::async_trait;
use noct_core::{ApiError, Entity, EntityExtension, ExtCtx, JsonMap, Op};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

const MOODS: &[&str] = &["calmo", "curioso", "allegro"];
const THEMES: &[&str] = &["chiaro", "scuro"];

pub fn all() -> Vec<Arc<dyn EntityExtension>> {
    vec![Arc::new(PostMood), Arc::new(BlogTheme)]
}

fn choice<'a>(input: &'a JsonMap, key: &str, allowed: &[&str]) -> Result<Option<&'a str>, ApiError> {
    match input.get(key) {
        None => Ok(None),
        Some(Value::String(s)) if allowed.contains(&s.as_str()) => Ok(Some(s)),
        Some(_) => Err(ApiError::bad_request(format!("`{key}` deve essere uno tra: {}.", allowed.join(", ")))),
    }
}

struct PostMood;

#[async_trait]
impl EntityExtension for PostMood {
    fn module(&self) -> &'static str {
        "sandbox"
    }
    fn entity(&self) -> Entity {
        Entity::Post
    }
    fn fields(&self) -> &'static [&'static str] {
        &["sandbox_mood"]
    }

    async fn apply(&self, conn: &mut PgConnection, ctx: &ExtCtx<'_>, input: &JsonMap) -> Result<(), ApiError> {
        let mood = choice(input, "sandbox_mood", MOODS)?;
        match (mood, ctx.op, ctx.source_id) {
            (Some(m), _, _) => {
                sqlx::query(
                    "INSERT INTO sandbox_post_marks (post_id, mood) VALUES ($1, $2)
                     ON CONFLICT (post_id) DO UPDATE SET mood = EXCLUDED.mood",
                )
                .bind(ctx.entity_id)
                .bind(m)
                .execute(conn)
                .await?;
            }
            // una traduzione senza valore proprio eredita quello dell'originale
            (None, Op::Translate, Some(source)) => {
                sqlx::query("INSERT INTO sandbox_post_marks (post_id, mood) SELECT $1, mood FROM sandbox_post_marks WHERE post_id = $2")
                    .bind(ctx.entity_id)
                    .bind(source)
                    .execute(conn)
                    .await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn enrich(&self, db: &PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, JsonMap>, ApiError> {
        let rows: Vec<(Uuid, String)> =
            sqlx::query_as("SELECT post_id, mood FROM sandbox_post_marks WHERE post_id = ANY($1)")
                .bind(ids)
                .fetch_all(db)
                .await?;
        let mut by_id: HashMap<Uuid, String> = rows.into_iter().collect();
        Ok(ids
            .iter()
            .map(|id| {
                let mood = by_id.remove(id).map_or(Value::Null, Value::String);
                (*id, json!({ "sandbox_mood": mood }).as_object().cloned().unwrap_or_default())
            })
            .collect())
    }
}

struct BlogTheme;

#[async_trait]
impl EntityExtension for BlogTheme {
    fn module(&self) -> &'static str {
        "sandbox"
    }
    fn entity(&self) -> Entity {
        Entity::Blog
    }
    fn fields(&self) -> &'static [&'static str] {
        &["sandbox_theme"]
    }

    async fn apply(&self, conn: &mut PgConnection, ctx: &ExtCtx<'_>, input: &JsonMap) -> Result<(), ApiError> {
        if let Some(theme) = choice(input, "sandbox_theme", THEMES)? {
            sqlx::query(
                "INSERT INTO sandbox_blog_marks (blog_id, theme) VALUES ($1, $2)
                 ON CONFLICT (blog_id) DO UPDATE SET theme = EXCLUDED.theme",
            )
            .bind(ctx.entity_id)
            .bind(theme)
            .execute(conn)
            .await?;
        }
        Ok(())
    }

    async fn enrich(&self, db: &PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, JsonMap>, ApiError> {
        let rows: Vec<(Uuid, String)> =
            sqlx::query_as("SELECT blog_id, theme FROM sandbox_blog_marks WHERE blog_id = ANY($1)")
                .bind(ids)
                .fetch_all(db)
                .await?;
        let mut by_id: HashMap<Uuid, String> = rows.into_iter().collect();
        Ok(ids
            .iter()
            .map(|id| {
                // default del modulo per i blog senza riga
                let theme = by_id.remove(id).unwrap_or_else(|| "chiaro".to_string());
                (*id, json!({ "sandbox_theme": theme }).as_object().cloned().unwrap_or_default())
            })
            .collect())
    }
}
