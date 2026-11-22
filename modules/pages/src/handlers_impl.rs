//! Handler `custom` del modulo. Le pagine dei blog e quelle del sito hanno lo stesso schema in
//! due tabelle: la logica è unica e dipende solo dall'ambito (`Scope`).

use crate::{PageCreate, PageTranslation, PageUpdate};
use chrono::{DateTime, Utc};
use noct_audit::{Entry, user_actor};
use noct_core::{ApiError, BlogRef, RouteCtx, can_write_blog, map_db_error, parse_uuid, revalidate};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool, QueryBuilder};
use uuid::Uuid;

const SLUG_TAKEN: (&str, u16, &str) = ("uq_blog_page_slug_locale", 409, "Slug già in uso per questa lingua.");
const PLATFORM_SLUG_TAKEN: (&str, u16, &str) =
    ("uq_platform_page_slug_locale", 409, "Slug già in uso per questa lingua.");
const TRANSLATION_TAKEN: (&str, u16, &str) =
    ("blog_page_translation_locale", 409, "Traduzione già presente per questa lingua.");
const PLATFORM_TRANSLATION_TAKEN: (&str, u16, &str) =
    ("platform_page_translation_locale", 409, "Traduzione già presente per questa lingua.");

#[derive(Clone, Copy)]
enum Scope<'a> {
    Blog(&'a BlogRef),
    Platform,
}

impl Scope<'_> {
    fn table(self) -> &'static str {
        match self {
            Scope::Blog(_) => "blog_pages",
            Scope::Platform => "platform_pages",
        }
    }
    fn blog_id(self) -> Option<Uuid> {
        match self {
            Scope::Blog(b) => Some(b.id),
            Scope::Platform => None,
        }
    }
    /// Condizione sull'ambito, con il blog (o NULL) legato al segnaposto `$n`.
    fn scope_filter(self, n: u8) -> String {
        match self {
            Scope::Blog(_) => format!("blog_id = ${n}"),
            Scope::Platform => format!("${n}::uuid IS NULL"),
        }
    }
    fn blog_column(self) -> &'static str {
        match self {
            Scope::Blog(_) => "blog_id",
            Scope::Platform => "NULL::uuid AS blog_id",
        }
    }
    fn conflicts(self) -> [(&'static str, u16, &'static str); 2] {
        match self {
            Scope::Blog(_) => [SLUG_TAKEN, TRANSLATION_TAKEN],
            Scope::Platform => [PLATFORM_SLUG_TAKEN, PLATFORM_TRANSLATION_TAKEN],
        }
    }
    fn permalink(self, slug: &str) -> String {
        match self {
            Scope::Blog(b) => format!("/{}/pagina/{slug}", b.slug),
            Scope::Platform => format!("/p/{slug}"),
        }
    }
    fn invalidate(self, ctx: &RouteCtx, slug: &str) {
        let tags = match self {
            Scope::Blog(b) => vec![revalidate::blog(&b.slug), revalidate::blog_page(&b.slug, slug)],
            Scope::Platform => vec![revalidate::platform_pages(), revalidate::platform_page(slug)],
        };
        ctx.state.services.revalidate.spawn(tags);
    }
}

#[derive(sqlx::FromRow)]
struct PageRow {
    id: Uuid,
    blog_id: Option<Uuid>,
    slug: String,
    locale: String,
    translation_group_id: Uuid,
    title: String,
    content: String,
    is_published: bool,
    created_at: DateTime<Utc>,
}

fn columns(scope: Scope) -> String {
    format!("id, {}, slug, locale, translation_group_id, title, content, is_published, created_at", scope.blog_column())
}

fn blog_of(ctx: &RouteCtx) -> Result<&BlogRef, ApiError> {
    ctx.blog.as_ref().ok_or_else(|| ApiError::internal("scope `blog` assente"))
}

fn key(ctx: &RouteCtx) -> Result<&str, ApiError> {
    ctx.path.get("key").map(String::as_str).ok_or_else(|| ApiError::internal("parametro `key` assente"))
}

fn locale_query(ctx: &RouteCtx) -> Result<&str, ApiError> {
    ctx.query
        .get("locale")
        .map(String::as_str)
        .ok_or_else(|| ApiError::unprocessable("Il parametro `locale` è obbligatorio."))
}

fn user_id(ctx: &RouteCtx) -> Result<Uuid, ApiError> {
    ctx.actor.as_ref().map(|a| a.user_id).ok_or_else(|| ApiError::unauthorized("Autenticazione richiesta."))
}

fn is_admin(ctx: &RouteCtx) -> bool {
    ctx.actor.as_ref().is_some_and(|a| matches!(a.platform_role.as_str(), "super_admin" | "amministratore"))
}

async fn can_see_drafts(ctx: &RouteCtx, scope: Scope<'_>) -> Result<bool, ApiError> {
    match scope {
        Scope::Platform => Ok(is_admin(ctx)),
        Scope::Blog(b) => can_write_blog(&ctx.state, ctx.actor.as_ref().map(|a| a.user_id), b).await,
    }
}

async fn mentions_enabled(db: &PgPool, scope: Scope<'_>) -> Result<bool, ApiError> {
    match scope.blog_id() {
        Some(id) => {
            Ok(sqlx::query_scalar("SELECT mentions_enabled FROM blogs WHERE id = $1").bind(id).fetch_one(db).await?)
        }
        None => Ok(true),
    }
}

fn render(scope: Scope, mentions: bool, p: &PageRow) -> Value {
    json!({
        "id": p.id, "blog_id": p.blog_id, "slug": p.slug, "locale": p.locale,
        "translation_group_id": p.translation_group_id, "title": p.title, "content": p.content,
        "is_published": p.is_published, "created_at": p.created_at,
        "permalink": scope.permalink(&p.slug), "mentions_enabled": mentions,
    })
}

async fn render_all(ctx: &RouteCtx, scope: Scope<'_>, rows: &[PageRow]) -> Result<Vec<Value>, ApiError> {
    let mentions = mentions_enabled(&ctx.state.db, scope).await?;
    Ok(rows.iter().map(|p| render(scope, mentions, p)).collect())
}

async fn find_by_id(db: &PgPool, scope: Scope<'_>, id: Uuid) -> Result<PageRow, ApiError> {
    let sql = format!("SELECT {} FROM {} WHERE id = $1 AND {}", columns(scope), scope.table(), scope.scope_filter(2));
    sqlx::query_as(&sql)
        .bind(id)
        .bind(scope.blog_id())
        .fetch_optional(db)
        .await?
        .ok_or_else(|| ApiError::not_found("Pagina non trovata."))
}

async fn audit(
    conn: &mut PgConnection,
    ctx: &RouteCtx,
    scope: Scope<'_>,
    action: &str,
    page: &PageRow,
) -> Result<(), ApiError> {
    let actor = user_actor(&mut *conn, user_id(ctx)?).await.map_err(ApiError::internal)?;
    let mut payload = json!({"slug": page.slug, "locale": page.locale, "is_published": page.is_published});
    if let (Scope::Blog(b), Some(map)) = (scope, payload.as_object_mut()) {
        map.insert("blog_slug".into(), json!(b.slug));
    }
    noct_audit::record(
        conn,
        Entry { action, actor, target: Some(("page", page.id)), blog_id: scope.blog_id(), meta: &ctx.meta, payload },
    )
    .await
    .map_err(ApiError::internal)
}

async fn require_static_pages(db: &PgPool, blog: &BlogRef) -> Result<(), ApiError> {
    let enabled: bool = sqlx::query_scalar(
        "SELECT coalesce((SELECT static_pages_enabled FROM blog_pages_settings WHERE blog_id = $1), false)",
    )
    .bind(blog.id)
    .fetch_one(db)
    .await?;
    if enabled { Ok(()) } else { Err(ApiError::forbidden("Le pagine statiche non sono attive per questo blog.")) }
}

// ---- operazioni comuni --------------------------------------------------------------------------------------------------

async fn list(ctx: &RouteCtx, scope: Scope<'_>, q: Option<&str>) -> Result<Vec<Value>, ApiError> {
    let locale = locale_query(ctx)?;
    let drafts = can_see_drafts(ctx, scope).await?;
    let needle = q
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{}%", s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")));
    let mut qb = QueryBuilder::new(format!("SELECT {} FROM {} WHERE locale = ", columns(scope), scope.table()));
    qb.push_bind(locale.to_string());
    if let Some(id) = scope.blog_id() {
        qb.push(" AND blog_id = ").push_bind(id);
    }
    if !drafts {
        qb.push(" AND is_published");
    }
    if let Some(n) = needle {
        qb.push(" AND (title ILIKE ").push_bind(n.clone()).push(" OR slug ILIKE ").push_bind(n).push(")");
    }
    qb.push(" ORDER BY slug, id");
    let rows: Vec<PageRow> = qb.build_query_as().fetch_all(&ctx.state.db).await?;
    render_all(ctx, scope, &rows).await
}

async fn get_by_slug(ctx: &RouteCtx, scope: Scope<'_>) -> Result<Value, ApiError> {
    let locale = locale_query(ctx)?;
    let drafts = can_see_drafts(ctx, scope).await?;
    let sql = format!(
        "SELECT {} FROM {} WHERE slug = $1 AND locale = $2 AND {} AND ($4 OR is_published)",
        columns(scope),
        scope.table(),
        scope.scope_filter(3)
    );
    let page: PageRow = sqlx::query_as(&sql)
        .bind(key(ctx)?)
        .bind(locale)
        .bind(scope.blog_id())
        .bind(drafts)
        .fetch_optional(&ctx.state.db)
        .await?
        .ok_or_else(|| ApiError::not_found("Pagina non trovata."))?;
    Ok(render(scope, mentions_enabled(&ctx.state.db, scope).await?, &page))
}

async fn create(
    ctx: &RouteCtx,
    scope: Scope<'_>,
    slug: &str,
    locale: &str,
    title: &str,
    content: &str,
    published: Option<bool>,
) -> Result<Value, ApiError> {
    let user = user_id(ctx)?;
    let mut tx = ctx.state.db.begin().await?;
    let mut qb = QueryBuilder::new(format!("INSERT INTO {} (", scope.table()));
    if scope.blog_id().is_some() {
        qb.push("blog_id, ");
    }
    qb.push("slug, locale, title, content, is_published, updated_by_id) VALUES (");
    if let Some(id) = scope.blog_id() {
        qb.push_bind(id).push(", ");
    }
    qb.push_bind(slug.to_string()).push(", ").push_bind(locale.to_string()).push(", ").push_bind(title.to_string());
    qb.push(", ").push_bind(content.to_string()).push(", ").push_bind(published.unwrap_or(false));
    qb.push(", ").push_bind(user).push(") RETURNING ").push(columns(scope));
    let page: PageRow =
        qb.build_query_as().fetch_one(&mut *tx).await.map_err(|e| map_db_error(e, &scope.conflicts()))?;
    audit(&mut tx, ctx, scope, "page.created", &page).await?;
    tx.commit().await?;
    scope.invalidate(ctx, &page.slug);
    Ok(render(scope, mentions_enabled(&ctx.state.db, scope).await?, &page))
}

async fn translate(ctx: &RouteCtx, scope: Scope<'_>, body: &PageTranslation) -> Result<Value, ApiError> {
    let user = user_id(ctx)?;
    let original = find_by_id(&ctx.state.db, scope, parse_uuid(key(ctx)?)?).await?;
    let mut qb = QueryBuilder::new(format!("INSERT INTO {} (", scope.table()));
    if scope.blog_id().is_some() {
        qb.push("blog_id, ");
    }
    qb.push("slug, locale, translation_group_id, title, content, is_published, updated_by_id) VALUES (");
    if let Some(id) = scope.blog_id() {
        qb.push_bind(id).push(", ");
    }
    qb.push_bind(body.slug.clone()).push(", ").push_bind(body.locale.clone()).push(", ");
    qb.push_bind(original.translation_group_id).push(", ").push_bind(body.title.clone()).push(", ");
    qb.push_bind(body.content.clone()).push(", ").push_bind(body.is_published.unwrap_or(false));
    qb.push(", ").push_bind(user).push(") RETURNING ").push(columns(scope));
    let page: PageRow =
        qb.build_query_as().fetch_one(&ctx.state.db).await.map_err(|e| map_db_error(e, &scope.conflicts()))?;
    scope.invalidate(ctx, &page.slug);
    Ok(render(scope, mentions_enabled(&ctx.state.db, scope).await?, &page))
}

async fn translations(ctx: &RouteCtx, scope: Scope<'_>) -> Result<Vec<Value>, ApiError> {
    let original = find_by_id(&ctx.state.db, scope, parse_uuid(key(ctx)?)?).await?;
    let rows: Vec<(Uuid, String, String, bool)> = sqlx::query_as(&format!(
        "SELECT id, locale, slug, is_published FROM {} WHERE translation_group_id = $1 AND is_published ORDER BY locale",
        scope.table()
    ))
    .bind(original.translation_group_id)
    .fetch_all(&ctx.state.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, locale, slug, p)| json!({"id": id, "locale": locale, "slug": slug, "is_published": p}))
        .collect())
}

async fn update(ctx: &RouteCtx, scope: Scope<'_>, body: &PageUpdate) -> Result<Value, ApiError> {
    let user = user_id(ctx)?;
    let current = find_by_id(&ctx.state.db, scope, parse_uuid(key(ctx)?)?).await?;
    let mut qb = QueryBuilder::new(format!("UPDATE {} SET updated_at = now(), updated_by_id = ", scope.table()));
    qb.push_bind(user);
    if let Some(v) = &body.slug {
        qb.push(", slug = ").push_bind(v.clone());
    }
    if let Some(v) = &body.title {
        qb.push(", title = ").push_bind(v.clone());
    }
    if let Some(v) = &body.content {
        qb.push(", content = ").push_bind(v.clone());
    }
    if let Some(v) = body.is_published {
        qb.push(", is_published = ").push_bind(v);
    }
    qb.push(" WHERE id = ").push_bind(current.id).push(" RETURNING ").push(columns(scope));
    let mut tx = ctx.state.db.begin().await?;
    let page: PageRow =
        qb.build_query_as().fetch_one(&mut *tx).await.map_err(|e| map_db_error(e, &scope.conflicts()))?;
    audit(&mut tx, ctx, scope, "page.updated", &page).await?;
    tx.commit().await?;
    // se lo slug è cambiato va invalidata anche la vecchia pagina
    scope.invalidate(ctx, &current.slug);
    scope.invalidate(ctx, &page.slug);
    Ok(render(scope, mentions_enabled(&ctx.state.db, scope).await?, &page))
}

// ---- pagine dei blog ----------------------------------------------------------------------------------------------------

pub async fn list_blog_pages(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    list(&ctx, Scope::Blog(blog_of(&ctx)?), None).await
}

pub async fn get_blog_page_by_id(ctx: RouteCtx) -> Result<Value, ApiError> {
    let scope = Scope::Blog(blog_of(&ctx)?);
    let id = parse_uuid(ctx.path.get("page_id").map(String::as_str).unwrap_or_default())?;
    let page = find_by_id(&ctx.state.db, scope, id).await?;
    Ok(render(scope, mentions_enabled(&ctx.state.db, scope).await?, &page))
}

pub async fn get_blog_page(ctx: RouteCtx) -> Result<Value, ApiError> {
    get_by_slug(&ctx, Scope::Blog(blog_of(&ctx)?)).await
}

pub async fn create_blog_page(ctx: RouteCtx, body: PageCreate) -> Result<Value, ApiError> {
    let blog = blog_of(&ctx)?;
    require_static_pages(&ctx.state.db, blog).await?;
    create(&ctx, Scope::Blog(blog), &body.slug, &body.locale, &body.title, &body.content, body.is_published).await
}

pub async fn add_blog_page_translation(ctx: RouteCtx, body: PageTranslation) -> Result<Value, ApiError> {
    let blog = blog_of(&ctx)?;
    require_static_pages(&ctx.state.db, blog).await?;
    translate(&ctx, Scope::Blog(blog), &body).await
}

pub async fn list_blog_page_translations(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    translations(&ctx, Scope::Blog(blog_of(&ctx)?)).await
}

/// Modifica e rimozione restano possibili anche a funzione disattivata: `static_pages_enabled`
/// blocca solo la creazione di pagine nuove.
pub async fn update_blog_page(ctx: RouteCtx, body: PageUpdate) -> Result<Value, ApiError> {
    update(&ctx, Scope::Blog(blog_of(&ctx)?), &body).await
}

pub async fn delete_blog_page(ctx: RouteCtx) -> Result<(), ApiError> {
    let scope = Scope::Blog(blog_of(&ctx)?);
    let page = find_by_id(&ctx.state.db, scope, parse_uuid(key(&ctx)?)?).await?;
    let mut tx = ctx.state.db.begin().await?;
    sqlx::query("DELETE FROM blog_pages WHERE id = $1").bind(page.id).execute(&mut *tx).await?;
    audit(&mut tx, &ctx, scope, "page.deleted", &page).await?;
    tx.commit().await?;
    scope.invalidate(&ctx, &page.slug);
    Ok(())
}

// ---- pagine del sito ----------------------------------------------------------------------------------------------------

pub async fn list_platform_pages(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let q = ctx.query.get("q").cloned();
    list(&ctx, Scope::Platform, q.as_deref()).await
}

pub async fn get_platform_page(ctx: RouteCtx) -> Result<Value, ApiError> {
    get_by_slug(&ctx, Scope::Platform).await
}

pub async fn create_platform_page(ctx: RouteCtx, body: PageCreate) -> Result<Value, ApiError> {
    create(&ctx, Scope::Platform, &body.slug, &body.locale, &body.title, &body.content, body.is_published).await
}

pub async fn add_platform_page_translation(ctx: RouteCtx, body: PageTranslation) -> Result<Value, ApiError> {
    translate(&ctx, Scope::Platform, &body).await
}

pub async fn list_platform_page_translations(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    translations(&ctx, Scope::Platform).await
}

pub async fn update_platform_page(ctx: RouteCtx, body: PageUpdate) -> Result<Value, ApiError> {
    update(&ctx, Scope::Platform, &body).await
}
