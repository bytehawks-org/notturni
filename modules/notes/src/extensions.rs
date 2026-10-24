//! Estensione di `posts`: `notes` in ingresso (assente o `null` = invariate, elenco anche vuoto =
//! sostituisce) e in uscita, ordinate per numero.

use crate::rules::{self, Note};
use async_trait::async_trait;
use noct_core::{ApiError, Entity, EntityExtension, ExtCtx, JsonMap};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool, QueryBuilder};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

pub fn all() -> Vec<Arc<dyn EntityExtension>> {
    vec![Arc::new(PostNotes)]
}

/// Aggancia ogni nota del post alla nota del blog con lo stesso testo normalizzato, creandola se
/// manca. I campi bibliografici si riportano **solo alla creazione**: una nota di libreria già
/// esistente non si sovrascrive mai da qui. Restituisce `contenuto → id`.
async fn link_blog_notes(
    conn: &mut PgConnection,
    blog_id: Uuid,
    created_by: Uuid,
    notes: &[Note],
) -> Result<HashMap<String, Uuid>, ApiError> {
    // serializza i salvataggi dello stesso blog: due post con la stessa nota nuova non la duplicano
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("blog_notes:{blog_id}"))
        .execute(&mut *conn)
        .await?;
    let mut out: HashMap<String, Uuid> = HashMap::new();
    for n in notes {
        let key = rules::normalize_note(&n.content);
        if key.is_empty() || out.contains_key(&n.content) {
            continue;
        }
        let existing: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM blog_notes WHERE blog_id = $1 AND normalized = $2 ORDER BY created_at, id LIMIT 1",
        )
        .bind(blog_id)
        .bind(&key)
        .fetch_optional(&mut *conn)
        .await?;
        let id = match existing {
            Some(id) => id,
            None => {
                // tipo e URL scelti dall'autore battono le euristiche
                let url = n.url.clone().or_else(|| rules::extract_url(&n.content));
                let kind = n.kind.clone().unwrap_or_else(|| rules::guess_kind(&n.content, url.as_deref()).to_string());
                sqlx::query_scalar(
                    "INSERT INTO blog_notes (blog_id, content, normalized, kind, url, created_by_id, title, author, isbn, doi, page, source, issued)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) RETURNING id",
                )
                .bind(blog_id)
                .bind(n.content.trim())
                .bind(&key)
                .bind(kind)
                .bind(url)
                .bind(created_by)
                .bind(&n.title)
                .bind(&n.author)
                .bind(&n.isbn)
                .bind(&n.doi)
                .bind(&n.page)
                .bind(&n.source)
                .bind(&n.issued)
                .fetch_one(&mut *conn)
                .await?
            }
        };
        out.insert(n.content.clone(), id);
    }
    Ok(out)
}

struct PostNotes;

#[async_trait]
impl EntityExtension for PostNotes {
    fn module(&self) -> &'static str {
        "notes"
    }
    fn entity(&self) -> Entity {
        Entity::Post
    }
    fn fields(&self) -> &'static [&'static str] {
        &["notes"]
    }

    async fn apply(&self, conn: &mut PgConnection, ctx: &ExtCtx<'_>, input: &JsonMap) -> Result<(), ApiError> {
        let Some(value) = input.get("notes").filter(|v| !v.is_null()) else { return Ok(()) };
        let notes = rules::parse_notes(value).map_err(ApiError::bad_request)?;
        sqlx::query("DELETE FROM post_notes WHERE post_id = $1").bind(ctx.entity_id).execute(&mut *conn).await?;
        if notes.is_empty() {
            return Ok(());
        }
        let ids = link_blog_notes(conn, ctx.blog.id, ctx.actor.user_id, &notes).await?;
        let mut qb = QueryBuilder::new(
            "INSERT INTO post_notes (post_id, idx, content, note_id, title, author, isbn, doi, page, kind, source, issued, url) ",
        );
        qb.push_values(&notes, |mut row, n| {
            row.push_bind(ctx.entity_id)
                .push_bind(n.idx)
                .push_bind(&n.content)
                .push_bind(ids.get(&n.content).copied())
                .push_bind(&n.title)
                .push_bind(&n.author)
                .push_bind(&n.isbn)
                .push_bind(&n.doi)
                .push_bind(&n.page)
                .push_bind(&n.kind)
                .push_bind(&n.source)
                .push_bind(&n.issued)
                .push_bind(&n.url);
        });
        qb.build().execute(conn).await?;
        Ok(())
    }

    async fn enrich(&self, db: &PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, JsonMap>, ApiError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            post_id: Uuid,
            idx: i32,
            content: String,
            title: Option<String>,
            author: Option<String>,
            isbn: Option<String>,
            doi: Option<String>,
            page: Option<String>,
            kind: Option<String>,
            source: Option<String>,
            issued: Option<String>,
            url: Option<String>,
        }
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT post_id, idx, content, title, author, isbn, doi, page, kind, source, issued, url
             FROM post_notes WHERE post_id = ANY($1) ORDER BY post_id, idx",
        )
        .bind(ids)
        .fetch_all(db)
        .await?;
        let mut by_post: HashMap<Uuid, Vec<Value>> = HashMap::new();
        for r in rows {
            by_post.entry(r.post_id).or_default().push(json!({
                "idx": r.idx, "content": r.content, "title": r.title, "author": r.author, "isbn": r.isbn,
                "doi": r.doi, "page": r.page, "kind": r.kind, "source": r.source, "issued": r.issued, "url": r.url,
            }));
        }
        Ok(ids
            .iter()
            .map(|id| {
                let notes = by_post.remove(id).unwrap_or_default();
                (*id, json!({"notes": notes}).as_object().cloned().unwrap_or_default())
            })
            .collect())
    }
}
