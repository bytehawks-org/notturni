//! Handler `custom` del modulo: `GET /graph`.

use crate::rules::{BlogIn, Inputs, PostIn, build};
use chrono::{DateTime, Utc};
use noct_core::{ApiError, RouteCtx};
use serde_json::{Value, json};
use sqlx::FromRow;
use uuid::Uuid;

const DEFAULT_LIMIT: i64 = 300;
const MAX_LIMIT: i64 = 800;
const ADMIN_ROLES: &[&str] = &["super_admin", "amministratore"];

/// Come per gli elenchi pubblici: pubblico, non sospeso, non in pausa, non in cancellazione.
const LISTABLE: &str = "b.visibility = 'public' AND NOT b.is_suspended AND NOT b.is_paused AND b.deleted_at IS NULL";

#[derive(FromRow)]
struct BlogRow {
    id: Uuid,
    slug: String,
    title: String,
    kind: String,
}

#[derive(FromRow)]
struct PostRow {
    id: Uuid,
    blog_slug: String,
    slug: String,
    title: String,
    locale: String,
    status: String,
    published_at: Option<DateTime<Utc>>,
}

fn flag(ctx: &RouteCtx, name: &str) -> bool {
    matches!(ctx.query.get(name).map(|v| v.to_ascii_lowercase()).as_deref(), Some("true" | "1" | "yes"))
}

pub async fn get_graph(ctx: RouteCtx) -> Result<Value, ApiError> {
    let actor = ctx.actor.clone().ok_or_else(|| ApiError::unauthorized("Autenticazione richiesta."))?;
    let st = &ctx.state;
    let scope = match ctx.query.get("scope").map(String::as_str) {
        None | Some("") | Some("mine") => "mine",
        Some("platform") => "platform",
        Some(_) => return Err(ApiError::bad_request("`scope` ammette mine o platform.")),
    };
    if scope == "platform" && !ADMIN_ROLES.contains(&actor.platform_role.as_str()) {
        return Err(ApiError::forbidden("La mappa dell'intera piattaforma è riservata agli amministratori."));
    }
    let kind = match ctx.query.get("kind").map(String::as_str) {
        None | Some("") => None,
        Some(k @ ("blog" | "wiki" | "docs")) => Some(k),
        Some(_) => return Err(ApiError::bad_request("`kind` ammette blog, wiki o docs.")),
    };
    let blog_filter = ctx.query.get("blog").map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let limit = match ctx.query.get("limit") {
        Some(v) => v.parse::<i64>().map_err(|_| ApiError::bad_request("`limit` non valido."))?.clamp(1, MAX_LIMIT),
        None => DEFAULT_LIMIT,
    };
    // le bozze compaiono solo nella mappa dei propri Spazi
    let drafts = scope == "mine" && flag(&ctx, "drafts");

    let access = if scope == "mine" {
        "b.deleted_at IS NULL AND (b.owner_id = $1 OR EXISTS (SELECT 1 FROM blog_memberships m WHERE m.blog_id = b.id AND m.user_id = $1))"
    } else {
        LISTABLE
    };
    let blogs: Vec<BlogRow> = sqlx::query_as(&format!(
        "SELECT b.id, b.slug, b.title, b.kind::text AS kind FROM blogs b
         WHERE {access} AND ($2::text IS NULL OR b.slug = $2) AND ($3::text IS NULL OR b.kind::text = $3)
         ORDER BY b.slug"
    ))
    .bind(actor.user_id)
    .bind(&blog_filter)
    .bind(kind)
    .fetch_all(&st.db)
    .await?;
    if blogs.is_empty() && blog_filter.is_some() {
        return Err(ApiError::not_found("Spazio non trovato."));
    }
    let blog_ids: Vec<Uuid> = blogs.iter().map(|b| b.id).collect();

    let mut posts: Vec<PostRow> = sqlx::query_as(
        "SELECT p.id, b.slug AS blog_slug, p.slug, p.title, p.locale, p.status::text AS status, p.published_at
         FROM posts p JOIN blogs b ON b.id = p.blog_id
         WHERE p.blog_id = ANY($1) AND NOT p.is_hidden
           AND ($2 OR (p.status = 'published' AND p.published_at <= now()))
         ORDER BY COALESCE(p.published_at, p.updated_at) DESC, p.id
         LIMIT $3",
    )
    .bind(&blog_ids)
    .bind(drafts)
    .bind(limit + 1)
    .fetch_all(&st.db)
    .await?;
    let truncated = posts.len() as i64 > limit;
    posts.truncate(limit as usize);
    let post_ids: Vec<Uuid> = posts.iter().map(|p| p.id).collect();

    let tags: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT pt.post_id, t.name FROM post_tags pt JOIN tags t ON t.id = pt.tag_id WHERE pt.post_id = ANY($1)",
    )
    .bind(&post_ids)
    .fetch_all(&st.db)
    .await?;
    let urls: Vec<(Uuid, String)> = sqlx::query_as("SELECT post_id, url FROM post_links WHERE post_id = ANY($1)")
        .bind(&post_ids)
        .fetch_all(&st.db)
        .await?;
    // tabelle di moduli opzionali: presenti solo se il modulo è stato attivato
    let has = |name: &'static str| async move {
        sqlx::query_scalar::<_, bool>("SELECT to_regclass($1) IS NOT NULL").bind(name).fetch_one(&st.db).await
    };
    let categories: Vec<(Uuid, Uuid, String)> = if has("categories").await? {
        sqlx::query_as(
            "SELECT pc.post_id, c.id, c.name FROM post_categories pc JOIN categories c ON c.id = pc.category_id
             WHERE pc.post_id = ANY($1)",
        )
        .bind(&post_ids)
        .fetch_all(&st.db)
        .await?
    } else {
        Vec::new()
    };
    let wikilinks: Vec<(Uuid, String)> = if has("post_wikilinks").await? {
        sqlx::query_as("SELECT post_id, target_slug FROM post_wikilinks WHERE post_id = ANY($1)")
            .bind(&post_ids)
            .fetch_all(&st.db)
            .await?
    } else {
        Vec::new()
    };

    let inputs = Inputs {
        blogs: blogs.into_iter().map(|b| BlogIn { slug: b.slug, title: b.title, kind: b.kind }).collect(),
        posts: posts
            .into_iter()
            .map(|p| PostIn {
                id: p.id,
                blog_slug: p.blog_slug,
                slug: p.slug,
                title: p.title,
                locale: p.locale,
                status: p.status,
                published_at: p.published_at,
            })
            .collect(),
        tags,
        categories,
        urls,
        wikilinks,
    };
    let graph = build(&inputs, &st.config.instance_fqdn);
    Ok(json!({"scope": scope, "nodes": graph.nodes, "edges": graph.edges, "truncated": truncated}))
}
