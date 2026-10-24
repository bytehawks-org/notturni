//! Handler `custom` del modulo: creazione e lettura dei commenti, moderazione, segnalazione e
//! lista dei bloccati.

use crate::rules;
use crate::{BlockRequest, CommentCreate, ReportRequest};
use chrono::{DateTime, Utc};
use noct_audit::{Entry, user_actor};
use noct_core::{Actor, ApiError, AppState, BlogRef, RouteCtx, can_view_blog, load_blog, membership_role, parse_uuid};
use noct_posts::{VISIBLE_SQL, rules::UserNames};
use serde_json::{Value, json};
use sqlx::PgConnection;
use std::{collections::HashMap, time::Duration};
use uuid::Uuid;

const FORBIDDEN_MODERATE: &str = "Serve essere proprietario del blog o mediatore.";
const DEFAULT_VERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";
const VERIFY_TIMEOUT: Duration = Duration::from_secs(10);

fn path_uuid(ctx: &RouteCtx, name: &str) -> Result<Uuid, ApiError> {
    parse_uuid(ctx.path.get(name).map(String::as_str).unwrap_or_default())
}

fn actor(ctx: &RouteCtx) -> Result<&Actor, ApiError> {
    ctx.actor.as_ref().ok_or_else(|| ApiError::unauthorized("Autenticazione richiesta."))
}

fn blog_of(ctx: &RouteCtx) -> Result<&BlogRef, ApiError> {
    ctx.blog.as_ref().ok_or_else(|| ApiError::internal("scope `blog` assente"))
}

/// Proprietario o mediatore del blog, oppure ruolo di piattaforma moderatore/amministratore.
async fn can_moderate(st: &AppState, actor: &Actor, blog: &BlogRef) -> Result<bool, ApiError> {
    if matches!(actor.platform_role.as_str(), "super_admin" | "amministratore" | "moderatore")
        || blog.owner_id == actor.user_id
    {
        return Ok(true);
    }
    Ok(membership_role(st, actor.user_id, blog.id).await?.as_deref() == Some("mediatore"))
}

async fn require_moderator(st: &AppState, actor: &Actor, blog: &BlogRef) -> Result<(), ApiError> {
    if can_moderate(st, actor, blog).await? { Ok(()) } else { Err(ApiError::forbidden(FORBIDDEN_MODERATE)) }
}

// ---- post -----------------------------------------------------------------------------------------------------------

struct PostInfo {
    id: Uuid,
    slug: String,
    visible: bool,
    published_at: Option<DateTime<Utc>>,
    blog: BlogRef,
}

async fn load_post(st: &AppState, id: Uuid) -> Result<PostInfo, ApiError> {
    let not_found = || ApiError::not_found("Post non trovato.");
    let (slug, blog_slug, published_at, visible): (String, String, Option<DateTime<Utc>>, bool) = sqlx::query_as(&format!(
        "SELECT p.slug, b.slug, p.published_at, ({VISIBLE_SQL}) FROM posts p JOIN blogs b ON b.id = p.blog_id WHERE p.id = $1"
    ))
    .bind(id)
    .fetch_optional(&st.db)
    .await?
    .ok_or_else(not_found)?;
    let blog = load_blog(st, &blog_slug).await?.ok_or_else(not_found)?;
    Ok(PostInfo { id, slug, visible, published_at, blog })
}

/// Un post si commenta e si legge solo se è pubblicato e il blog è visibile al chiamante.
async fn load_commentable(ctx: &RouteCtx, id: Uuid) -> Result<PostInfo, ApiError> {
    let post = load_post(&ctx.state, id).await?;
    let viewer = ctx.actor.as_ref().map(|a| a.user_id);
    if !post.visible || !can_view_blog(&ctx.state, viewer, &post.blog).await? {
        return Err(ApiError::not_found("Post non trovato."));
    }
    Ok(post)
}

async fn effective_mode(st: &AppState, post: &PostInfo) -> Result<String, ApiError> {
    let (override_mode, blog_mode, days): (Option<String>, Option<String>, Option<i32>) = sqlx::query_as(
        "SELECT (SELECT comments_mode::text FROM comment_post_settings WHERE post_id = $1),
                (SELECT comments_mode::text FROM comment_blog_settings WHERE blog_id = $2),
                (SELECT comments_auto_close_days FROM comment_blog_settings WHERE blog_id = $2)",
    )
    .bind(post.id)
    .bind(post.blog.id)
    .fetch_one(&st.db)
    .await?;
    Ok(rules::effective_mode(override_mode.as_deref(), blog_mode.as_deref(), days, post.published_at, Utc::now()))
}

// ---- commenti ---------------------------------------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct CommentRow {
    id: Uuid,
    post_id: Uuid,
    parent_id: Option<Uuid>,
    author_id: Option<Uuid>,
    author_display_name: String,
    author_email: Option<String>,
    content: String,
    status: String,
    created_at: DateTime<Utc>,
    reported_to_platform: bool,
    report_note: Option<String>,
    post_title: Option<String>,
    post_slug: Option<String>,
}

const COMMENT_COLS: &str =
    "c.id, c.post_id, c.parent_id, c.author_id, c.author_display_name, c.author_email, c.content,
    c.status::text AS status, c.created_at, c.reported_to_platform, c.report_note,
    NULL::text AS post_title, NULL::text AS post_slug";

/// Nome e avatar degli autori registrati, ricalcolati a ogni lettura (un cambio di username o di
/// preferenza si riflette sui commenti passati); un solo SELECT per l'intero elenco.
async fn comments_out(st: &AppState, rows: &[CommentRow], moderator_view: bool) -> Result<Vec<Value>, ApiError> {
    let ids: Vec<Uuid> = rows.iter().filter_map(|c| c.author_id).collect();
    type AuthorRow =
        (Uuid, String, Option<String>, Option<String>, Option<String>, String, Option<String>, Option<String>);
    let authors: Vec<AuthorRow> = sqlx::query_as(
        "SELECT id, username, display_name, first_name, last_name, post_author_name_style::text, verified_domain, avatar_object_key
         FROM users WHERE id = ANY($1)",
    )
    .bind(&ids)
    .fetch_all(&st.db)
    .await?;
    let by_id: HashMap<Uuid, (String, Option<String>)> = authors
        .into_iter()
        .map(|(id, username, display_name, first_name, last_name, style, verified_domain, avatar)| {
            let name = noct_posts::rules::personal_display_name(&UserNames {
                username,
                display_name,
                first_name,
                last_name,
                style,
                verified_domain,
            });
            let avatar_url = avatar.map(|k| st.services.storage.public_url(&st.config.storage.bucket_avatars, &k));
            (id, (name, avatar_url))
        })
        .collect();
    Ok(rows
        .iter()
        .map(|c| {
            let (name, avatar) = match c.author_id.and_then(|id| by_id.get(&id)) {
                Some((n, a)) => (n.clone(), a.clone()),
                None => (c.author_display_name.clone(), None),
            };
            let mut v = json!({
                "id": c.id, "post_id": c.post_id, "parent_id": c.parent_id, "author_id": c.author_id,
                "author_display_name": name, "author_avatar_url": avatar, "status": c.status,
                "content": c.content, "created_at": c.created_at,
                // la segnalazione ai moderatori di piattaforma è riservata a chi modera
                "reported_to_platform": moderator_view && c.reported_to_platform,
                "report_note": if moderator_view { c.report_note.clone() } else { None },
            });
            if let (Some(t), Some(s)) = (&c.post_title, &c.post_slug) {
                v["post_title"] = json!(t);
                v["post_slug"] = json!(s);
            }
            v
        })
        .collect())
}

async fn comment_by_id(st: &AppState, id: Uuid) -> Result<CommentRow, ApiError> {
    sqlx::query_as(&format!("SELECT {COMMENT_COLS} FROM comments c WHERE c.id = $1"))
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or_else(|| ApiError::not_found("Commento non trovato."))
}

async fn is_blocked(st: &AppState, blog: Uuid, user: Option<Uuid>, email: Option<&str>) -> Result<bool, ApiError> {
    let hash = email.filter(|e| !e.trim().is_empty()).map(rules::email_hash);
    if user.is_none() && hash.is_none() {
        return Ok(false);
    }
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM blog_blocked_authors WHERE blog_id = $1 AND (user_id = $2 OR email_hash = $3))",
    )
    .bind(blog)
    .bind(user)
    .bind(hash)
    .fetch_one(&st.db)
    .await?)
}

/// Verifica del token Turnstile. Fail closed: a differenza della moderazione immagini, qui lo
/// scopo è proprio fermare i bot, quindi un servizio irraggiungibile respinge il commento.
async fn verify_captcha(ctx: &RouteCtx, token: &str) -> bool {
    let Some(secret) = ctx.config_str("turnstile_secret_key") else { return false };
    let url = ctx.config_str("turnstile_verify_url").unwrap_or(DEFAULT_VERIFY_URL);
    let mut form = vec![("secret", secret.to_string()), ("response", token.to_string())];
    if let Some(ip) = &ctx.meta.ip {
        form.push(("remoteip", ip.clone()));
    }
    let Ok(client) = reqwest::Client::builder().timeout(VERIFY_TIMEOUT).build() else { return false };
    let result = async {
        let resp = client.post(url).form(&form).send().await?.error_for_status()?;
        resp.json::<Value>().await
    }
    .await;
    match result {
        Ok(v) => v.get("success").and_then(Value::as_bool).unwrap_or(false),
        Err(e) => {
            tracing::warn!("verifica Turnstile fallita (servizio irraggiungibile o errore): {e}");
            false
        }
    }
}

pub async fn create_comment(ctx: RouteCtx, body: CommentCreate) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let post = load_commentable(&ctx, path_uuid(&ctx, "post_id")?).await?;
    let mode = effective_mode(st, &post).await?;
    if mode == "closed" {
        return Err(ApiError::forbidden("I commenti sono chiusi per questo post."));
    }

    let parent_id = match body.parent_id.as_deref() {
        Some(raw) => {
            let id = parse_uuid(raw)?;
            let same_post: bool =
                sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM comments WHERE id = $1 AND post_id = $2)")
                    .bind(id)
                    .bind(post.id)
                    .fetch_one(&st.db)
                    .await?;
            if !same_post {
                return Err(ApiError::bad_request("Commento a cui rispondere non valido."));
            }
            Some(id)
        }
        None => None,
    };

    let user = ctx.actor.as_ref().map(|a| a.user_id);
    if is_blocked(st, post.blog.id, user, body.author_email.as_deref()).await? {
        return Err(ApiError::forbidden("Non puoi commentare su questo blog."));
    }

    let row: CommentRow = if let Some(user) = user {
        // utenti registrati: nessuna moderazione obbligatoria
        let username: String =
            sqlx::query_scalar("SELECT username FROM users WHERE id = $1").bind(user).fetch_one(&st.db).await?;
        sqlx::query_as(&format!(
            "WITH c AS (INSERT INTO comments (post_id, parent_id, author_id, author_display_name, content, status)
                        VALUES ($1, $2, $3, $4, $5, 'approved') RETURNING *)
             SELECT {} FROM c",
            COMMENT_COLS
        ))
        .bind(post.id)
        .bind(parent_id)
        .bind(user)
        .bind(username)
        .bind(&body.content)
        .fetch_one(&st.db)
        .await?
    } else {
        if mode == "members" {
            return Err(ApiError::unauthorized("Questo blog accetta commenti solo da utenti registrati."));
        }
        let name = body.author_display_name.as_deref().map(str::trim).filter(|s| !s.is_empty());
        let email = body.author_email.as_deref().map(str::trim).filter(|s| !s.is_empty());
        let (Some(name), Some(email)) = (name, email) else {
            return Err(ApiError::bad_request("Nome e email sono richiesti per commentare senza account."));
        };
        if !rules::valid_email(email) {
            return Err(ApiError::bad_request("Email non valida."));
        }
        let verified = match body.captcha_token.as_deref().filter(|t| !t.is_empty()) {
            Some(token) => verify_captcha(&ctx, token).await,
            None => false,
        };
        if !verified {
            return Err(ApiError::bad_request("Verifica captcha non superata."));
        }
        // i commenti non registrati sono sempre moderati prima della pubblicazione
        sqlx::query_as(&format!(
            "WITH c AS (INSERT INTO comments (post_id, parent_id, author_display_name, author_email, content, status)
                        VALUES ($1, $2, $3, $4, $5, 'pending') RETURNING *)
             SELECT {} FROM c",
            COMMENT_COLS
        ))
        .bind(post.id)
        .bind(parent_id)
        .bind(name)
        .bind(email)
        .bind(&body.content)
        .fetch_one(&st.db)
        .await?
    };
    Ok(comments_out(st, &[row], false).await?.remove(0))
}

pub async fn list_approved_comments(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let post = load_commentable(&ctx, path_uuid(&ctx, "post_id")?).await?;
    let rows: Vec<CommentRow> = sqlx::query_as(&format!(
        "SELECT {COMMENT_COLS} FROM comments c WHERE c.post_id = $1 AND c.status = 'approved' ORDER BY c.created_at, c.id"
    ))
    .bind(post.id)
    .fetch_all(&ctx.state.db)
    .await?;
    comments_out(&ctx.state, &rows, false).await
}

pub async fn list_pending_comments(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let post = load_post(&ctx.state, path_uuid(&ctx, "post_id")?).await?;
    require_moderator(&ctx.state, actor(&ctx)?, &post.blog).await?;
    let rows: Vec<CommentRow> = sqlx::query_as(&format!(
        "SELECT {COMMENT_COLS} FROM comments c WHERE c.post_id = $1 AND c.status = 'pending' ORDER BY c.created_at, c.id"
    ))
    .bind(post.id)
    .fetch_all(&ctx.state.db)
    .await?;
    comments_out(&ctx.state, &rows, true).await
}

pub async fn list_blog_comments(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let blog = blog_of(&ctx)?;
    require_moderator(&ctx.state, actor(&ctx)?, blog).await?;
    let status = ctx.query.get("status").map(String::as_str).unwrap_or("pending");
    if !["pending", "approved", "rejected"].contains(&status) {
        return Err(ApiError::unprocessable("`status` deve essere pending, approved o rejected."));
    }
    let reported = matches!(ctx.query.get("reported").map(String::as_str), Some("true" | "1"));
    let filter = if reported { "c.reported_to_platform" } else { "c.status::text = $2::text" };
    let sql = format!(
        "SELECT c.id, c.post_id, c.parent_id, c.author_id, c.author_display_name, c.author_email, c.content,
                c.status::text AS status, c.created_at, c.reported_to_platform, c.report_note,
                p.title AS post_title, p.slug AS post_slug
         FROM comments c JOIN posts p ON p.id = c.post_id
         WHERE p.blog_id = $1 AND {filter} AND ($2::text IS NOT NULL) ORDER BY c.created_at DESC, c.id"
    );
    let rows: Vec<CommentRow> = sqlx::query_as(&sql).bind(blog.id).bind(status).fetch_all(&ctx.state.db).await?;
    comments_out(&ctx.state, &rows, true).await
}

// ---- moderazione -------------------------------------------------------------------------------------------------------

/// Commento, post e blog, con il permesso di moderare già verificato.
async fn moderated(ctx: &RouteCtx) -> Result<(CommentRow, PostInfo), ApiError> {
    let comment = comment_by_id(&ctx.state, path_uuid(ctx, "comment_id")?).await?;
    let post = load_post(&ctx.state, comment.post_id).await?;
    require_moderator(&ctx.state, actor(ctx)?, &post.blog).await?;
    Ok((comment, post))
}

async fn audit(
    conn: &mut PgConnection,
    ctx: &RouteCtx,
    action: &str,
    comment: Uuid,
    blog: Uuid,
    payload: Value,
) -> Result<(), ApiError> {
    let who = user_actor(&mut *conn, actor(ctx)?.user_id).await.map_err(ApiError::internal)?;
    noct_audit::record(
        conn,
        Entry { action, actor: who, target: Some(("comment", comment)), blog_id: Some(blog), meta: &ctx.meta, payload },
    )
    .await
    .map_err(ApiError::internal)
}

async fn moderate(ctx: RouteCtx, new_status: &str) -> Result<Value, ApiError> {
    let (comment, post) = moderated(&ctx).await?;
    let alias: Option<String> = sqlx::query_scalar("SELECT default_author_display_name FROM blogs WHERE id = $1")
        .bind(post.blog.id)
        .fetch_one(&ctx.state.db)
        .await?;
    let mut tx = ctx.state.db.begin().await?;
    sqlx::query("UPDATE comments SET status = $1::comment_status, updated_at = now() WHERE id = $2")
        .bind(new_status)
        .bind(comment.id)
        .execute(&mut *tx)
        .await?;
    // l'alias è quello pubblico del blog, non dell'attore: sotto quale identità compare il commento
    audit(
        &mut tx,
        &ctx,
        &format!("comment.{new_status}"),
        comment.id,
        post.blog.id,
        json!({"post_id": post.id, "post_slug": post.slug, "blog_slug": post.blog.slug, "blog_alias": alias}),
    )
    .await?;
    tx.commit().await?;
    let row = comment_by_id(&ctx.state, comment.id).await?;
    Ok(comments_out(&ctx.state, &[row], true).await?.remove(0))
}

pub async fn approve_comment(ctx: RouteCtx) -> Result<Value, ApiError> {
    moderate(ctx, "approved").await
}

pub async fn reject_comment(ctx: RouteCtx) -> Result<Value, ApiError> {
    moderate(ctx, "rejected").await
}

pub async fn report_comment(ctx: RouteCtx, body: ReportRequest) -> Result<Value, ApiError> {
    let note = body.note.trim();
    if note.is_empty() {
        return Err(ApiError::bad_request("La nota della segnalazione è obbligatoria."));
    }
    let note: String = note.chars().take(1000).collect();
    let (comment, post) = moderated(&ctx).await?;
    let mut tx = ctx.state.db.begin().await?;
    sqlx::query(
        "UPDATE comments SET reported_to_platform = true, report_note = $1, reported_at = now(), updated_at = now() WHERE id = $2",
    )
    .bind(&note)
    .bind(comment.id)
    .execute(&mut *tx)
    .await?;
    audit(
        &mut tx,
        &ctx,
        "comment.reported",
        comment.id,
        post.blog.id,
        json!({"post_slug": post.slug, "blog_slug": post.blog.slug, "note": note}),
    )
    .await?;
    tx.commit().await?;
    let row = comment_by_id(&ctx.state, comment.id).await?;
    Ok(comments_out(&ctx.state, &[row], true).await?.remove(0))
}

fn blocked_out(id: Uuid, label: &str, is_anonymous: bool, note: &Option<String>, created_at: DateTime<Utc>) -> Value {
    json!({"id": id, "label": label, "is_anonymous": is_anonymous, "note": note, "created_at": created_at})
}

pub async fn block_comment_author(ctx: RouteCtx, body: BlockRequest) -> Result<Value, ApiError> {
    let (comment, post) = moderated(&ctx).await?;
    if comment.author_id == Some(post.blog.owner_id) {
        return Err(ApiError::bad_request("Non puoi bloccare il proprietario del blog."));
    }
    let email = comment.author_email.as_deref().filter(|e| !e.is_empty());
    if comment.author_id.is_none() && email.is_none() {
        return Err(ApiError::bad_request("Autore non identificabile."));
    }
    let hash = comment.author_id.is_none().then(|| rules::email_hash(email.unwrap_or_default()));
    let user = actor(&ctx)?.user_id;
    let note =
        body.note.as_deref().map(str::trim).map(|n| n.chars().take(255).collect::<String>()).filter(|n| !n.is_empty());

    let mut tx = ctx.state.db.begin().await?;
    // un blocco per autore: due moderatori concorrenti non lo duplicano
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("blocked:{}", post.blog.id))
        .execute(&mut *tx)
        .await?;
    let existing: Option<(Uuid, String, Option<String>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, label, note, created_at FROM blog_blocked_authors
         WHERE blog_id = $1 AND (($2::uuid IS NOT NULL AND user_id = $2) OR ($3::text IS NOT NULL AND email_hash = $3))",
    )
    .bind(post.blog.id)
    .bind(comment.author_id)
    .bind(&hash)
    .fetch_optional(&mut *tx)
    .await?;
    let (id, label, note, created_at) = match existing {
        Some(e) => e,
        None => {
            let label = match comment.author_id {
                Some(uid) => {
                    let username: Option<String> = sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
                        .bind(uid)
                        .fetch_optional(&mut *tx)
                        .await?;
                    username.map(|u| format!("@{u}")).unwrap_or_else(|| comment.author_display_name.clone())
                }
                None => comment.author_display_name.clone(),
            };
            let (id, created_at): (Uuid, DateTime<Utc>) = sqlx::query_as(
                "INSERT INTO blog_blocked_authors (blog_id, user_id, email_hash, label, note, created_by_id)
                 VALUES ($1, $2, $3, $4, $5, $6) RETURNING id, created_at",
            )
            .bind(post.blog.id)
            .bind(comment.author_id)
            .bind(&hash)
            .bind(&label)
            .bind(&note)
            .bind(user)
            .fetch_one(&mut *tx)
            .await?;
            (id, label, note, created_at)
        }
    };
    sqlx::query("UPDATE comments SET status = 'rejected', updated_at = now() WHERE id = $1 AND status <> 'rejected'")
        .bind(comment.id)
        .execute(&mut *tx)
        .await?;
    audit(
        &mut tx,
        &ctx,
        "comment.author_blocked",
        comment.id,
        post.blog.id,
        json!({"post_slug": post.slug, "blog_slug": post.blog.slug, "anonymous": comment.author_id.is_none()}),
    )
    .await?;
    tx.commit().await?;
    Ok(blocked_out(id, &label, comment.author_id.is_none(), &note, created_at))
}

/// (id, etichetta, anonimo, nota, data)
type BlockedRow = (Uuid, String, bool, Option<String>, DateTime<Utc>);

pub async fn list_blocked_authors(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let blog = blog_of(&ctx)?;
    require_moderator(&ctx.state, actor(&ctx)?, blog).await?;
    let rows: Vec<BlockedRow> = sqlx::query_as(
        "SELECT id, label, user_id IS NULL, note, created_at FROM blog_blocked_authors WHERE blog_id = $1
         ORDER BY created_at DESC, id",
    )
    .bind(blog.id)
    .fetch_all(&ctx.state.db)
    .await?;
    Ok(rows.iter().map(|(id, label, anon, note, at)| blocked_out(*id, label, *anon, note, *at)).collect())
}

pub async fn unblock_author(ctx: RouteCtx) -> Result<(), ApiError> {
    let blog = blog_of(&ctx)?;
    require_moderator(&ctx.state, actor(&ctx)?, blog).await?;
    let done = sqlx::query("DELETE FROM blog_blocked_authors WHERE id = $1 AND blog_id = $2")
        .bind(path_uuid(&ctx, "block_id")?)
        .bind(blog.id)
        .execute(&ctx.state.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::not_found("Blocco non trovato."));
    }
    Ok(())
}

/// Commenti approvati di un utente su post pubblici di blog pubblici (tab «Commenti» del profilo).
/// Come per blog e post, solo quelli firmati con lo username: un commento con un altro nome non
/// deve collegare quel nome all'identità reale.
pub async fn list_user_comments(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let st = &ctx.state;
    let identifier = ctx.path.get("username").map(String::as_str).unwrap_or_default();
    let limit = ctx
        .query
        .get("limit")
        .map(|v| v.trim().parse::<i64>())
        .transpose()
        .map_err(|_| ApiError::unprocessable("`limit` deve essere un intero."))?
        .unwrap_or(20)
        .clamp(1, 50);
    let offset = ctx
        .query
        .get("offset")
        .map(|v| v.trim().parse::<i64>())
        .transpose()
        .map_err(|_| ApiError::unprocessable("`offset` deve essere un intero."))?
        .unwrap_or(0)
        .max(0);
    let user: Option<(Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT id, username, verified_domain FROM users
         WHERE is_active AND (lower(username) = lower($1) OR verified_domain = lower($1))
         ORDER BY (lower(username) = lower($1)) DESC LIMIT 1",
    )
    .bind(identifier)
    .fetch_optional(&st.db)
    .await?;
    let (user, username, domain) = user.ok_or_else(|| ApiError::not_found("Utente non trovato."))?;
    let rows: Vec<(Uuid, String, DateTime<Utc>, String, String, String)> = sqlx::query_as(&format!(
        "SELECT c.id, c.content, c.created_at, p.title, p.slug, b.slug
         FROM comments c JOIN posts p ON p.id = c.post_id JOIN blogs b ON b.id = p.blog_id
         WHERE c.author_id = $1 AND lower(c.author_display_name) IN (lower($2), lower($5)) AND c.status = 'approved' AND {VISIBLE_SQL}
           AND b.visibility = 'public' AND NOT b.is_suspended AND NOT b.is_paused AND b.deleted_at IS NULL
         ORDER BY c.created_at DESC, c.id LIMIT $3 OFFSET $4"
    ))
    .bind(user)
    .bind(&username)
    .bind(limit)
    .bind(offset)
    .bind(domain)
    .fetch_all(&st.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, content, created_at, post_title, post_slug, blog_slug)| {
            json!({"id": id, "content": content, "created_at": created_at, "post_title": post_title, "permalink": format!("/{blog_slug}/{post_slug}")})
        })
        .collect())
}
