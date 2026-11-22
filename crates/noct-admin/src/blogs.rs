use crate::handlers_impl::{Target, audit, path_uuid, require_note, text_filter};
use chrono::{DateTime, Utc};
use noct_core::{ApiError, AppState, RouteCtx, revalidate};
use serde_json::{Value, json};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

/// Le segnalazioni aperte esistono solo se il modulo `reports` è attivo.
async fn reports_enabled(st: &AppState) -> bool {
    st.module_enabled("reports")
}

#[derive(FromRow)]
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

fn blog_select(with_reports: bool) -> String {
    let reports = if with_reports {
        "(SELECT count(*) FROM content_reports r WHERE r.blog_id = b.id AND r.status = 'open')"
    } else {
        "0::bigint"
    };
    format!(
        "SELECT b.id, b.slug, b.title, u.username AS owner_username, b.visibility::text AS visibility, b.is_suspended,
                b.is_paused, b.deleted_at, b.created_at, (SELECT count(*) FROM posts p WHERE p.blog_id = b.id) AS posts_count,
                {reports} AS reports_open
         FROM blogs b JOIN users u ON u.id = b.owner_id"
    )
}

fn blog_out(r: BlogRow) -> Value {
    json!({
        "id": r.id, "slug": r.slug, "title": r.title, "owner_username": r.owner_username, "visibility": r.visibility,
        "is_suspended": r.is_suspended, "is_paused": r.is_paused, "deleted_at": r.deleted_at,
        "created_at": r.created_at, "posts_count": r.posts_count, "reports_open": r.reports_open,
    })
}

pub async fn list_blogs(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let st = &ctx.state;
    let reports = reports_enabled(st).await;
    let q = text_filter(&ctx, "q");
    let visibility = text_filter(&ctx, "visibility");
    if let Some(v) = &visibility
        && !["public", "members", "private"].contains(&v.as_str())
    {
        return Err(ApiError::unprocessable("`visibility` non valido."));
    }
    let state = text_filter(&ctx, "state");
    let state_sql = match state.as_deref() {
        None => "TRUE".to_string(),
        Some("active") => "(NOT b.is_suspended AND NOT b.is_paused AND b.deleted_at IS NULL)".into(),
        Some("suspended") => "b.is_suspended".into(),
        Some("paused") => "b.is_paused".into(),
        Some("deleted") => "b.deleted_at IS NOT NULL".into(),
        Some("reported") if reports => {
            "EXISTS (SELECT 1 FROM content_reports r WHERE r.blog_id = b.id AND r.status = 'open')".into()
        }
        // senza il modulo non ci sono segnalazioni: l'elenco è vuoto
        Some("reported") => "FALSE".into(),
        Some(_) => return Err(ApiError::unprocessable("`state` non valido.")),
    };
    let mut rows: Vec<BlogRow> = sqlx::query_as(&format!(
        "{} WHERE ($1::text IS NULL OR position(lower($1) IN lower(b.slug)) > 0
                OR position(lower($1) IN lower(b.title)) > 0 OR position(lower($1) IN lower(u.username)) > 0)
           AND ($2::text IS NULL OR b.visibility::text = $2) AND {state_sql}
         ORDER BY b.created_at, b.id",
        blog_select(reports)
    ))
    .bind(q)
    .bind(visibility)
    .fetch_all(&st.db)
    .await?;
    if state.as_deref() == Some("reported") {
        rows.sort_by_key(|b| std::cmp::Reverse(b.reports_open));
    }
    Ok(rows.into_iter().map(blog_out).collect())
}

pub async fn update_blog(ctx: RouteCtx, body: crate::AdminBlogUpdate) -> Result<Value, ApiError> {
    let id = path_uuid(&ctx, "blog_id")?;
    let reports = reports_enabled(&ctx.state).await;
    let mut tx = ctx.state.db.begin().await?;
    let current: Option<(String, bool, Option<String>)> =
        sqlx::query_as("SELECT slug, is_suspended, default_author_display_name FROM blogs WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let (slug, suspended, alias) = current.ok_or_else(|| ApiError::not_found("Blog non trovato."))?;
    let changed = body.is_suspended != suspended;
    if changed {
        let note = require_note(body.note.as_deref())?;
        audit(
            &mut tx,
            &ctx,
            if body.is_suspended { "blog.suspended" } else { "blog.unsuspended" },
            Some(Target { kind: "blog", id, blog: Some(id) }),
            json!({"slug": slug, "blog_alias": alias, "note": note}),
        )
        .await?;
        sqlx::query("UPDATE blogs SET is_suspended = $2, updated_at = now() WHERE id = $1")
            .bind(id)
            .bind(body.is_suspended)
            .execute(&mut *tx)
            .await?;
    }
    let row: BlogRow =
        sqlx::query_as(&format!("{} WHERE b.id = $1", blog_select(reports))).bind(id).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    if changed {
        // la sospensione blocca lettura e scrittura pubbliche: pagine del blog e home da rigenerare
        ctx.state.services.revalidate.spawn(vec![revalidate::blog(&slug), revalidate::feed()]);
    }
    Ok(blog_out(row))
}

// ---- post -----------------------------------------------------------------------------------------------------------

#[derive(FromRow)]
struct PostRow {
    id: Uuid,
    title: String,
    slug: String,
    blog_slug: String,
    blog_title: String,
    author_username: String,
    status: String,
    is_hidden: bool,
    published_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    reports_open: i64,
}

fn post_select(with_reports: bool) -> String {
    let reports = if with_reports {
        "(SELECT count(*) FROM content_reports r WHERE r.target_type = 'post' AND r.target_id = p.id AND r.status = 'open')"
    } else {
        "0::bigint"
    };
    format!(
        "SELECT p.id, p.title, p.slug, b.slug AS blog_slug, b.title AS blog_title, u.username AS author_username,
                p.status::text AS status, p.is_hidden, p.published_at, p.created_at, {reports} AS reports_open
         FROM posts p JOIN blogs b ON b.id = p.blog_id JOIN users u ON u.id = p.author_id"
    )
}

fn post_out(r: PostRow) -> Value {
    json!({
        "id": r.id, "title": r.title, "slug": r.slug, "blog_slug": r.blog_slug, "blog_title": r.blog_title,
        "author_username": r.author_username, "status": r.status, "is_hidden": r.is_hidden,
        "published_at": r.published_at, "created_at": r.created_at, "reports_open": r.reports_open,
    })
}

pub async fn list_posts(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let reports = reports_enabled(&ctx.state).await;
    let rows: Vec<PostRow> = sqlx::query_as(&format!(
        "{} WHERE $1::text IS NULL OR position(lower($1) IN lower(p.title)) > 0 OR position(lower($1) IN lower(p.slug)) > 0
                OR position(lower($1) IN lower(b.slug)) > 0 OR position(lower($1) IN lower(u.username)) > 0
         ORDER BY p.created_at DESC, p.id",
        post_select(reports)
    ))
    .bind(text_filter(&ctx, "q"))
    .fetch_all(&ctx.state.db)
    .await?;
    Ok(rows.into_iter().map(post_out).collect())
}

pub async fn update_post(ctx: RouteCtx, body: crate::AdminPostUpdate) -> Result<Value, ApiError> {
    let id = path_uuid(&ctx, "post_id")?;
    let reports = reports_enabled(&ctx.state).await;
    let mut tx = ctx.state.db.begin().await?;
    let current: Option<(Uuid, String, bool, String, Option<String>)> = sqlx::query_as(
        "SELECT p.blog_id, p.slug, p.is_hidden, b.slug, b.default_author_display_name
         FROM posts p JOIN blogs b ON b.id = p.blog_id WHERE p.id = $1 FOR UPDATE OF p",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let (blog_id, slug, hidden, blog_slug, alias) = current.ok_or_else(|| ApiError::not_found("Post non trovato."))?;
    let changed = body.is_hidden != hidden;
    if changed {
        let note = require_note(body.note.as_deref())?;
        audit(
            &mut tx,
            &ctx,
            if body.is_hidden { "post.hidden" } else { "post.unhidden" },
            Some(Target { kind: "post", id, blog: Some(blog_id) }),
            json!({"slug": slug, "blog_slug": blog_slug, "blog_alias": alias, "note": note}),
        )
        .await?;
        set_hidden(&mut tx, id, body.is_hidden).await?;
    }
    let row: PostRow =
        sqlx::query_as(&format!("{} WHERE p.id = $1", post_select(reports))).bind(id).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    if changed {
        ctx.state.services.revalidate.spawn(vec![
            revalidate::post(&blog_slug, &slug),
            revalidate::blog(&blog_slug),
            revalidate::feed(),
        ]);
    }
    Ok(post_out(row))
}

async fn set_hidden(conn: &mut PgConnection, id: Uuid, hidden: bool) -> Result<(), ApiError> {
    sqlx::query("UPDATE posts SET is_hidden = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(hidden)
        .execute(conn)
        .await?;
    Ok(())
}
