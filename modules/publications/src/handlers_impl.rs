//! Handler `custom` del modulo: CRUD delle pubblicazioni, indice dei capitoli e ordine.

use crate::{PublicationCreate, PublicationOrder, PublicationUpdate};
use chrono::{DateTime, Utc};
use noct_core::{ApiError, BlogRef, RouteCtx, can_write_blog, map_db_error, parse_uuid, revalidate};
use noct_posts::{VISIBLE_SQL, rules};
use serde_json::{Value, json};
use sqlx::{PgPool, QueryBuilder};
use uuid::Uuid;

const CONFLICT: (&str, u16, &str) = ("uq_publication_blog_name", 409, "Esiste già una pubblicazione con questo nome.");

fn blog_of(ctx: &RouteCtx) -> Result<&BlogRef, ApiError> {
    ctx.blog.as_ref().ok_or_else(|| ApiError::internal("scope `blog` assente"))
}

async fn is_writer(ctx: &RouteCtx, blog: &BlogRef) -> Result<bool, ApiError> {
    can_write_blog(&ctx.state, ctx.actor.as_ref().map(|a| a.user_id), blog).await
}

fn ref_param(ctx: &RouteCtx) -> Result<&str, ApiError> {
    ctx.path.get("ref").map(String::as_str).ok_or_else(|| ApiError::internal("parametro `ref` assente"))
}

#[derive(sqlx::FromRow)]
struct PubRow {
    id: Uuid,
    name: String,
    title: String,
    description: Option<String>,
    created_at: DateTime<Utc>,
}

/// `ref` è l'id oppure il nome (segmento URL).
async fn find(db: &PgPool, blog: &BlogRef, reference: &str) -> Result<PubRow, ApiError> {
    let by_id = reference.parse::<Uuid>().ok();
    sqlx::query_as(
        "SELECT id, name, title, description, created_at FROM publications
         WHERE blog_id = $1 AND (id = $2 OR name = $3)",
    )
    .bind(blog.id)
    .bind(by_id)
    .bind(reference)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| ApiError::not_found("Pubblicazione non trovata."))
}

/// (totali, pubblicati) per ogni pubblicazione.
async fn counts(db: &PgPool, ids: &[Uuid]) -> Result<std::collections::HashMap<Uuid, (i64, i64)>, ApiError> {
    let rows: Vec<(Uuid, i64, i64)> = sqlx::query_as(&format!(
        "SELECT pp.publication_id, count(*), count(*) FILTER (WHERE {VISIBLE_SQL})
         FROM post_publications pp JOIN posts p ON p.id = pp.post_id
         WHERE pp.publication_id = ANY($1) GROUP BY pp.publication_id"
    ))
    .bind(ids)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|(id, t, p)| (id, (t, p))).collect())
}

fn out(p: &PubRow, counts: &std::collections::HashMap<Uuid, (i64, i64)>) -> Value {
    let (total, published) = counts.get(&p.id).copied().unwrap_or((0, 0));
    json!({
        "id": p.id, "name": p.name, "title": p.title, "description": p.description,
        "chapters_total": total, "chapters_published": published, "created_at": p.created_at,
    })
}

fn invalidate(ctx: &RouteCtx, blog: &BlogRef) {
    ctx.state.services.revalidate.spawn(vec![revalidate::blog(&blog.slug)]);
}

pub async fn list_publications(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let blog = blog_of(&ctx)?;
    let rows: Vec<PubRow> = sqlx::query_as(
        "SELECT id, name, title, description, created_at FROM publications WHERE blog_id = $1 ORDER BY created_at, id",
    )
    .bind(blog.id)
    .fetch_all(&ctx.state.db)
    .await?;
    let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let counts = counts(&ctx.state.db, &ids).await?;
    let writer = is_writer(&ctx, blog).await?;
    Ok(rows.iter().filter(|r| writer || counts.get(&r.id).is_some_and(|c| c.1 > 0)).map(|r| out(r, &counts)).collect())
}

pub async fn create_publication(ctx: RouteCtx, body: PublicationCreate) -> Result<Value, ApiError> {
    let blog = blog_of(&ctx)?;
    let description = body.description.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let row: PubRow = sqlx::query_as(
        "INSERT INTO publications (blog_id, name, title, description) VALUES ($1, $2, $3, $4)
         RETURNING id, name, title, description, created_at",
    )
    .bind(blog.id)
    .bind(&body.name)
    .bind(body.title.trim())
    .bind(description)
    .fetch_one(&ctx.state.db)
    .await
    .map_err(|e| map_db_error(e, &[CONFLICT]))?;
    Ok(out(&row, &Default::default()))
}

#[derive(sqlx::FromRow)]
struct ChapterRow {
    id: Uuid,
    slug: String,
    title: String,
    locale: String,
    status: String,
    published_at: Option<DateTime<Utc>>,
    words: i32,
    is_public: bool,
}

async fn detail(ctx: &RouteCtx, blog: &BlogRef, pubrow: &PubRow, writer: bool) -> Result<Value, ApiError> {
    // ordine esplicito prima, poi cronologico dal più vecchio
    let mut chapters: Vec<ChapterRow> = sqlx::query_as(&format!(
        "SELECT p.id, p.slug, p.title, p.locale, p.status::text AS status, p.published_at,
                coalesce(cardinality(regexp_split_to_array(btrim(p.content), '\\s+')), 0) AS words,
                ({VISIBLE_SQL}) AS is_public
         FROM post_publications pp JOIN posts p ON p.id = pp.post_id
         WHERE pp.publication_id = $1
         ORDER BY pp.chapter_order NULLS LAST, coalesce(p.published_at, p.created_at), p.created_at, p.id"
    ))
    .bind(pubrow.id)
    .fetch_all(&ctx.state.db)
    .await?;
    if !writer {
        chapters.retain(|c| c.is_public);
        if chapters.is_empty() {
            return Err(ApiError::not_found("Pubblicazione non trovata."));
        }
    }
    let counts = counts(&ctx.state.db, &[pubrow.id]).await?;
    let mut value = out(pubrow, &counts);
    let list: Vec<Value> = chapters
        .iter()
        .enumerate()
        .map(|(i, c)| {
            json!({
                "n": i + 1, "post_id": c.id, "slug": c.slug, "title": c.title, "locale": c.locale,
                "status": c.status, "published_at": c.published_at,
                "permalink": rules::permalink(&blog.slug, &c.slug),
                "reading_minutes": ((f64::from(c.words) / 200.0).round() as i64).max(1),
                "is_public": c.is_public,
            })
        })
        .collect();
    value["chapters"] = Value::Array(list);
    Ok(value)
}

pub async fn get_publication(ctx: RouteCtx) -> Result<Value, ApiError> {
    let blog = blog_of(&ctx)?;
    let row = find(&ctx.state.db, blog, ref_param(&ctx)?).await?;
    let writer = is_writer(&ctx, blog).await?;
    detail(&ctx, blog, &row, writer).await
}

pub async fn update_publication(ctx: RouteCtx, body: PublicationUpdate) -> Result<Value, ApiError> {
    let blog = blog_of(&ctx)?;
    let current = find(&ctx.state.db, blog, ref_param(&ctx)?).await?;
    let mut qb = QueryBuilder::new("UPDATE publications SET updated_at = now()");
    if let Some(name) = &body.name {
        qb.push(", name = ").push_bind(name.clone());
    }
    if let Some(title) = &body.title {
        qb.push(", title = ").push_bind(title.trim().to_string());
    }
    if let Some(d) = &body.description {
        qb.push(", description = ").push_bind(Some(d.trim().to_string()).filter(|s| !s.is_empty()));
    }
    qb.push(" WHERE id = ").push_bind(current.id);
    qb.push(" RETURNING id, name, title, description, created_at");
    let row: PubRow = qb.build_query_as().fetch_one(&ctx.state.db).await.map_err(|e| map_db_error(e, &[CONFLICT]))?;
    invalidate(&ctx, blog);
    let counts = counts(&ctx.state.db, &[row.id]).await?;
    Ok(out(&row, &counts))
}

pub async fn order_publication(ctx: RouteCtx, body: PublicationOrder) -> Result<Value, ApiError> {
    let blog = blog_of(&ctx)?;
    let row = find(&ctx.state.db, blog, ref_param(&ctx)?).await?;
    let mut wanted: Vec<Uuid> = Vec::new();
    for raw in &body.post_ids {
        let id = parse_uuid(raw)?;
        if !wanted.contains(&id) {
            wanted.push(id);
        }
    }
    let mut tx = ctx.state.db.begin().await?;
    // blocca la pubblicazione: due riordini concorrenti non si mescolano
    sqlx::query("SELECT id FROM publications WHERE id = $1 FOR UPDATE").bind(row.id).execute(&mut *tx).await?;
    let members: Vec<Uuid> = sqlx::query_scalar("SELECT post_id FROM post_publications WHERE publication_id = $1")
        .bind(row.id)
        .fetch_all(&mut *tx)
        .await?;
    if wanted.iter().any(|id| !members.contains(id)) {
        return Err(ApiError::bad_request("Alcuni post non appartengono a questa pubblicazione."));
    }
    sqlx::query("UPDATE post_publications SET chapter_order = NULL WHERE publication_id = $1")
        .bind(row.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE post_publications pp SET chapter_order = w.n::int
         FROM unnest($2::uuid[]) WITH ORDINALITY AS w(id, n)
         WHERE pp.publication_id = $1 AND pp.post_id = w.id",
    )
    .bind(row.id)
    .bind(&wanted)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    invalidate(&ctx, blog);
    detail(&ctx, blog, &row, true).await
}

pub async fn delete_publication(ctx: RouteCtx) -> Result<(), ApiError> {
    let blog = blog_of(&ctx)?;
    let row = find(&ctx.state.db, blog, ref_param(&ctx)?).await?;
    sqlx::query("DELETE FROM publications WHERE id = $1").bind(row.id).execute(&ctx.state.db).await?;
    invalidate(&ctx, blog);
    Ok(())
}

/// Pubblicazioni di tutti i blog dell'utente (proprietario o collaboratore, esclusi quelli in
/// cancellazione), più recenti prima.
pub async fn my_publications(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let user =
        ctx.actor.as_ref().map(|a| a.user_id).ok_or_else(|| ApiError::unauthorized("Autenticazione richiesta."))?;
    #[derive(sqlx::FromRow)]
    struct Row {
        #[sqlx(flatten)]
        p: PubRow,
        blog_slug: String,
        blog_title: String,
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT u.id, u.name, u.title, u.description, u.created_at, b.slug AS blog_slug, b.title AS blog_title
         FROM publications u JOIN blogs b ON b.id = u.blog_id
         WHERE b.deleted_at IS NULL AND (b.owner_id = $1 OR EXISTS
               (SELECT 1 FROM blog_memberships m WHERE m.blog_id = b.id AND m.user_id = $1))
         ORDER BY u.created_at DESC, u.id",
    )
    .bind(user)
    .fetch_all(&ctx.state.db)
    .await?;
    let ids: Vec<Uuid> = rows.iter().map(|r| r.p.id).collect();
    let counts = counts(&ctx.state.db, &ids).await?;
    Ok(rows
        .iter()
        .map(|r| {
            let mut v = out(&r.p, &counts);
            v["blog_slug"] = json!(r.blog_slug);
            v["blog_title"] = json!(r.blog_title);
            v
        })
        .collect())
}
