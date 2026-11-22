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

// ---- amministrazione ------------------------------------------------------------------------------------------------------

use crate::BlogAction;
use noct_audit::{Entry, user_actor};
use noct_core::revalidate;
use sqlx::PgConnection;

const BLOG_SELECT: &str = "SELECT b.id, b.slug, b.title, u.username AS owner_username, b.visibility::text AS visibility,
        b.is_suspended, b.is_paused, b.deleted_at, b.created_at, (SELECT count(*) FROM posts p WHERE p.blog_id = b.id) AS posts_count,
        (SELECT count(*) FROM content_reports r WHERE r.blog_id = b.id AND r.status = 'open') AS reports_open
    FROM blogs b JOIN users u ON u.id = b.owner_id";

#[derive(sqlx::FromRow)]
struct BlogRow {
    id: Uuid,
    slug: String,
    title: String,
    owner_username: String,
    visibility: String,
    is_suspended: bool,
    is_paused: bool,
    deleted_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    posts_count: i64,
    reports_open: i64,
}

fn blog_json(b: &BlogRow) -> Value {
    json!({
        "id": b.id, "slug": b.slug, "title": b.title, "owner_username": b.owner_username, "visibility": b.visibility,
        "is_suspended": b.is_suspended, "is_paused": b.is_paused, "deleted_at": b.deleted_at, "created_at": b.created_at,
        "posts_count": b.posts_count, "reports_open": b.reports_open,
    })
}

async fn load_blog_row(conn: &mut PgConnection, id: Uuid) -> Result<BlogRow, ApiError> {
    sqlx::query_as(&format!("{BLOG_SELECT} WHERE b.id = $1"))
        .bind(id)
        .fetch_optional(conn)
        .await?
        .ok_or_else(|| ApiError::not_found("Blog non trovato."))
}

pub async fn blog_reports(ctx: RouteCtx) -> Result<Value, ApiError> {
    let id = parse_uuid(ctx.path.get("blog_id").map(String::as_str).unwrap_or_default())?;
    let mut conn = ctx.state.db.acquire().await?;
    let blog = load_blog_row(&mut conn, id).await?;
    let (mfa, email): (bool, String) =
        sqlx::query_as("SELECT u.mfa_enabled, u.email FROM blogs b JOIN users u ON u.id = b.owner_id WHERE b.id = $1")
            .bind(id)
            .fetch_one(&mut *conn)
            .await?;
    #[derive(sqlx::FromRow)]
    struct Detail {
        id: Uuid,
        target_type: String,
        target_id: Uuid,
        reason: String,
        note: Option<String>,
        reporter_username: String,
        post_slug: Option<String>,
        post_title: Option<String>,
        created_at: DateTime<Utc>,
    }
    let rows: Vec<Detail> = sqlx::query_as(
        "SELECT r.id, r.target_type::text AS target_type, r.target_id, r.reason::text AS reason, r.note,
                ru.username AS reporter_username, p.slug AS post_slug, p.title AS post_title, r.created_at
         FROM content_reports r JOIN users ru ON ru.id = r.reporter_id
         LEFT JOIN posts p ON r.target_type = 'post' AND p.id = r.target_id
         WHERE r.blog_id = $1 AND r.status = 'open' ORDER BY r.created_at DESC, r.id",
    )
    .bind(id)
    .fetch_all(&mut *conn)
    .await?;
    Ok(json!({
        "blog": blog_json(&blog),
        "owner_mfa_enabled": mfa,
        "owner_email_domain": email.rsplit('@').next().unwrap_or_default(),
        "reports": rows.into_iter().map(|r| json!({
            "id": r.id, "target_type": r.target_type, "target_id": r.target_id, "post_slug": r.post_slug,
            "post_title": r.post_title, "reason": r.reason, "note": r.note, "reporter_username": r.reporter_username,
            "created_at": r.created_at,
        })).collect::<Vec<_>>(),
    }))
}

/// Sospende o riattiva, nasconde i post segnalati, disattiva il proprietario o archivia: in ogni caso
/// le segnalazioni aperte del blog si chiudono (`actioned`/`dismissed`) e l'azione va nel registro.
pub async fn blog_action(ctx: RouteCtx, body: BlogAction) -> Result<Value, ApiError> {
    let admin = user_id(&ctx)?;
    let id = parse_uuid(ctx.path.get("blog_id").map(String::as_str).unwrap_or_default())?;
    let note = body.note.trim();
    if note.chars().count() < 3 {
        return Err(ApiError::bad_request("Serve una nota per il registro (almeno 3 caratteri)."));
    }
    let note: String = note.chars().take(1000).collect();
    let mut tx = ctx.state.db.begin().await?;
    let blog = load_blog_row(&mut tx, id).await?;
    let slug = blog.slug.clone();
    let mut revalidate_tags = vec![revalidate::blog(&slug), revalidate::feed()];
    let mut payload = json!({"slug": slug, "note": note, "action": body.action});

    let audit_action = match body.action.as_str() {
        "suspend" => {
            sqlx::query("UPDATE blogs SET is_suspended = TRUE, updated_at = now() WHERE id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            "blog.suspended"
        }
        "restore" => {
            sqlx::query("UPDATE blogs SET is_suspended = FALSE, updated_at = now() WHERE id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            "blog.unsuspended"
        }
        "hide_reported_posts" => {
            let hidden: Vec<String> = sqlx::query_scalar(
                "UPDATE posts SET is_hidden = TRUE, updated_at = now()
                 WHERE id IN (SELECT target_id FROM content_reports
                              WHERE blog_id = $1 AND target_type = 'post' AND status = 'open')
                 RETURNING slug",
            )
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
            for post_slug in &hidden {
                revalidate_tags.push(revalidate::post(&slug, post_slug));
            }
            payload["hidden_posts"] = json!(hidden);
            "blog.reported_posts_hidden"
        }
        "deactivate_owner" => {
            let (owner, role, username): (Uuid, String, String) = sqlx::query_as(
                "SELECT u.id, u.platform_role::text, u.username FROM blogs b JOIN users u ON u.id = b.owner_id WHERE b.id = $1",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            let admin_role = ctx.actor.as_ref().map(|a| a.platform_role.as_str()).unwrap_or_default();
            if matches!(role.as_str(), "amministratore" | "super_admin") && admin_role != "super_admin" {
                return Err(ApiError::forbidden("Solo un Super Admin può disattivare un amministratore."));
            }
            if owner == admin {
                return Err(ApiError::bad_request("Non puoi disattivare il tuo stesso account."));
            }
            sqlx::query(
                "UPDATE users SET is_active = FALSE, credentials_changed_at = now(), updated_at = now() WHERE id = $1",
            )
            .bind(owner)
            .execute(&mut *tx)
            .await?;
            sqlx::query("UPDATE user_sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL")
                .bind(owner)
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE blogs SET is_suspended = TRUE, updated_at = now() WHERE id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            payload["owner"] = json!(username);
            "user.deactivated"
        }
        "dismiss" => "blog.reports_dismissed",
        _ => {
            return Err(ApiError::bad_request(
                "`action` deve essere suspend, restore, hide_reported_posts, deactivate_owner o dismiss.",
            ));
        }
    };
    sqlx::query(
        "UPDATE content_reports SET status = $2::report_status, resolved_at = now(), resolved_by_id = $3
         WHERE blog_id = $1 AND status = 'open'",
    )
    .bind(id)
    .bind(if body.action == "dismiss" { "dismissed" } else { "actioned" })
    .bind(admin)
    .execute(&mut *tx)
    .await?;
    let actor = user_actor(&mut tx, admin).await.map_err(ApiError::internal)?;
    noct_audit::record(
        &mut tx,
        Entry { action: audit_action, actor, target: Some(("blog", id)), blog_id: Some(id), meta: &ctx.meta, payload },
    )
    .await
    .map_err(ApiError::internal)?;
    let fresh = load_blog_row(&mut tx, id).await?;
    tx.commit().await?;
    if body.action != "dismiss" {
        ctx.state.services.revalidate.spawn(revalidate_tags);
    }
    Ok(blog_json(&fresh))
}
