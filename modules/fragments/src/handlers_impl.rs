//! Handler `custom` del modulo.

use crate::{FragmentCreate, FragmentUpdate, validate_text};
use chrono::{DateTime, Utc};
use noct_core::{ApiError, RouteCtx, can_view_blog, load_blog, parse_uuid};
use noct_posts::VISIBLE_SQL;
use serde_json::{Value, json};
use uuid::Uuid;

fn user_id(ctx: &RouteCtx) -> Result<Uuid, ApiError> {
    ctx.actor.as_ref().map(|a| a.user_id).ok_or_else(|| ApiError::unauthorized("Autenticazione richiesta."))
}

#[derive(sqlx::FromRow)]
struct FragmentRow {
    id: Uuid,
    post_id: Uuid,
    text: String,
    is_public: bool,
    created_at: DateTime<Utc>,
}

const COLS: &str = "id, post_id, text, is_public, created_at";

fn out(f: &FragmentRow) -> Value {
    json!({"id": f.id, "post_id": f.post_id, "text": f.text, "is_public": f.is_public, "created_at": f.created_at})
}

pub async fn create_fragment(ctx: RouteCtx, body: FragmentCreate) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let user = user_id(&ctx)?;
    let post_id = parse_uuid(ctx.path.get("post_id").map(String::as_str).unwrap_or_default())?;
    let not_found = || ApiError::not_found("Post non trovato.");

    // un frammento si salva solo su un post pubblicato e visibile (stessa regola del permalink)
    let (content, blog_slug, visible): (String, String, bool) = sqlx::query_as(&format!(
        "SELECT p.content, b.slug, ({VISIBLE_SQL}) FROM posts p JOIN blogs b ON b.id = p.blog_id WHERE p.id = $1"
    ))
    .bind(post_id)
    .fetch_optional(&st.db)
    .await?
    .ok_or_else(not_found)?;
    let blog = load_blog(st, &blog_slug).await?.ok_or_else(not_found)?;
    if !visible || !can_view_blog(st, Some(user), &blog).await? {
        return Err(not_found());
    }
    let text = validate_text(&body.text, &content).map_err(ApiError::bad_request)?;

    // insert idempotente: lo stesso frammento già salvato (anche in parallelo) non è un errore
    let hash = text_hash(&text);
    let inserted: Option<FragmentRow> = sqlx::query_as(&format!(
        "INSERT INTO post_fragments (user_id, post_id, text, text_hash, is_public) VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (user_id, post_id, text_hash) DO NOTHING RETURNING {COLS}"
    ))
    .bind(user)
    .bind(post_id)
    .bind(&text)
    .bind(&hash)
    .bind(body.is_public.unwrap_or(false))
    .fetch_optional(&st.db)
    .await?;
    let row = match inserted {
        Some(r) => r,
        None => {
            sqlx::query_as(&format!(
                "SELECT {COLS} FROM post_fragments WHERE user_id = $1 AND post_id = $2 AND text_hash = $3"
            ))
            .bind(user)
            .bind(post_id)
            .bind(&hash)
            .fetch_one(&st.db)
            .await?
        }
    };
    Ok(out(&row))
}

/// Chiave di deduplica del testo: primi 16 byte (32 hex) dello SHA-256. Un frammento può essere
/// più lungo del limite di un indice btree, l'hash no.
fn text_hash(text: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(&Sha256::digest(text.as_bytes())[..16])
}

pub async fn list_post_fragments(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let user = user_id(&ctx)?;
    let post_id = parse_uuid(ctx.path.get("post_id").map(String::as_str).unwrap_or_default())?;
    let rows: Vec<FragmentRow> = sqlx::query_as(&format!(
        "SELECT {COLS} FROM post_fragments WHERE user_id = $1 AND post_id = $2 ORDER BY created_at, id"
    ))
    .bind(user)
    .bind(post_id)
    .fetch_all(&ctx.state.db)
    .await?;
    Ok(rows.iter().map(out).collect())
}

/// (id, testo, pubblico, data, titolo post, autore, slug blog, slug post)
type CollectionRow = (Uuid, String, bool, DateTime<Utc>, String, String, String, String);

pub async fn list_my_fragments(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let user = user_id(&ctx)?;
    let rows: Vec<CollectionRow> = sqlx::query_as(
        "SELECT f.id, f.text, f.is_public, f.created_at, p.title, p.author_display_name, b.slug, p.slug
         FROM post_fragments f JOIN posts p ON p.id = f.post_id JOIN blogs b ON b.id = p.blog_id
         WHERE f.user_id = $1 ORDER BY f.created_at DESC, f.id",
    )
    .bind(user)
    .fetch_all(&ctx.state.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, text, is_public, created_at, post_title, author, blog_slug, post_slug)| {
            json!({
                "id": id, "text": text, "is_public": is_public, "created_at": created_at,
                "post_title": post_title,
                // già risolto al salvataggio del post: vista secondaria, non ricalcolata come PostOut
                "author_display_name": author,
                "permalink": noct_posts::rules::permalink(&blog_slug, &post_slug),
            })
        })
        .collect())
}

async fn own_fragment(ctx: &RouteCtx) -> Result<FragmentRow, ApiError> {
    let id = parse_uuid(ctx.path.get("fragment_id").map(String::as_str).unwrap_or_default())?;
    sqlx::query_as(&format!("SELECT {COLS} FROM post_fragments WHERE id = $1 AND user_id = $2"))
        .bind(id)
        .bind(user_id(ctx)?)
        .fetch_optional(&ctx.state.db)
        .await?
        .ok_or_else(|| ApiError::not_found("Frammento non trovato."))
}

pub async fn update_fragment(ctx: RouteCtx, body: FragmentUpdate) -> Result<Value, ApiError> {
    let f = own_fragment(&ctx).await?;
    let row: FragmentRow = sqlx::query_as(&format!(
        "UPDATE post_fragments SET is_public = $1, updated_at = now() WHERE id = $2 RETURNING {COLS}"
    ))
    .bind(body.is_public)
    .bind(f.id)
    .fetch_one(&ctx.state.db)
    .await?;
    Ok(out(&row))
}

pub async fn delete_fragment(ctx: RouteCtx) -> Result<(), ApiError> {
    let f = own_fragment(&ctx).await?;
    sqlx::query("DELETE FROM post_fragments WHERE id = $1").bind(f.id).execute(&ctx.state.db).await?;
    Ok(())
}
