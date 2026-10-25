//! Estensione di `posts` per gli Spazi wiki e docs: posizione nell'albero (`parent_id`, `position`),
//! istantanea di titolo e contenuto a ogni modifica (`revision_summary` in ingresso, `revisions`
//! in uscita) e link interni. Per gli Spazi `blog` non fa nulla e rifiuta i campi dell'albero.

use crate::rules::{self, MAX_REVISIONS};
use async_trait::async_trait;
use noct_core::{ApiError, Entity, EntityExtension, ExtCtx, JsonMap};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool, QueryBuilder};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

pub fn all() -> Vec<Arc<dyn EntityExtension>> {
    vec![Arc::new(PostWiki)]
}

const SUMMARY_MAX: usize = 300;

/// `post → padre` di tutti i post dello Spazio.
pub async fn parent_map(conn: &mut PgConnection, blog: Uuid) -> Result<HashMap<Uuid, Option<Uuid>>, ApiError> {
    let rows: Vec<(Uuid, Option<Uuid>)> = sqlx::query_as(
        "SELECT p.id, o.parent_id FROM posts p LEFT JOIN post_outline o ON o.post_id = p.id WHERE p.blog_id = $1",
    )
    .bind(blog)
    .fetch_all(conn)
    .await?;
    Ok(rows.into_iter().collect())
}

fn opt_uuid(v: &Value) -> Result<Option<Uuid>, ApiError> {
    match v {
        Value::Null => Ok(None),
        Value::String(s) => {
            Uuid::parse_str(s).map(Some).map_err(|_| ApiError::bad_request("`parent_id` non è un UUID valido."))
        }
        _ => Err(ApiError::bad_request("`parent_id` deve essere un UUID o null.")),
    }
}

struct PostWiki;

impl PostWiki {
    async fn place(conn: &mut PgConnection, ctx: &ExtCtx<'_>, input: &JsonMap) -> Result<(), ApiError> {
        let id = ctx.entity_id;
        let current: Option<(Option<Uuid>, i32)> =
            sqlx::query_as("SELECT parent_id, position FROM post_outline WHERE post_id = $1")
                .bind(id)
                .fetch_optional(&mut *conn)
                .await?;
        let parent = match input.get("parent_id") {
            Some(v) => opt_uuid(v)?,
            None => current.and_then(|c| c.0),
        };
        if input.contains_key("parent_id") {
            let mut parents = parent_map(&mut *conn, ctx.blog.id).await?;
            parents.insert(id, parent);
            rules::check_parents(&parents, &[id]).map_err(ApiError::bad_request)?;
        }
        let moved = current.is_none_or(|c| c.0 != parent);
        let position = match input.get("position") {
            Some(Value::Number(n)) if n.as_i64().is_some_and(|p| (0..=i64::from(i32::MAX)).contains(&p)) => {
                n.as_i64().unwrap_or_default() as i32
            }
            Some(_) => return Err(ApiError::bad_request("`position` deve essere un intero non negativo.")),
            None if !moved => return Ok(()),
            // nuova pagina o spostata senza posizione: in coda ai fratelli
            None => sqlx::query_scalar(
                "SELECT COALESCE(max(position) + 1, 0) FROM post_outline WHERE parent_id IS NOT DISTINCT FROM $1 AND post_id <> $2",
            )
            .bind(parent)
            .bind(id)
            .fetch_one(&mut *conn)
            .await?,
        };
        sqlx::query(
            "INSERT INTO post_outline (post_id, parent_id, position) VALUES ($1, $2, $3)
             ON CONFLICT (post_id) DO UPDATE SET parent_id = EXCLUDED.parent_id, position = EXCLUDED.position, updated_at = now()",
        )
        .bind(id)
        .bind(parent)
        .bind(position)
        .execute(conn)
        .await?;
        Ok(())
    }

    /// Nuova revisione solo se titolo o contenuto differiscono dall'ultima.
    async fn snapshot(conn: &mut PgConnection, ctx: &ExtCtx<'_>, input: &JsonMap) -> Result<String, ApiError> {
        let (title, content): (String, String) = sqlx::query_as("SELECT title, content FROM posts WHERE id = $1")
            .bind(ctx.entity_id)
            .fetch_one(&mut *conn)
            .await?;
        let last: Option<(i32, String, String)> = sqlx::query_as(
            "SELECT revision, title, content FROM post_revisions WHERE post_id = $1 ORDER BY revision DESC LIMIT 1",
        )
        .bind(ctx.entity_id)
        .fetch_optional(&mut *conn)
        .await?;
        if last.as_ref().is_none_or(|(_, t, c)| *t != title || *c != content) {
            let next = last.map_or(1, |l| l.0 + 1);
            let summary = match input.get("revision_summary") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) if s.chars().count() <= SUMMARY_MAX => Some(s.trim().to_string()),
                Some(_) => {
                    return Err(ApiError::bad_request(format!(
                        "`revision_summary` deve essere un testo di al massimo {SUMMARY_MAX} caratteri."
                    )));
                }
            }
            .filter(|s| !s.is_empty());
            sqlx::query(
                "INSERT INTO post_revisions (post_id, revision, title, content, editor_id, summary) VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(ctx.entity_id)
            .bind(next)
            .bind(&title)
            .bind(&content)
            .bind(ctx.actor.user_id)
            .bind(summary)
            .execute(&mut *conn)
            .await?;
            sqlx::query("DELETE FROM post_revisions WHERE post_id = $1 AND revision <= $2")
                .bind(ctx.entity_id)
                .bind(next - MAX_REVISIONS)
                .execute(&mut *conn)
                .await?;
        }
        Ok(content)
    }

    async fn sync_links(conn: &mut PgConnection, post: Uuid, content: &str) -> Result<(), ApiError> {
        sqlx::query("DELETE FROM post_wikilinks WHERE post_id = $1").bind(post).execute(&mut *conn).await?;
        let links = rules::extract_wikilinks(content);
        if links.is_empty() {
            return Ok(());
        }
        let mut qb = QueryBuilder::new("INSERT INTO post_wikilinks (post_id, position, target_slug, label) ");
        qb.push_values(links.iter().enumerate(), |mut row, (i, l)| {
            row.push_bind(post).push_bind(i as i32).push_bind(&l.target).push_bind(&l.label);
        });
        qb.build().execute(conn).await?;
        Ok(())
    }
}

#[async_trait]
impl EntityExtension for PostWiki {
    fn module(&self) -> &'static str {
        "wiki"
    }
    fn entity(&self) -> Entity {
        Entity::Post
    }
    fn fields(&self) -> &'static [&'static str] {
        &["parent_id", "position", "revision_summary", "revisions"]
    }

    async fn apply(&self, conn: &mut PgConnection, ctx: &ExtCtx<'_>, input: &JsonMap) -> Result<(), ApiError> {
        let kind: String = sqlx::query_scalar("SELECT kind::text FROM blogs WHERE id = $1")
            .bind(ctx.blog.id)
            .fetch_one(&mut *conn)
            .await?;
        if kind == "blog" {
            if input.get("parent_id").is_some_and(|v| !v.is_null()) || input.contains_key("position") {
                return Err(ApiError::bad_request("La gerarchia delle pagine esiste solo negli spazi wiki e docs."));
            }
            return Ok(());
        }
        Self::place(&mut *conn, ctx, input).await?;
        let content = Self::snapshot(&mut *conn, ctx, input).await?;
        Self::sync_links(conn, ctx.entity_id, &content).await
    }

    async fn enrich(&self, db: &PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, JsonMap>, ApiError> {
        let outline: Vec<(Uuid, Option<Uuid>, i32)> =
            sqlx::query_as("SELECT post_id, parent_id, position FROM post_outline WHERE post_id = ANY($1)")
                .bind(ids)
                .fetch_all(db)
                .await?;
        let revisions: Vec<(Uuid, i64)> =
            sqlx::query_as("SELECT post_id, count(*) FROM post_revisions WHERE post_id = ANY($1) GROUP BY post_id")
                .bind(ids)
                .fetch_all(db)
                .await?;
        let outline: HashMap<Uuid, (Option<Uuid>, i32)> = outline.into_iter().map(|(i, p, o)| (i, (p, o))).collect();
        let revisions: HashMap<Uuid, i64> = revisions.into_iter().collect();
        Ok(ids
            .iter()
            .map(|id| {
                let (parent, position) = outline.get(id).copied().unwrap_or((None, 0));
                let fields = json!({
                    "parent_id": parent, "position": position, "revisions": revisions.get(id).copied().unwrap_or(0),
                });
                (*id, fields.as_object().cloned().unwrap_or_default())
            })
            .collect())
    }
}
