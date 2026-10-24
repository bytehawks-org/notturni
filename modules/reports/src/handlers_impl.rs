//! Handler `custom` del modulo.

use crate::ReportRequest;
use chrono::{DateTime, Utc};
use noct_core::{ApiError, AppState, RouteCtx, can_view_blog, load_blog, parse_uuid};
use noct_posts::VISIBLE_SQL;
use serde_json::{Value, json};
use uuid::Uuid;

const MAX_NOTE_CHARS: usize = 500;

fn user_id(ctx: &RouteCtx) -> Result<Uuid, ApiError> {
    ctx.actor.as_ref().map(|a| a.user_id).ok_or_else(|| ApiError::unauthorized("Autenticazione richiesta."))
}

/// Registra la segnalazione; idempotente: chi ha già segnalato lo stesso bersaglio riottiene la
/// propria segnalazione, senza duplicati.
async fn create_report(
    st: &AppState,
    reporter: Uuid,
    target_type: &str,
    target: Uuid,
    blog: Uuid,
    body: &ReportRequest,
) -> Result<Value, ApiError> {
    let note: String = body.note.as_deref().unwrap_or_default().trim().chars().take(MAX_NOTE_CHARS).collect();
    let inserted: Option<(Uuid, DateTime<Utc>)> = sqlx::query_as(
        "INSERT INTO content_reports (reporter_id, target_type, target_id, blog_id, reason, note)
         VALUES ($1, $2::report_target_type, $3, $4, $5::report_reason, $6)
         ON CONFLICT ON CONSTRAINT uq_content_report_per_reporter DO NOTHING RETURNING id, created_at",
    )
    .bind(reporter)
    .bind(target_type)
    .bind(target)
    .bind(blog)
    .bind(&body.reason)
    .bind((!note.is_empty()).then_some(note))
    .fetch_optional(&st.db)
    .await?;
    let id = match inserted {
        Some((id, _)) => id,
        None => sqlx::query_scalar(
            "SELECT id FROM content_reports WHERE reporter_id = $1 AND target_type = $2::report_target_type AND target_id = $3",
        )
        .bind(reporter)
        .bind(target_type)
        .bind(target)
        .fetch_one(&st.db)
        .await?,
    };
    let (reason, note, status, created_at): (String, Option<String>, String, DateTime<Utc>) =
        sqlx::query_as("SELECT reason::text, note, status::text, created_at FROM content_reports WHERE id = $1")
            .bind(id)
            .fetch_one(&st.db)
            .await?;
    Ok(json!({
        "id": id, "target_type": target_type, "target_id": target, "reason": reason,
        "note": note, "status": status, "created_at": created_at,
    }))
}

pub async fn report_blog(ctx: RouteCtx, body: ReportRequest) -> Result<Value, ApiError> {
    let me = user_id(&ctx)?;
    // `scope = { blog }` ha già nascosto (404) un blog che il chiamante non può vedere
    let blog = ctx.blog.as_ref().ok_or_else(|| ApiError::internal("scope `blog` assente"))?;
    if blog.owner_id == me {
        return Err(ApiError::bad_request("Non puoi segnalare il tuo stesso blog."));
    }
    create_report(&ctx.state, me, "blog", blog.id, blog.id, &body).await
}

pub async fn report_post(ctx: RouteCtx, body: ReportRequest) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let me = user_id(&ctx)?;
    let post = parse_uuid(ctx.path.get("post_id").map(String::as_str).unwrap_or_default())?;
    let not_found = || ApiError::not_found("Post non trovato.");
    let (blog_slug, author, visible): (String, Uuid, bool) = sqlx::query_as(&format!(
        "SELECT b.slug, p.author_id, ({VISIBLE_SQL}) FROM posts p JOIN blogs b ON b.id = p.blog_id WHERE p.id = $1"
    ))
    .bind(post)
    .fetch_optional(&st.db)
    .await?
    .ok_or_else(not_found)?;
    let blog = load_blog(st, &blog_slug).await?.ok_or_else(not_found)?;
    if !visible || !can_view_blog(st, Some(me), &blog).await? {
        return Err(not_found());
    }
    if author == me || blog.owner_id == me {
        return Err(ApiError::bad_request("Non puoi segnalare un tuo contenuto."));
    }
    create_report(st, me, "post", post, blog.id, &body).await
}
