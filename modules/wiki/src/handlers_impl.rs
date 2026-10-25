//! Handler `custom` del modulo: indice (albero) dello Spazio, cronologia delle revisioni con
//! differenze, backlink dei link interni.

use crate::{OutlineUpdate, extensions::parent_map, rules};
use chrono::{DateTime, Utc};
use noct_audit::{Entry, user_actor};
use noct_core::{
    Actor, ApiError, AppState, BlogRef, RouteCtx, can_view_blog, can_write_blog, load_blog, parse_uuid, revalidate,
};
use noct_posts::VISIBLE_SQL;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

const MSG_WRITE: &str = "Non hai i permessi per scrivere in questo spazio.";

fn actor(ctx: &RouteCtx) -> Result<Actor, ApiError> {
    ctx.actor.clone().ok_or_else(|| ApiError::unauthorized("Autenticazione richiesta."))
}

fn scoped_blog(ctx: &RouteCtx) -> Result<&BlogRef, ApiError> {
    ctx.blog.as_ref().ok_or_else(|| ApiError::internal("scope `blog` assente"))
}

fn post_id(ctx: &RouteCtx) -> Result<Uuid, ApiError> {
    parse_uuid(ctx.path.get("post_id").map(String::as_str).unwrap_or_default())
}

fn not_found() -> ApiError {
    ApiError::not_found("Post non trovato.")
}

/// Il post con il suo Spazio, solo se visibile a chi chiama (altrimenti 404).
struct Target {
    id: Uuid,
    slug: String,
    blog: BlogRef,
    publicly_visible: bool,
}

async fn target(ctx: &RouteCtx) -> Result<Target, ApiError> {
    let st = &ctx.state;
    let id = post_id(ctx)?;
    let row: Option<(String, String, bool)> = sqlx::query_as(&format!(
        "SELECT p.slug, b.slug, ({VISIBLE_SQL}) FROM posts p JOIN blogs b ON b.id = p.blog_id WHERE p.id = $1"
    ))
    .bind(id)
    .fetch_optional(&st.db)
    .await?;
    let (slug, blog_slug, publicly_visible) = row.ok_or_else(not_found)?;
    let blog = load_blog(st, &blog_slug).await?.ok_or_else(|| ApiError::not_found("Spazio non trovato."))?;
    if !can_view_blog(st, ctx.actor.as_ref().map(|a| a.user_id), &blog).await? {
        return Err(not_found());
    }
    Ok(Target { id, slug, blog, publicly_visible })
}

/// Come [`target`], ma solo per chi può scrivere nello Spazio.
async fn writable_target(ctx: &RouteCtx) -> Result<Target, ApiError> {
    let actor = actor(ctx)?;
    let t = target(ctx).await?;
    if !can_write_blog(&ctx.state, Some(actor.user_id), &t.blog).await? {
        return Err(ApiError::forbidden(MSG_WRITE));
    }
    Ok(t)
}

async fn is_space_kind(st: &AppState, blog: Uuid) -> Result<bool, ApiError> {
    let kind: String =
        sqlx::query_scalar("SELECT kind::text FROM blogs WHERE id = $1").bind(blog).fetch_one(&st.db).await?;
    Ok(kind != "blog")
}

// ---- indice ---------------------------------------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct Row {
    id: Uuid,
    slug: String,
    title: String,
    locale: String,
    status: String,
    parent_id: Option<Uuid>,
    position: i32,
    visible: bool,
}

/// L'indice dello Spazio in ordine di lettura (depth-first). Chi non scrive vede solo i post
/// pubblicati; una pagina il cui padre non è visibile sale alla radice.
async fn outline(st: &AppState, blog: Uuid, locale: Option<&str>, writer: bool) -> Result<Vec<Value>, ApiError> {
    let rows: Vec<Row> = sqlx::query_as(&format!(
        "SELECT p.id, p.slug, p.title, p.locale, p.status::text AS status, o.parent_id, COALESCE(o.position, 2147483647) AS position, ({VISIBLE_SQL}) AS visible
         FROM posts p LEFT JOIN post_outline o ON o.post_id = p.id
         WHERE p.blog_id = $1 AND ($2::text IS NULL OR p.locale = $2) AND ($3 OR ({VISIBLE_SQL}))
         ORDER BY COALESCE(o.position, 2147483647), p.created_at, p.id"
    ))
    .bind(blog)
    .bind(locale)
    .bind(writer)
    .fetch_all(&st.db)
    .await?;
    let rows: Vec<Row> =
        rows.into_iter().map(|r| Row { position: if r.position == i32::MAX { 0 } else { r.position }, ..r }).collect();
    let present: HashSet<Uuid> = rows.iter().map(|r| r.id).collect();
    let mut children: HashMap<Option<Uuid>, Vec<usize>> = HashMap::new();
    for (i, r) in rows.iter().enumerate() {
        children.entry(r.parent_id.filter(|p| present.contains(p))).or_default().push(i);
    }
    let mut out = Vec::with_capacity(rows.len());
    let mut seen: HashSet<usize> = HashSet::new();
    // visita iterativa: la pila contiene (indice, profondità) e i figli si impilano al contrario
    let mut stack: Vec<(usize, usize)> = children.get(&None).into_iter().flatten().rev().map(|i| (*i, 0)).collect();
    loop {
        while let Some((i, depth)) = stack.pop() {
            if !seen.insert(i) {
                continue;
            }
            let r = &rows[i];
            out.push(json!({
                "id": r.id, "slug": r.slug, "title": r.title, "locale": r.locale, "status": r.status,
                "parent_id": r.parent_id.filter(|p| present.contains(p)), "position": r.position, "depth": depth,
                "is_visible": r.visible,
            }));
            stack.extend(children.get(&Some(r.id)).into_iter().flatten().rev().map(|c| (*c, depth + 1)));
        }
        // quanto non raggiunto (non dovrebbe esistere dopo la validazione) torna comunque alla radice
        match (0..rows.len()).find(|i| !seen.contains(i)) {
            Some(i) => stack.push((i, 0)),
            None => break,
        }
    }
    Ok(out)
}

pub async fn get_outline(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let blog = scoped_blog(&ctx)?;
    let writer = can_write_blog(&ctx.state, ctx.actor.as_ref().map(|a| a.user_id), blog).await?;
    outline(&ctx.state, blog.id, ctx.query.get("locale").map(String::as_str).filter(|l| !l.is_empty()), writer).await
}

fn parse_items(items: &Value) -> Result<Vec<(Uuid, Option<Uuid>)>, ApiError> {
    let bad = || ApiError::bad_request("`items` deve essere un elenco di {id, parent_id}.");
    items
        .as_array()
        .ok_or_else(bad)?
        .iter()
        .map(|it| {
            let id = it.get("id").and_then(Value::as_str).and_then(|s| Uuid::parse_str(s).ok()).ok_or_else(bad)?;
            let parent = match it.get("parent_id") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) => Some(Uuid::parse_str(s).map_err(|_| bad())?),
                Some(_) => return Err(bad()),
            };
            Ok((id, parent))
        })
        .collect()
}

pub async fn put_outline(ctx: RouteCtx, body: OutlineUpdate) -> Result<Vec<Value>, ApiError> {
    let actor = actor(&ctx)?;
    let st = &ctx.state;
    let blog = scoped_blog(&ctx)?;
    if !is_space_kind(st, blog.id).await? {
        return Err(ApiError::bad_request("L'indice esiste solo negli spazi wiki e docs."));
    }
    let items = parse_items(&body.items)?;

    let mut tx = st.db.begin().await?;
    // serializza le riorganizzazioni dello stesso Spazio
    sqlx::query("SELECT id FROM blogs WHERE id = $1 FOR UPDATE").bind(blog.id).execute(&mut *tx).await?;
    let existing: HashSet<Uuid> = parent_map(&mut tx, blog.id).await?.into_keys().collect();
    let listed: HashSet<Uuid> = items.iter().map(|i| i.0).collect();
    if listed.len() != items.len() || listed != existing {
        return Err(ApiError::conflict("L'indice è cambiato nel frattempo: ricarica la pagina e riprova."));
    }
    let parents: HashMap<Uuid, Option<Uuid>> = items.iter().copied().collect();
    let ids: Vec<Uuid> = items.iter().map(|i| i.0).collect();
    rules::check_parents(&parents, &ids).map_err(ApiError::bad_request)?;

    let mut next: HashMap<Option<Uuid>, i32> = HashMap::new();
    let positions: Vec<i32> = items
        .iter()
        .map(|(_, parent)| {
            let n = next.entry(*parent).or_insert(0);
            *n += 1;
            *n - 1
        })
        .collect();
    let parent_ids: Vec<Option<Uuid>> = items.iter().map(|i| i.1).collect();
    sqlx::query(
        "INSERT INTO post_outline (post_id, parent_id, position) SELECT * FROM UNNEST($1::uuid[], $2::uuid[], $3::int[])
         ON CONFLICT (post_id) DO UPDATE SET parent_id = EXCLUDED.parent_id, position = EXCLUDED.position, updated_at = now()",
    )
    .bind(&ids)
    .bind(&parent_ids)
    .bind(&positions)
    .execute(&mut *tx)
    .await?;
    let who = user_actor(&mut tx, actor.user_id).await.map_err(ApiError::internal)?;
    noct_audit::record(
        &mut tx,
        Entry {
            action: "wiki.outline_updated",
            actor: who,
            target: Some(("blog", blog.id)),
            blog_id: Some(blog.id),
            meta: &ctx.meta,
            payload: json!({"blog_slug": blog.slug, "pages": ids.len()}),
        },
    )
    .await
    .map_err(ApiError::internal)?;
    tx.commit().await?;
    st.services.revalidate.spawn(vec![revalidate::blog(&blog.slug)]);
    outline(st, blog.id, ctx.query.get("locale").map(String::as_str).filter(|l| !l.is_empty()), true).await
}

// ---- revisioni ------------------------------------------------------------------------------------------------------

/// Nome con cui un editor compare nella cronologia: l'alias della sua membership, altrimenti lo username.
const EDITOR_SQL: &str = "COALESCE(m.author_display_name, u.username)";
const REVISION_JOINS: &str = "LEFT JOIN users u ON u.id = r.editor_id
     LEFT JOIN blog_memberships m ON m.user_id = r.editor_id AND m.blog_id = p.blog_id";

#[derive(sqlx::FromRow)]
struct RevisionRow {
    revision: i32,
    title: String,
    editor: Option<String>,
    summary: Option<String>,
    size: i32,
    created_at: DateTime<Utc>,
}

pub async fn list_revisions(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let t = writable_target(&ctx).await?;
    let rows: Vec<RevisionRow> = sqlx::query_as(&format!(
        "SELECT r.revision, r.title, {EDITOR_SQL} AS editor, r.summary, length(r.content) AS size, r.created_at
         FROM post_revisions r JOIN posts p ON p.id = r.post_id {REVISION_JOINS}
         WHERE r.post_id = $1 ORDER BY r.revision DESC"
    ))
    .bind(t.id)
    .fetch_all(&ctx.state.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            json!({"revision": r.revision, "title": r.title, "editor": r.editor, "summary": r.summary, "size": r.size, "created_at": r.created_at})
        })
        .collect())
}

fn revision_param(ctx: &RouteCtx) -> Result<i32, ApiError> {
    ctx.path
        .get("revision")
        .and_then(|r| r.parse::<i32>().ok())
        .filter(|r| *r >= 1)
        .ok_or_else(|| ApiError::bad_request("Numero di revisione non valido."))
}

async fn revision_content(st: &AppState, post: Uuid, revision: i32) -> Result<(String, String), ApiError> {
    sqlx::query_as("SELECT title, content FROM post_revisions WHERE post_id = $1 AND revision = $2")
        .bind(post)
        .bind(revision)
        .fetch_optional(&st.db)
        .await?
        .ok_or_else(|| ApiError::not_found("Revisione non trovata."))
}

#[derive(sqlx::FromRow)]
struct FullRevision {
    title: String,
    content: String,
    editor: Option<String>,
    summary: Option<String>,
    created_at: DateTime<Utc>,
}

pub async fn get_revision(ctx: RouteCtx) -> Result<Value, ApiError> {
    let t = writable_target(&ctx).await?;
    let revision = revision_param(&ctx)?;
    let row: Option<FullRevision> = sqlx::query_as(&format!(
        "SELECT r.title, r.content, {EDITOR_SQL} AS editor, r.summary, r.created_at
         FROM post_revisions r JOIN posts p ON p.id = r.post_id {REVISION_JOINS}
         WHERE r.post_id = $1 AND r.revision = $2"
    ))
    .bind(t.id)
    .bind(revision)
    .fetch_optional(&ctx.state.db)
    .await?;
    let r = row.ok_or_else(|| ApiError::not_found("Revisione non trovata."))?;
    Ok(json!({
        "revision": revision, "title": r.title, "content": r.content, "editor": r.editor, "summary": r.summary,
        "created_at": r.created_at,
    }))
}

pub async fn diff_revision(ctx: RouteCtx) -> Result<Value, ApiError> {
    let t = writable_target(&ctx).await?;
    let to = revision_param(&ctx)?;
    let from = match ctx.query.get("against") {
        Some(a) => {
            a.parse::<i32>().ok().filter(|r| *r >= 1).ok_or_else(|| ApiError::bad_request("`against` non valido."))?
        }
        None => to - 1,
    };
    let (_, new) = revision_content(&ctx.state, t.id, to).await?;
    // senza una revisione precedente il confronto è con il testo vuoto
    let old = if from >= 1 { revision_content(&ctx.state, t.id, from).await?.1 } else { String::new() };
    let hunks: Vec<Value> =
        rules::line_diff(&old, &new).into_iter().map(|h| json!({"op": h.op, "text": h.text})).collect();
    Ok(json!({"from": from.max(0), "to": to, "hunks": hunks}))
}

// ---- backlink -------------------------------------------------------------------------------------------------------

pub async fn list_backlinks(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let st = &ctx.state;
    let t = target(&ctx).await?;
    let writer = can_write_blog(st, ctx.actor.as_ref().map(|a| a.user_id), &t.blog).await?;
    if !t.publicly_visible && !writer {
        return Err(not_found());
    }
    let rows: Vec<(Uuid, String, String, String)> = sqlx::query_as(&format!(
        "SELECT DISTINCT p.id, p.slug, p.title, p.locale FROM post_wikilinks w JOIN posts p ON p.id = w.post_id
         WHERE p.blog_id = $1 AND w.target_slug = $2 AND p.id <> $3 AND ($4 OR ({VISIBLE_SQL}))
         ORDER BY p.title, p.id"
    ))
    .bind(t.blog.id)
    .bind(rules::slugify(&t.slug))
    .bind(t.id)
    .bind(writer)
    .fetch_all(&st.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, slug, title, locale)| json!({"id": id, "slug": slug, "title": title, "locale": locale}))
        .collect())
}
