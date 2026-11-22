use crate::{
    LinkBibliographyEntry, MediaBibliographyEntry, MyLinkEntry, PostCreate, PostRow, PostTranslation, PostUpdate,
    PublishRequest, TranslationSummary, VISIBLE_SQL, render,
    rules::{self, UserNames},
};
use chrono::{DateTime, Utc};
use noct_core::{
    Actor, ApiError, AppState, BlogRef, Entity, ExtCtx, Op, RouteCtx, body_map,
    bus::{Kind, outbox},
    can_view_blog, can_write_blog, event_envelope, load_blog, map_db_error, membership_role, parse_uuid, revalidate,
};
use serde_json::{Value, json};
use sqlx::{PgConnection, Postgres, QueryBuilder};
use std::collections::HashMap;
use uuid::Uuid;

const MSG_WRITE: &str = "Serve essere proprietario del blog o avere ruolo autore/co-autore.";
const MSG_SLUG_TAKEN: &str = "Slug già in uso su questo blog per questa lingua.";
const SLUG_CONFLICT: [(&str, u16, &str); 1] = [("uq_post_blog_slug_locale", 409, MSG_SLUG_TAKEN)];

fn actor(ctx: &RouteCtx) -> Result<Actor, ApiError> {
    ctx.actor.clone().ok_or_else(|| ApiError::unauthorized("Not authenticated"))
}

fn scoped_blog(ctx: &RouteCtx) -> Result<&BlogRef, ApiError> {
    ctx.blog.as_ref().ok_or_else(|| ApiError::internal("scope del blog assente"))
}

fn path_uuid(ctx: &RouteCtx, name: &str) -> Result<Uuid, ApiError> {
    parse_uuid(ctx.path.get(name).map(String::as_str).unwrap_or_default())
}

fn not_found() -> ApiError {
    ApiError::not_found("Post non trovato.")
}

async fn fetch_row(conn: &mut PgConnection, id: Uuid, lock: bool) -> Result<Option<PostRow>, ApiError> {
    let sql = format!("{} WHERE p.id = $1{}", PostRow::SELECT, if lock { " FOR UPDATE OF p" } else { "" });
    Ok(sqlx::query_as::<_, PostRow>(&sql).bind(id).fetch_optional(conn).await?)
}

async fn fetch_row_pool(st: &AppState, id: Uuid) -> Result<Option<PostRow>, ApiError> {
    Ok(sqlx::query_as::<_, PostRow>(&format!("{} WHERE p.id = $1", PostRow::SELECT))
        .bind(id)
        .fetch_optional(&st.db)
        .await?)
}

async fn row_or_404(st: &AppState, id: Uuid) -> Result<PostRow, ApiError> {
    fetch_row_pool(st, id).await?.ok_or_else(not_found)
}

async fn blog_of(st: &AppState, row: &PostRow) -> Result<BlogRef, ApiError> {
    load_blog(st, &row.blog_slug).await?.ok_or_else(|| ApiError::not_found("Blog non trovato."))
}

async fn render_one(st: &AppState, row: PostRow) -> Result<Value, ApiError> {
    render(st, vec![row]).await?.pop().ok_or_else(not_found)
}

fn revalidate_post(st: &AppState, row: &PostRow) {
    st.services.revalidate.spawn(vec![
        revalidate::feed(),
        revalidate::blog(&row.blog_slug),
        revalidate::post(&row.blog_slug, &row.slug),
    ]);
}

async fn can_review(st: &AppState, user: Uuid, blog: &BlogRef) -> Result<bool, ApiError> {
    Ok(blog.owner_id == user || membership_role(st, user, blog.id).await?.as_deref() == Some("revisore"))
}

/// Nome pubblico con cui `user` firma i post di `blog` (alias di membership → nome del blog → profilo).
async fn author_name(conn: &mut PgConnection, user: Uuid, blog: Uuid) -> Result<String, ApiError> {
    let (alias, default): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT (SELECT author_display_name FROM blog_memberships WHERE user_id = $1 AND blog_id = $2),
                (SELECT default_author_display_name FROM blogs WHERE id = $2)",
    )
    .bind(user)
    .bind(blog)
    .fetch_one(&mut *conn)
    .await?;
    let (username, display_name, first_name, last_name, style, verified_domain): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT username, display_name, first_name, last_name, post_author_name_style::text, verified_domain
         FROM users WHERE id = $1",
    )
    .bind(user)
    .fetch_one(&mut *conn)
    .await?;
    Ok(rules::pick_display_name(
        alias.as_deref(),
        default.as_deref(),
        &UserNames { username, display_name, first_name, last_name, style, verified_domain },
    ))
}

/// Sostituisce i tag effettivi del post (tag creati al bisogno).
async fn sync_tags(conn: &mut PgConnection, post: Uuid, tags: &[String]) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM post_tags WHERE post_id = $1").bind(post).execute(&mut *conn).await?;
    if tags.is_empty() {
        return Ok(());
    }
    sqlx::query("INSERT INTO tags (name) SELECT unnest($1::text[]) ON CONFLICT (name) DO NOTHING")
        .bind(tags)
        .execute(&mut *conn)
        .await?;
    sqlx::query("INSERT INTO post_tags (post_id, tag_id) SELECT $1, id FROM tags WHERE name = ANY($2)")
        .bind(post)
        .bind(tags)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Riscrive la cache di media e link citati a partire dal Markdown (unica fonte di verità).
async fn sync_content_refs(conn: &mut PgConnection, post: Uuid, content: &str) -> Result<(), ApiError> {
    let media: Vec<Value> = rules::extract_media(content)
        .into_iter()
        .map(|m| json!({"position": m.position, "url": m.url, "alt_text": m.alt_text, "categories": m.categories, "is_sensitive": m.is_sensitive}))
        .collect();
    let links: Vec<Value> = rules::extract_links(content)
        .into_iter()
        .map(|l| json!({"position": l.position, "url": l.url, "link_text": l.link_text}))
        .collect();
    sqlx::query("DELETE FROM post_media WHERE post_id = $1").bind(post).execute(&mut *conn).await?;
    sqlx::query("DELETE FROM post_links WHERE post_id = $1").bind(post).execute(&mut *conn).await?;
    if !media.is_empty() {
        sqlx::query(
            "INSERT INTO post_media (post_id, position, url, alt_text, categories, is_sensitive)
             SELECT $1, x.position, x.url, x.alt_text, ARRAY(SELECT jsonb_array_elements_text(x.categories)), x.is_sensitive
             FROM jsonb_to_recordset($2::jsonb) AS x(position int, url text, alt_text text, categories jsonb, is_sensitive bool)",
        )
        .bind(post)
        .bind(Value::Array(media))
        .execute(&mut *conn)
        .await?;
    }
    if !links.is_empty() {
        sqlx::query(
            "INSERT INTO post_links (post_id, position, url, link_text)
             SELECT $1, x.position, x.url, x.link_text
             FROM jsonb_to_recordset($2::jsonb) AS x(position int, url text, link_text text)",
        )
        .bind(post)
        .bind(Value::Array(links))
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

async fn emit(
    conn: &mut PgConnection,
    ctx: &RouteCtx,
    blog: &BlogRef,
    actor: &Actor,
    subject: &str,
    data: Value,
) -> Result<(), ApiError> {
    let envelope = event_envelope(crate::MODULE_NAME, Some(blog), Some(actor), data);
    outbox::enqueue(conn, Kind::Event, subject, &envelope).await.map(|_| ()).map_err(ApiError::internal)?;
    let _ = ctx;
    Ok(())
}

/// Evento "post salvato": consumato dal backup su storage. Contiene il testo perché il
/// consumer non deve rileggere uno stato che nel frattempo può essere cambiato.
async fn emit_saved(
    conn: &mut PgConnection,
    ctx: &RouteCtx,
    blog: &BlogRef,
    actor: &Actor,
    row: &PostRow,
) -> Result<(), ApiError> {
    emit(
        conn,
        ctx,
        blog,
        actor,
        "posts.post.saved",
        json!({
            "post_id": row.id, "blog_id": row.blog_id, "owner_id": row.blog_owner_id,
            "title": row.title, "content": row.content, "locale": row.locale,
        }),
    )
    .await
}

struct NewPost<'a> {
    id: Uuid,
    group: Uuid,
    locale: &'a str,
    slug: &'a str,
    title: &'a str,
    content: &'a str,
    cover_url: Option<&'a str>,
    cover_categories: Vec<String>,
    cover_sensitive: bool,
    cover_alt: &'a str,
    manual_tags: Vec<String>,
    effective_tags: Vec<String>,
}

async fn insert_post(conn: &mut PgConnection, blog: &BlogRef, author: Uuid, p: &NewPost<'_>) -> Result<(), ApiError> {
    let name = author_name(conn, author, blog.id).await?;
    sqlx::query(
        "INSERT INTO posts (id, blog_id, author_id, author_display_name, locale, translation_group_id, title, slug, content,
                            cover_image_url, cover_image_is_sensitive, cover_image_categories, cover_image_alt_text, manual_tags)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
    )
    .bind(p.id)
    .bind(blog.id)
    .bind(author)
    .bind(name)
    .bind(p.locale)
    .bind(p.group)
    .bind(p.title)
    .bind(p.slug)
    .bind(p.content)
    .bind(p.cover_url)
    .bind(p.cover_sensitive)
    .bind(&p.cover_categories)
    .bind(p.cover_alt)
    .bind(&p.manual_tags)
    .execute(&mut *conn)
    .await
    .map_err(|e| map_db_error(e, &SLUG_CONFLICT))?;
    sync_tags(conn, p.id, &p.effective_tags).await?;
    sync_content_refs(conn, p.id, p.content).await
}

// ---- creazione e traduzioni ------------------------------------------------------------------

pub async fn create_post(ctx: RouteCtx, body: PostCreate) -> Result<Value, ApiError> {
    let actor = actor(&ctx)?;
    let st = &ctx.state;
    let blog = scoped_blog(&ctx)?;
    let default_locale: String =
        sqlx::query_scalar("SELECT default_locale FROM blogs WHERE id = $1").bind(blog.id).fetch_one(&st.db).await?;
    let locale = body.locale.clone().unwrap_or(default_locale);
    noct_blogs::rules::validate_locale(&locale).map_err(ApiError::bad_request)?;
    rules::validate_post_slug(&body.slug).map_err(ApiError::bad_request)?;
    let (manual_tags, effective_tags) =
        rules::resolve_tags(body.tags.as_deref().unwrap_or_default(), &body.content).map_err(ApiError::bad_request)?;
    let cover_categories = rules::validate_categories(body.cover_image_categories.as_deref().unwrap_or_default())
        .map_err(ApiError::bad_request)?;

    let id = Uuid::new_v4();
    let mut tx = st.db.begin().await?;
    insert_post(
        &mut tx,
        blog,
        actor.user_id,
        &NewPost {
            id,
            group: id,
            locale: &locale,
            slug: &body.slug,
            title: &body.title,
            content: &body.content,
            cover_url: body.cover_image_url.as_deref().filter(|u| !u.is_empty()),
            cover_sensitive: body.cover_image_is_sensitive.unwrap_or(false) || !cover_categories.is_empty(),
            cover_categories,
            cover_alt: body.cover_image_alt_text.as_deref().unwrap_or_default(),
            manual_tags,
            effective_tags,
        },
    )
    .await?;
    st.extensions
        .apply(
            &mut tx,
            Entity::Post,
            &ExtCtx { state: st, op: Op::Create, entity_id: id, source_id: None, blog, actor: &actor },
            &body_map(&ctx.body),
        )
        .await?;
    let row = fetch_row(&mut tx, id, false).await?.ok_or_else(not_found)?;
    emit_saved(&mut tx, &ctx, blog, &actor, &row).await?;
    tx.commit().await?;
    if row.is_publicly_visible() {
        revalidate_post(st, &row);
    }
    render_one(st, row).await
}

pub async fn add_translation(ctx: RouteCtx, body: PostTranslation) -> Result<Value, ApiError> {
    let actor = actor(&ctx)?;
    let st = &ctx.state;
    let original = row_or_404(st, path_uuid(&ctx, "post_id")?).await?;
    let blog = blog_of(st, &original).await?;
    if !can_write_blog(st, Some(actor.user_id), &blog).await? {
        return Err(ApiError::forbidden(MSG_WRITE));
    }
    noct_blogs::rules::validate_locale(&body.locale).map_err(ApiError::bad_request)?;
    rules::validate_post_slug(&body.slug).map_err(ApiError::bad_request)?;
    let (manual_tags, effective_tags) =
        rules::resolve_tags(body.tags.as_deref().unwrap_or_default(), &body.content).map_err(ApiError::bad_request)?;
    let cover_categories = rules::validate_categories(body.cover_image_categories.as_deref().unwrap_or_default())
        .map_err(ApiError::bad_request)?;

    let id = Uuid::new_v4();
    let mut tx = st.db.begin().await?;
    insert_post(
        &mut tx,
        &blog,
        actor.user_id,
        &NewPost {
            id,
            group: original.translation_group_id,
            locale: &body.locale,
            slug: &body.slug,
            title: &body.title,
            content: &body.content,
            cover_url: body.cover_image_url.as_deref().filter(|u| !u.is_empty()),
            cover_sensitive: body.cover_image_is_sensitive.unwrap_or(false) || !cover_categories.is_empty(),
            cover_categories,
            cover_alt: body.cover_image_alt_text.as_deref().unwrap_or_default(),
            manual_tags,
            effective_tags,
        },
    )
    .await?;
    st.extensions
        .apply(
            &mut tx,
            Entity::Post,
            &ExtCtx {
                state: st,
                op: Op::Translate,
                entity_id: id,
                source_id: Some(original.id),
                blog: &blog,
                actor: &actor,
            },
            &body_map(&ctx.body),
        )
        .await?;
    let row = fetch_row(&mut tx, id, false).await?.ok_or_else(not_found)?;
    emit_saved(&mut tx, &ctx, &blog, &actor, &row).await?;
    tx.commit().await?;
    if row.is_publicly_visible() {
        revalidate_post(st, &row);
    }
    render_one(st, row).await
}

pub async fn list_translations(ctx: RouteCtx) -> Result<Vec<TranslationSummary>, ApiError> {
    let st = &ctx.state;
    let original = row_or_404(st, path_uuid(&ctx, "post_id")?).await?;
    let blog = blog_of(st, &original).await?;
    if !can_view_blog(st, ctx.actor.as_ref().map(|a| a.user_id), &blog).await? {
        return Err(not_found());
    }
    Ok(sqlx::query_as(&format!(
        "SELECT p.id, p.locale, p.slug, p.status::text AS status FROM posts p
         WHERE p.translation_group_id = $1 AND {VISIBLE_SQL} ORDER BY p.locale"
    ))
    .bind(original.translation_group_id)
    .fetch_all(&st.db)
    .await?)
}

// ---- letture -----------------------------------------------------------------------------------------

fn limit_offset(ctx: &RouteCtx) -> Result<(i64, i64), ApiError> {
    let num = |k: &str, default: i64| -> Result<i64, ApiError> {
        match ctx.query.get(k) {
            None => Ok(default),
            Some(v) => v.parse().map_err(|_| ApiError::unprocessable(format!("Parametro `{k}` non valido."))),
        }
    };
    Ok((num("limit", 20)?.clamp(1, 50), num("offset", 0)?.max(0)))
}

pub async fn list_posts(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let st = &ctx.state;
    let blog = scoped_blog(&ctx)?;
    // chi può scrivere vede anche bozze, revisioni e pianificati; gli altri solo i pubblicati
    let writer = can_write_blog(st, ctx.actor.as_ref().map(|a| a.user_id), blog).await?;
    let mut qb = QueryBuilder::<Postgres>::new(format!("{} WHERE p.blog_id = ", PostRow::SELECT));
    qb.push_bind(blog.id);
    if let Some(locale) = ctx.query.get("locale") {
        qb.push(" AND p.locale = ").push_bind(locale);
    }
    if !writer {
        qb.push(format!(" AND {VISIBLE_SQL}"));
    }
    qb.push(" ORDER BY p.created_at, p.id");
    let rows: Vec<PostRow> = qb.build_query_as().fetch_all(&st.db).await?;
    render(st, rows).await
}

pub async fn search_posts(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let st = &ctx.state;
    let blog = scoped_blog(&ctx)?;
    let q = ctx.query.get("q").map(|q| q.trim()).unwrap_or_default();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let (limit, offset) = limit_offset(&ctx)?;
    // `%`, `_` e `\` nel testo cercato sono letterali
    let needle = format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
    let rows: Vec<PostRow> = sqlx::query_as(&format!(
        "{} WHERE p.blog_id = $1 AND {VISIBLE_SQL} AND (p.title ILIKE $2 OR p.content ILIKE $2)
         ORDER BY p.published_at DESC NULLS LAST, p.id LIMIT $3 OFFSET $4",
        PostRow::SELECT
    ))
    .bind(blog.id)
    .bind(needle)
    .bind(limit)
    .bind(offset)
    .fetch_all(&st.db)
    .await?;
    render(st, rows).await
}

/// Un post non visibile al pubblico si mostra solo a chi può scrivere (404, non 403).
async fn visible_to(st: &AppState, ctx: &RouteCtx, row: &PostRow, blog: &BlogRef) -> Result<(), ApiError> {
    if row.is_publicly_visible() {
        return Ok(());
    }
    let user = ctx.actor.as_ref().map(|a| a.user_id);
    if user.is_none() || !can_write_blog(st, user, blog).await? {
        return Err(not_found());
    }
    Ok(())
}

pub async fn get_post(ctx: RouteCtx) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let row = row_or_404(st, path_uuid(&ctx, "post_id")?).await?;
    let blog = blog_of(st, &row).await?;
    if !can_view_blog(st, ctx.actor.as_ref().map(|a| a.user_id), &blog).await? {
        return Err(not_found());
    }
    visible_to(st, &ctx, &row, &blog).await?;
    render_one(st, row).await
}

pub async fn get_post_by_permalink(ctx: RouteCtx) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let blog = scoped_blog(&ctx)?;
    let slug = ctx.path.get("post_slug").map(String::as_str).unwrap_or_default();
    let candidates: Vec<PostRow> =
        sqlx::query_as(&format!("{} WHERE p.blog_id = $1 AND p.slug = $2 ORDER BY p.locale", PostRow::SELECT))
            .bind(blog.id)
            .bind(slug)
            .fetch_all(&st.db)
            .await?;
    // lo slug è unico per (blog, lingua): se due traduzioni lo condividono vince la lingua principale
    let pos = candidates.iter().position(|p| p.locale == p.blog_default_locale).unwrap_or(0);
    let row = candidates.into_iter().nth(pos).ok_or_else(not_found)?;
    visible_to(st, &ctx, &row, blog).await?;
    render_one(st, row).await
}

// ---- modifica ---------------------------------------------------------------------------------------------------

/// Tri-stato dal corpo grezzo: assente → `None`; `null` → `Some(None)`; booleano → `Some(Some(b))`.
fn tristate_bool(raw: &noct_core::JsonMap, key: &str) -> Result<Option<Option<bool>>, ApiError> {
    match raw.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::Bool(b)) => Ok(Some(Some(*b))),
        Some(_) => Err(ApiError::unprocessable(format!("Il campo `{key}` deve essere un booleano o null."))),
    }
}

pub async fn update_post(ctx: RouteCtx, body: PostUpdate) -> Result<Value, ApiError> {
    let actor = actor(&ctx)?;
    let st = &ctx.state;
    let found = row_or_404(st, path_uuid(&ctx, "post_id")?).await?;
    let blog = blog_of(st, &found).await?;
    if !can_write_blog(st, Some(actor.user_id), &blog).await? {
        return Err(ApiError::forbidden(MSG_WRITE));
    }
    let raw = body_map(&ctx.body);
    let search = tristate_bool(&raw, "search_indexing_enabled")?;
    let ai = tristate_bool(&raw, "ai_crawling_enabled")?;
    let new_categories = match &body.cover_image_categories {
        Some(c) => Some(rules::validate_categories(c).map_err(ApiError::bad_request)?),
        None => None,
    };

    let mut tx = st.db.begin().await?;
    let row = fetch_row(&mut tx, found.id, true).await?.ok_or_else(not_found)?;
    let content = body.content.clone().unwrap_or_else(|| row.content.clone());
    let mut qb = QueryBuilder::<Postgres>::new("UPDATE posts SET updated_at = now()");
    if let Some(t) = &body.title {
        qb.push(", title = ").push_bind(t);
    }
    if body.content.is_some() {
        qb.push(", content = ").push_bind(&content);
    }
    if let Some(url) = &body.cover_image_url {
        if url.is_empty() {
            // rimossa: non ha senso restare "sensibile"
            qb.push(", cover_image_url = NULL, cover_image_categories = '{}', cover_image_is_sensitive = false");
        } else {
            let cats = new_categories.clone().unwrap_or_default();
            let sensitive = body.cover_image_is_sensitive.unwrap_or(false) || !cats.is_empty();
            qb.push(", cover_image_url = ").push_bind(url);
            qb.push(", cover_image_is_sensitive = ").push_bind(sensitive);
            qb.push(", cover_image_categories = ").push_bind(cats);
        }
    } else if let Some(cats) = new_categories {
        // la copertina non cambia: il modal di avviso aggiorna le categorie di quella esistente
        if !cats.is_empty() {
            qb.push(", cover_image_is_sensitive = true");
        }
        qb.push(", cover_image_categories = ").push_bind(cats);
    }
    if raw.contains_key("cover_image_alt_text") {
        qb.push(", cover_image_alt_text = ").push_bind(body.cover_image_alt_text.clone().unwrap_or_default());
    }
    // i tag si ricalcolano se cambia il contenuto (gli #hashtag) o se il campo dedicato è stato inviato
    let mut effective = None;
    if body.content.is_some() || body.tags.is_some() {
        let manual_in = body.tags.clone().unwrap_or_else(|| row.manual_tags.clone());
        let (manual, eff) = rules::resolve_tags(&manual_in, &content).map_err(ApiError::bad_request)?;
        qb.push(", manual_tags = ").push_bind(manual);
        effective = Some(eff);
    }
    if let Some(v) = search {
        qb.push(", search_indexing_enabled = ").push_bind(v);
    }
    if let Some(v) = ai {
        qb.push(", ai_crawling_enabled = ").push_bind(v);
    }
    // quando è l'autore a risalvare, il nome pubblico si riallinea alle regole correnti
    if actor.user_id == row.author_id {
        let name = author_name(&mut tx, actor.user_id, blog.id).await?;
        qb.push(", author_display_name = ").push_bind(name);
    }
    qb.push(" WHERE id = ").push_bind(row.id);
    qb.build().execute(&mut *tx).await?;
    if let Some(eff) = effective {
        sync_tags(&mut tx, row.id, &eff).await?;
    }
    if body.content.is_some() {
        sync_content_refs(&mut tx, row.id, &content).await?;
    }
    st.extensions
        .apply(
            &mut tx,
            Entity::Post,
            &ExtCtx { state: st, op: Op::Update, entity_id: row.id, source_id: None, blog: &blog, actor: &actor },
            &raw,
        )
        .await?;
    let updated = fetch_row(&mut tx, row.id, false).await?.ok_or_else(not_found)?;
    emit_saved(&mut tx, &ctx, &blog, &actor, &updated).await?;
    tx.commit().await?;
    if updated.is_publicly_visible() {
        revalidate_post(st, &updated);
    }
    render_one(st, updated).await
}

// ---- stati ------------------------------------------------------------------------------------------------------------

pub async fn submit_for_review(ctx: RouteCtx) -> Result<Value, ApiError> {
    let actor = actor(&ctx)?;
    let st = &ctx.state;
    let found = row_or_404(st, path_uuid(&ctx, "post_id")?).await?;
    let blog = blog_of(st, &found).await?;
    if !can_write_blog(st, Some(actor.user_id), &blog).await? {
        return Err(ApiError::forbidden(MSG_WRITE));
    }
    let done = sqlx::query(
        "UPDATE posts SET status = 'pending_review', updated_at = now() WHERE id = $1 AND status = 'draft'",
    )
    .bind(found.id)
    .execute(&st.db)
    .await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::bad_request("Solo una bozza può essere mandata in revisione."));
    }
    render_one(st, row_or_404(st, found.id).await?).await
}

pub async fn return_to_draft(ctx: RouteCtx) -> Result<Value, ApiError> {
    let actor = actor(&ctx)?;
    let st = &ctx.state;
    let found = row_or_404(st, path_uuid(&ctx, "post_id")?).await?;
    let blog = blog_of(st, &found).await?;
    if !can_review(st, actor.user_id, &blog).await? {
        return Err(ApiError::forbidden("Serve essere proprietario del blog o revisore."));
    }
    let done = sqlx::query(
        "UPDATE posts SET status = 'draft', updated_at = now() WHERE id = $1 AND status = 'pending_review'",
    )
    .bind(found.id)
    .execute(&st.db)
    .await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::bad_request("Il post non è in revisione."));
    }
    render_one(st, row_or_404(st, found.id).await?).await
}

pub async fn publish_post(ctx: RouteCtx, body: PublishRequest) -> Result<Value, ApiError> {
    let actor = actor(&ctx)?;
    let st = &ctx.state;
    let found = row_or_404(st, path_uuid(&ctx, "post_id")?).await?;
    let blog = blog_of(st, &found).await?;
    if !can_write_blog(st, Some(actor.user_id), &blog).await? {
        // un revisore può solo approvare un post in revisione
        let approval = found.status == "pending_review" && can_review(st, actor.user_id, &blog).await?;
        if !approval {
            return Err(ApiError::forbidden(
                "Serve essere proprietario/autore/co-autore, o revisore per approvare un post in revisione.",
            ));
        }
    }
    let scheduled: Option<DateTime<Utc>> = body.published_at;
    let mut tx = st.db.begin().await?;
    let row = fetch_row(&mut tx, found.id, true).await?.ok_or_else(not_found)?;
    if row.status == "published" && scheduled.is_none() {
        // già pubblicato e nessun nuovo orario: no-op idempotente
        tx.commit().await?;
        return render_one(st, row).await;
    }
    let published_at = scheduled.unwrap_or_else(Utc::now);
    sqlx::query("UPDATE posts SET status = 'published', published_at = $2, updated_at = now() WHERE id = $1")
        .bind(row.id)
        .bind(published_at)
        .execute(&mut *tx)
        .await?;
    let updated = fetch_row(&mut tx, row.id, false).await?.ok_or_else(not_found)?;
    // la newsletter (se attiva) crea una sola campagna per post anche se l'evento si ripete
    emit(
        &mut tx,
        &ctx,
        &blog,
        &actor,
        "posts.post.published",
        json!({"post_id": updated.id, "blog_id": updated.blog_id, "title": updated.title, "slug": updated.slug,
               "locale": updated.locale, "published_at": updated.published_at}),
    )
    .await?;
    tx.commit().await?;
    if updated.is_publicly_visible() {
        revalidate_post(st, &updated);
    }
    render_one(st, updated).await
}

/// Autore del post (scrittura sul blog) richiesto da `unpublish_post` e `delete_post`.
async fn writable_post(ctx: &RouteCtx) -> Result<(Actor, PostRow, BlogRef), ApiError> {
    let actor = actor(ctx)?;
    let st = &ctx.state;
    let found = row_or_404(st, path_uuid(ctx, "post_id")?).await?;
    let blog = blog_of(st, &found).await?;
    if !can_write_blog(st, Some(actor.user_id), &blog).await? {
        return Err(ApiError::forbidden(MSG_WRITE));
    }
    Ok((actor, found, blog))
}

pub async fn unpublish_post(ctx: RouteCtx) -> Result<Value, ApiError> {
    let (actor, found, blog) = writable_post(&ctx).await?;
    let st = &ctx.state;
    let mut tx = st.db.begin().await?;
    let done = sqlx::query(
        "UPDATE posts SET status = 'draft', published_at = NULL, updated_at = now() WHERE id = $1 AND status = 'published'",
    )
    .bind(found.id)
    .execute(&mut *tx)
    .await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::bad_request("Il post non è pubblicato."));
    }
    emit(
        &mut tx,
        &ctx,
        &blog,
        &actor,
        "posts.post.unpublished",
        json!({"post_id": found.id, "blog_id": found.blog_id, "slug": found.slug, "locale": found.locale}),
    )
    .await?;
    tx.commit().await?;
    revalidate_post(st, &found);
    render_one(st, row_or_404(st, found.id).await?).await
}

pub async fn delete_post(ctx: RouteCtx) -> Result<(), ApiError> {
    let (actor, found, blog) = writable_post(&ctx).await?;
    let st = &ctx.state;
    let mut tx = st.db.begin().await?;
    // commenti, note, frammenti, tag, media e link citati cadono in cascata (FK `on_delete = cascade`)
    sqlx::query("DELETE FROM posts WHERE id = $1").bind(found.id).execute(&mut *tx).await?;
    emit(
        &mut tx,
        &ctx,
        &blog,
        &actor,
        "posts.post.deleted",
        json!({"post_id": found.id, "blog_id": found.blog_id, "slug": found.slug, "locale": found.locale}),
    )
    .await?;
    tx.commit().await?;
    revalidate_post(st, &found);
    Ok(())
}

// ---- bibliografie -------------------------------------------------------------------------------------------------------

pub async fn media_bibliography(ctx: RouteCtx) -> Result<Vec<MediaBibliographyEntry>, ApiError> {
    let blog = scoped_blog(&ctx)?;
    #[derive(sqlx::FromRow)]
    struct MediaRow {
        title: String,
        slug: String,
        locale: String,
        published_at: Option<DateTime<Utc>>,
        alt_text: String,
        categories: Vec<String>,
        is_sensitive: bool,
        url: String,
    }
    let rows: Vec<MediaRow> = sqlx::query_as(&format!(
        "SELECT p.title, p.slug, p.locale, p.published_at, m.alt_text, m.categories, m.is_sensitive, m.url
         FROM posts p JOIN post_media m ON m.post_id = p.id
         WHERE p.blog_id = $1 AND {VISIBLE_SQL}
         ORDER BY p.published_at DESC NULLS LAST, p.id, m.position"
    ))
    .bind(blog.id)
    .fetch_all(&ctx.state.db)
    .await?;
    let mut order: Vec<String> = Vec::new();
    let mut entries: HashMap<String, MediaBibliographyEntry> = HashMap::new();
    for r in rows {
        let cite = json!({
            "post_title": r.title, "post_slug": r.slug, "permalink": rules::permalink(&blog.slug, &r.slug),
            "locale": r.locale, "used_at": r.published_at,
        });
        let entry = entries.entry(r.url.clone()).or_insert_with(|| {
            order.push(r.url.clone());
            MediaBibliographyEntry {
                url: r.url.clone(),
                alt_text: r.alt_text.clone(),
                categories: r.categories.clone(),
                is_sensitive: r.is_sensitive,
                citations: json!([]),
            }
        });
        // una sola citazione sensibile rende sensibile l'intera voce
        entry.is_sensitive |= r.is_sensitive;
        if let Some(list) = entry.citations.as_array_mut() {
            list.push(cite);
        }
    }
    Ok(order.into_iter().filter_map(|u| entries.remove(&u)).collect())
}

pub async fn links_bibliography(ctx: RouteCtx) -> Result<Vec<LinkBibliographyEntry>, ApiError> {
    let blog = scoped_blog(&ctx)?;
    #[derive(sqlx::FromRow)]
    struct LinkRow {
        title: String,
        slug: String,
        locale: String,
        published_at: Option<DateTime<Utc>>,
        link_text: String,
        url: String,
    }
    let rows: Vec<LinkRow> = sqlx::query_as(&format!(
        "SELECT p.title, p.slug, p.locale, p.published_at, l.link_text, l.url
         FROM posts p JOIN post_links l ON l.post_id = p.id
         WHERE p.blog_id = $1 AND {VISIBLE_SQL}
         ORDER BY p.published_at DESC NULLS LAST, p.id, l.position"
    ))
    .bind(blog.id)
    .fetch_all(&ctx.state.db)
    .await?;
    let mut order: Vec<String> = Vec::new();
    let mut entries: HashMap<String, LinkBibliographyEntry> = HashMap::new();
    for r in rows {
        let cite = json!({
            "post_title": r.title, "post_slug": r.slug, "permalink": rules::permalink(&blog.slug, &r.slug),
            "locale": r.locale, "used_at": r.published_at,
        });
        let entry = entries.entry(r.url.clone()).or_insert_with(|| {
            order.push(r.url.clone());
            LinkBibliographyEntry { url: r.url.clone(), link_text: r.link_text.clone(), citations: json!([]) }
        });
        if let Some(list) = entry.citations.as_array_mut() {
            list.push(cite);
        }
    }
    Ok(order.into_iter().filter_map(|u| entries.remove(&u)).collect())
}

// ---- viste dell'autore su tutti i suoi blog ------------------------------------------------------------------------

/// Condizione SQL dei blog dell'utente `$1`: proprietario o collaboratore, non in cancellazione
/// (una vista aggregata non deve mostrare contenuti di un blog che il proprietario ha già eliminato).
pub const MY_BLOGS_SQL: &str = "b.deleted_at IS NULL AND (b.owner_id = $1 OR EXISTS (SELECT 1 FROM blog_memberships m WHERE m.blog_id = b.id AND m.user_id = $1))";

pub async fn my_posts(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let user = actor(&ctx)?.user_id;
    let rows: Vec<PostRow> =
        sqlx::query_as(&format!("{} WHERE {MY_BLOGS_SQL} ORDER BY p.created_at DESC, p.id", PostRow::SELECT))
            .bind(user)
            .fetch_all(&ctx.state.db)
            .await?;
    render(&ctx.state, rows).await
}

pub async fn my_links(ctx: RouteCtx) -> Result<Vec<MyLinkEntry>, ApiError> {
    let user = actor(&ctx)?.user_id;
    #[derive(sqlx::FromRow)]
    struct Row {
        title: String,
        slug: String,
        locale: String,
        published_at: Option<DateTime<Utc>>,
        link_text: String,
        url: String,
        blog_slug: String,
        blog_title: String,
    }
    let rows: Vec<Row> = sqlx::query_as(&format!(
        "SELECT p.title, p.slug, p.locale, p.published_at, l.link_text, l.url, b.slug AS blog_slug, b.title AS blog_title
         FROM posts p JOIN blogs b ON b.id = p.blog_id JOIN post_links l ON l.post_id = p.id
         WHERE {MY_BLOGS_SQL} ORDER BY p.created_at DESC, p.id, l.position"
    ))
    .bind(user)
    .fetch_all(&ctx.state.db)
    .await?;
    let mut order: Vec<String> = Vec::new();
    let mut entries: HashMap<String, MyLinkEntry> = HashMap::new();
    for r in rows {
        let cite = json!({
            "post_title": r.title, "post_slug": r.slug, "permalink": rules::permalink(&r.blog_slug, &r.slug),
            "locale": r.locale, "used_at": r.published_at, "blog_slug": r.blog_slug, "blog_title": r.blog_title,
        });
        let entry = entries.entry(r.url.clone()).or_insert_with(|| {
            order.push(r.url.clone());
            MyLinkEntry { url: r.url.clone(), link_text: r.link_text.clone(), citations: json!([]) }
        });
        if let Some(list) = entry.citations.as_array_mut() {
            list.push(cite);
        }
    }
    Ok(order.into_iter().filter_map(|u| entries.remove(&u)).collect())
}
