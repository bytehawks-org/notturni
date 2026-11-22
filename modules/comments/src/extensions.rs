//! Estensioni di `blogs` (modalità e chiusura automatica) e `posts` (override della modalità,
//! modalità effettiva).

use crate::rules;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use noct_core::{ApiError, Entity, EntityExtension, ExtCtx, JsonMap};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

const NO_CAPTCHA: &str = "Commenti aperti a tutti non disponibili: questa istanza non ha un captcha \
configurato (NOCT_TURNSTILE_SECRET_KEY).";

pub fn all() -> Vec<Arc<dyn EntityExtension>> {
    vec![Arc::new(BlogComments), Arc::new(PostComments)]
}

fn parse_mode(value: &Value) -> Result<&str, ApiError> {
    match value.as_str() {
        Some(m) if rules::MODES.contains(&m) => Ok(m),
        _ => Err(ApiError::bad_request(format!("`comments_mode` deve essere uno tra: {}.", rules::MODES.join(", ")))),
    }
}

fn captcha_configured(ctx: &ExtCtx<'_>) -> bool {
    ctx.config_str("comments", "turnstile_secret_key").is_some()
}

struct BlogComments;

#[async_trait]
impl EntityExtension for BlogComments {
    fn module(&self) -> &'static str {
        "comments"
    }
    fn entity(&self) -> Entity {
        Entity::Blog
    }
    fn fields(&self) -> &'static [&'static str] {
        &["comments_mode", "comments_auto_close_days"]
    }

    async fn apply(&self, conn: &mut PgConnection, ctx: &ExtCtx<'_>, input: &JsonMap) -> Result<(), ApiError> {
        // `null` per la modalità = invariata (come in v1)
        if let Some(v) = input.get("comments_mode").filter(|v| !v.is_null()) {
            let mode = parse_mode(v)?;
            if mode == "everyone" {
                let allowed: bool = sqlx::query_scalar(
                    "SELECT coalesce((SELECT anonymous_comments_allowed FROM comments_platform_settings WHERE id = 1), true)",
                )
                .fetch_one(&mut *conn)
                .await?;
                if !allowed {
                    return Err(ApiError::bad_request("I commenti anonimi sono disattivati su questa piattaforma."));
                }
                if !captcha_configured(ctx) {
                    return Err(ApiError::bad_request(NO_CAPTCHA));
                }
            }
            sqlx::query(
                "INSERT INTO comment_blog_settings (blog_id, comments_mode) VALUES ($1, $2::comments_mode)
                 ON CONFLICT (blog_id) DO UPDATE SET comments_mode = EXCLUDED.comments_mode",
            )
            .bind(ctx.entity_id)
            .bind(mode)
            .execute(&mut *conn)
            .await?;
        }
        // tri-stato: `null` o 0 tolgono la chiusura automatica
        if let Some(v) = input.get("comments_auto_close_days") {
            let days = match v {
                Value::Null => None,
                other => {
                    let n = other
                        .as_i64()
                        .and_then(|n| i32::try_from(n).ok())
                        .ok_or_else(|| ApiError::bad_request("comments_auto_close_days deve essere un intero."))?;
                    if n < 0 {
                        return Err(ApiError::bad_request("comments_auto_close_days non può essere negativo."));
                    }
                    (n > 0).then_some(n)
                }
            };
            sqlx::query(
                "INSERT INTO comment_blog_settings (blog_id, comments_auto_close_days) VALUES ($1, $2)
                 ON CONFLICT (blog_id) DO UPDATE SET comments_auto_close_days = EXCLUDED.comments_auto_close_days",
            )
            .bind(ctx.entity_id)
            .bind(days)
            .execute(conn)
            .await?;
        }
        Ok(())
    }

    async fn enrich(&self, db: &PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, JsonMap>, ApiError> {
        let rows: Vec<(Uuid, String, Option<i32>)> = sqlx::query_as(
            "SELECT blog_id, comments_mode::text, comments_auto_close_days FROM comment_blog_settings WHERE blog_id = ANY($1)",
        )
        .bind(ids)
        .fetch_all(db)
        .await?;
        let mut by_id: HashMap<Uuid, (String, Option<i32>)> = rows.into_iter().map(|(id, m, d)| (id, (m, d))).collect();
        Ok(ids
            .iter()
            .map(|id| {
                let (mode, days) = by_id.remove(id).unwrap_or_else(|| (rules::DEFAULT_MODE.into(), None));
                (
                    *id,
                    json!({"comments_mode": mode, "comments_auto_close_days": days})
                        .as_object()
                        .cloned()
                        .unwrap_or_default(),
                )
            })
            .collect())
    }
}

struct PostComments;

#[async_trait]
impl EntityExtension for PostComments {
    fn module(&self) -> &'static str {
        "comments"
    }
    fn entity(&self) -> Entity {
        Entity::Post
    }
    fn fields(&self) -> &'static [&'static str] {
        &["comments_mode"]
    }

    async fn apply(&self, conn: &mut PgConnection, ctx: &ExtCtx<'_>, input: &JsonMap) -> Result<(), ApiError> {
        match input.get("comments_mode") {
            None => {}
            // `null` torna a ereditare dal blog
            Some(Value::Null) => {
                sqlx::query("DELETE FROM comment_post_settings WHERE post_id = $1")
                    .bind(ctx.entity_id)
                    .execute(conn)
                    .await?;
            }
            Some(v) => {
                let mode = parse_mode(v)?;
                if mode == "everyone" && !captcha_configured(ctx) {
                    return Err(ApiError::bad_request(NO_CAPTCHA));
                }
                sqlx::query(
                    "INSERT INTO comment_post_settings (post_id, comments_mode) VALUES ($1, $2::comments_mode)
                     ON CONFLICT (post_id) DO UPDATE SET comments_mode = EXCLUDED.comments_mode",
                )
                .bind(ctx.entity_id)
                .bind(mode)
                .execute(conn)
                .await?;
            }
        }
        Ok(())
    }

    async fn enrich(&self, db: &PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, JsonMap>, ApiError> {
        let rows: Vec<(Uuid, Option<String>, Option<String>, Option<i32>, Option<DateTime<Utc>>)> = sqlx::query_as(
            "SELECT p.id, ps.comments_mode::text, bs.comments_mode::text, bs.comments_auto_close_days, p.published_at
             FROM posts p
             LEFT JOIN comment_post_settings ps ON ps.post_id = p.id
             LEFT JOIN comment_blog_settings bs ON bs.blog_id = p.blog_id
             WHERE p.id = ANY($1)",
        )
        .bind(ids)
        .fetch_all(db)
        .await?;
        let now = Utc::now();
        let mut out: HashMap<Uuid, JsonMap> = rows
            .into_iter()
            .map(|(id, post_mode, blog_mode, days, published)| {
                let effective = rules::effective_mode(post_mode.as_deref(), blog_mode.as_deref(), days, published, now);
                (
                    id,
                    json!({"comments_mode": post_mode, "effective_comments_mode": effective})
                        .as_object()
                        .cloned()
                        .unwrap_or_default(),
                )
            })
            .collect();
        // ogni id deve avere una voce
        for id in ids {
            out.entry(*id).or_insert_with(|| {
                json!({"comments_mode": null, "effective_comments_mode": rules::DEFAULT_MODE})
                    .as_object()
                    .cloned()
                    .unwrap_or_default()
            });
        }
        Ok(out)
    }
}
