//! Handler `custom` del modulo: libreria note del blog, import/export BibTeX, bibliografie.

use crate::rules::{self, BibNote};
use crate::{BlogNoteCreate, BlogNoteUpdate, NoteImport, NoteMerge};
use axum::{
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use noct_core::{ApiError, BlogRef, RouteCtx, membership_role, parse_uuid, revalidate};
use noct_posts::VISIBLE_SQL;
use serde_json::{Value, json};
use std::collections::HashMap;
use uuid::Uuid;

/// Le note della libreria alimentano la bibliografia pubblica del blog.
fn revalidate_blog(ctx: &RouteCtx) {
    if let Some(b) = &ctx.blog {
        ctx.state.services.revalidate.spawn(vec![revalidate::blog(&b.slug)]);
    }
}

fn blog_of(ctx: &RouteCtx) -> Result<&BlogRef, ApiError> {
    ctx.blog.as_ref().ok_or_else(|| ApiError::internal("scope `blog` assente"))
}

fn user_id(ctx: &RouteCtx) -> Result<Uuid, ApiError> {
    ctx.actor.as_ref().map(|a| a.user_id).ok_or_else(|| ApiError::unauthorized("Autenticazione richiesta."))
}

fn note_id(ctx: &RouteCtx) -> Result<Uuid, ApiError> {
    parse_uuid(ctx.path.get("note_id").map(String::as_str).unwrap_or_default())
}

/// Libreria: proprietario o collaboratore (qualunque ruolo).
async fn require_member(ctx: &RouteCtx, blog: &BlogRef) -> Result<(), ApiError> {
    let user = user_id(ctx)?;
    if blog.owner_id == user || membership_role(&ctx.state, user, blog.id).await?.is_some() {
        return Ok(());
    }
    Err(ApiError::forbidden("Serve essere proprietario o collaboratore del blog."))
}

#[derive(sqlx::FromRow, Clone)]
struct NoteRow {
    id: Uuid,
    content: String,
    normalized: String,
    kind: String,
    url: Option<String>,
    title: Option<String>,
    author: Option<String>,
    source: Option<String>,
    issued: Option<String>,
    isbn: Option<String>,
    doi: Option<String>,
    page: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

const NOTE_COLS: &str =
    "id, content, normalized, kind, url, title, author, source, issued, isbn, doi, page, created_at, updated_at";

async fn get_note(ctx: &RouteCtx, blog: &BlogRef, id: Uuid) -> Result<NoteRow, ApiError> {
    sqlx::query_as(&format!("SELECT {NOTE_COLS} FROM blog_notes WHERE id = $1 AND blog_id = $2"))
        .bind(id)
        .bind(blog.id)
        .fetch_optional(&ctx.state.db)
        .await?
        .ok_or_else(|| ApiError::not_found("Nota non trovata."))
}

/// Risposta per `notes`: i post che le citano (una query) e i possibili duplicati calcolati su
/// `dup_pool` (default: le stesse note).
async fn notes_out(ctx: &RouteCtx, notes: &[NoteRow], dup_pool: Option<&[NoteRow]>) -> Result<Vec<Value>, ApiError> {
    let ids: Vec<Uuid> = notes.iter().map(|n| n.id).collect();
    let usages: Vec<(Uuid, i32, Uuid, String, String)> = sqlx::query_as(
        "SELECT pn.note_id, pn.idx, p.id, p.slug, p.title FROM post_notes pn JOIN posts p ON p.id = pn.post_id
         WHERE pn.note_id = ANY($1) ORDER BY p.created_at DESC, pn.idx",
    )
    .bind(&ids)
    .fetch_all(&ctx.state.db)
    .await?;
    let mut used: HashMap<Uuid, Vec<Value>> = HashMap::new();
    for (nid, idx, post_id, slug, title) in usages {
        used.entry(nid)
            .or_default()
            .push(json!({"post_id": post_id, "post_slug": slug, "post_title": title, "idx": idx}));
    }
    let pool: Vec<(Uuid, String)> = dup_pool.unwrap_or(notes).iter().map(|n| (n.id, n.normalized.clone())).collect();
    let dups = rules::duplicate_groups(&pool);
    Ok(notes
        .iter()
        .map(|n| {
            json!({
                "id": n.id, "content": n.content, "kind": n.kind, "url": n.url, "title": n.title, "author": n.author,
                "source": n.source, "issued": n.issued, "isbn": n.isbn, "doi": n.doi, "page": n.page,
                "created_at": n.created_at, "updated_at": n.updated_at,
                "used_in": used.remove(&n.id).unwrap_or_default(),
                "possible_duplicates": dups.get(&n.id).cloned().unwrap_or_default(),
            })
        })
        .collect())
}

fn check_kind(kind: &str) -> Result<(), ApiError> {
    if rules::NOTE_KINDS.contains(&kind) {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!("Tipo non valido: usare uno tra {}.", rules::NOTE_KINDS.join(", "))))
    }
}

fn bad<T>(r: Result<T, String>) -> Result<T, ApiError> {
    r.map_err(ApiError::bad_request)
}

pub async fn list_blog_notes(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let blog = blog_of(&ctx)?;
    require_member(&ctx, blog).await?;
    let all: Vec<NoteRow> =
        sqlx::query_as(&format!("SELECT {NOTE_COLS} FROM blog_notes WHERE blog_id = $1 ORDER BY updated_at DESC, id"))
            .bind(blog.id)
            .fetch_all(&ctx.state.db)
            .await?;
    let needle = ctx.query.get("q").map(|q| q.trim().to_lowercase()).filter(|q| !q.is_empty());
    let shown: Vec<NoteRow> = match needle {
        Some(q) => all.iter().filter(|n| n.content.to_lowercase().contains(&q)).cloned().collect(),
        None => all.clone(),
    };
    notes_out(&ctx, &shown, Some(&all)).await
}

pub async fn create_blog_note(ctx: RouteCtx, body: BlogNoteCreate) -> Result<Value, ApiError> {
    let blog = blog_of(&ctx)?;
    let content = body.content.trim();
    if content.is_empty() {
        return Err(ApiError::bad_request("Il testo della nota è obbligatorio."));
    }
    let kind = body.kind.as_deref().unwrap_or("note");
    check_kind(kind)?;
    let (title, author, source, issued, isbn, doi, page, url) = (
        bad(rules::clean_optional(body.title.as_deref(), rules::MAX_TITLE, "titolo"))?,
        bad(rules::clean_optional(body.author.as_deref(), rules::MAX_AUTHOR, "autore"))?,
        bad(rules::clean_optional(body.source.as_deref(), rules::MAX_SOURCE, "editore/rivista/sito"))?,
        bad(rules::clean_optional(body.issued.as_deref(), rules::MAX_ISSUED, "anno/data"))?,
        bad(rules::clean_optional(body.isbn.as_deref(), rules::MAX_ISBN, "ISBN"))?,
        bad(rules::clean_optional(body.doi.as_deref(), rules::MAX_DOI, "DOI"))?,
        bad(rules::clean_optional(body.page.as_deref(), rules::MAX_PAGE, "pagina"))?,
        bad(rules::clean_url(body.url.as_deref()))?,
    );
    let content: String = content.chars().take(rules::MAX_NOTE_LENGTH).collect();
    let row: NoteRow = sqlx::query_as(&format!(
        "INSERT INTO blog_notes (blog_id, content, normalized, kind, url, title, author, source, issued, isbn, doi, page, created_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) RETURNING {NOTE_COLS}"
    ))
    .bind(blog.id)
    .bind(&content)
    .bind(rules::normalize_note(&content))
    .bind(kind)
    .bind(url)
    .bind(title)
    .bind(author)
    .bind(source)
    .bind(issued)
    .bind(isbn)
    .bind(doi)
    .bind(page)
    .bind(user_id(&ctx)?)
    .fetch_one(&ctx.state.db)
    .await?;
    revalidate_blog(&ctx);
    Ok(notes_out(&ctx, &[row], None).await?.remove(0))
}

pub async fn update_blog_note(ctx: RouteCtx, body: BlogNoteUpdate) -> Result<Value, ApiError> {
    let blog = blog_of(&ctx)?;
    let current = get_note(&ctx, blog, note_id(&ctx)?).await?;
    if let Some(k) = &body.kind {
        check_kind(k)?;
    }
    let mut tx = ctx.state.db.begin().await?;
    let mut qb = sqlx::QueryBuilder::new("UPDATE blog_notes SET updated_at = now()");
    if let Some(k) = &body.kind {
        qb.push(", kind = ").push_bind(k.clone());
    }
    if body.url.is_some() {
        qb.push(", url = ").push_bind(bad(rules::clean_url(body.url.as_deref()))?);
    }
    let text_fields = [
        ("title", &body.title, rules::MAX_TITLE, "titolo"),
        ("author", &body.author, rules::MAX_AUTHOR, "autore"),
        ("source", &body.source, rules::MAX_SOURCE, "editore/rivista/sito"),
        ("issued", &body.issued, rules::MAX_ISSUED, "anno/data"),
        ("isbn", &body.isbn, rules::MAX_ISBN, "ISBN"),
        ("doi", &body.doi, rules::MAX_DOI, "DOI"),
        ("page", &body.page, rules::MAX_PAGE, "pagina"),
    ];
    for (column, value, max, label) in text_fields {
        if value.is_some() {
            qb.push(format!(", {column} = ")).push_bind(bad(rules::clean_optional(value.as_deref(), max, label))?);
        }
    }
    let mut new_content = None;
    if let Some(c) = &body.content {
        let c = c.trim();
        if c.is_empty() {
            return Err(ApiError::bad_request("Il testo della nota è obbligatorio."));
        }
        let c: String = c.chars().take(rules::MAX_NOTE_LENGTH).collect();
        qb.push(", content = ").push_bind(c.clone()).push(", normalized = ").push_bind(rules::normalize_note(&c));
        new_content = Some(c);
    }
    qb.push(" WHERE id = ").push_bind(current.id).push(format!(" RETURNING {NOTE_COLS}"));
    let row: NoteRow = qb.build_query_as().fetch_one(&mut *tx).await?;
    // bibliografia e note a piè di pagina dei post restano coerenti con la libreria
    if let Some(c) = new_content {
        sqlx::query("UPDATE post_notes SET content = $1 WHERE note_id = $2")
            .bind(c)
            .bind(row.id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    revalidate_blog(&ctx);
    Ok(notes_out(&ctx, &[row], None).await?.remove(0))
}

pub async fn delete_blog_note(ctx: RouteCtx) -> Result<(), ApiError> {
    let blog = blog_of(&ctx)?;
    let note = get_note(&ctx, blog, note_id(&ctx)?).await?;
    let mut tx = ctx.state.db.begin().await?;
    // lock sulla nota: un salvataggio concorrente non può agganciarla mentre la si elimina
    sqlx::query("SELECT id FROM blog_notes WHERE id = $1 FOR UPDATE").bind(note.id).execute(&mut *tx).await?;
    let used: i64 = sqlx::query_scalar("SELECT count(*) FROM post_notes WHERE note_id = $1")
        .bind(note.id)
        .fetch_one(&mut *tx)
        .await?;
    if used > 0 {
        return Err(ApiError::conflict("La nota è citata in un post: rimuovila prima dal post o uniscila a un'altra."));
    }
    sqlx::query("DELETE FROM blog_notes WHERE id = $1").bind(note.id).execute(&mut *tx).await?;
    tx.commit().await?;
    revalidate_blog(&ctx);
    Ok(())
}

pub async fn merge_blog_note(ctx: RouteCtx, body: NoteMerge) -> Result<Value, ApiError> {
    let blog = blog_of(&ctx)?;
    let source_id = note_id(&ctx)?;
    let into_id = parse_uuid(&body.into_id)?;
    if into_id == source_id {
        return Err(ApiError::bad_request("Scegli una nota diversa in cui unire."));
    }
    let source = get_note(&ctx, blog, source_id).await?;
    let target = get_note(&ctx, blog, into_id).await?;
    let mut tx = ctx.state.db.begin().await?;
    sqlx::query("UPDATE post_notes SET note_id = $1, content = $2 WHERE note_id = $3")
        .bind(target.id)
        .bind(&target.content)
        .bind(source.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM blog_notes WHERE id = $1").bind(source.id).execute(&mut *tx).await?;
    tx.commit().await?;
    revalidate_blog(&ctx);
    Ok(notes_out(&ctx, &[target], None).await?.remove(0))
}

pub async fn export_blog_notes(ctx: RouteCtx) -> Result<Response, ApiError> {
    let blog = blog_of(&ctx)?;
    require_member(&ctx, blog).await?;
    let rows: Vec<NoteRow> =
        sqlx::query_as(&format!("SELECT {NOTE_COLS} FROM blog_notes WHERE blog_id = $1 ORDER BY created_at, id"))
            .bind(blog.id)
            .fetch_all(&ctx.state.db)
            .await?;
    let notes: Vec<BibNote> = rows
        .into_iter()
        .map(|n| BibNote {
            content: n.content,
            kind: n.kind,
            url: n.url,
            title: n.title,
            author: n.author,
            source: n.source,
            issued: n.issued,
            isbn: n.isbn,
            doi: n.doi,
            page: n.page,
        })
        .collect();
    let disposition =
        HeaderValue::from_str(&format!("attachment; filename=\"{}.bib\"", blog.slug)).map_err(ApiError::internal)?;
    let mut res = (StatusCode::OK, rules::to_bibtex(&notes)).into_response();
    res.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/x-bibtex"));
    res.headers_mut().insert(header::CONTENT_DISPOSITION, disposition);
    Ok(res)
}

pub async fn import_blog_notes(ctx: RouteCtx, body: NoteImport) -> Result<Vec<Value>, ApiError> {
    let blog = blog_of(&ctx)?;
    let parsed = bad(rules::parse_bibtex(&body.bibtex))?;
    if parsed.is_empty() {
        return Err(ApiError::bad_request("Nessuna voce BibTeX riconosciuta."));
    }
    let user = user_id(&ctx)?;
    let mut tx = ctx.state.db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("blog_notes:{}", blog.id))
        .execute(&mut *tx)
        .await?;
    let mut existing: std::collections::HashSet<String> =
        sqlx::query_scalar("SELECT normalized FROM blog_notes WHERE blog_id = $1")
            .bind(blog.id)
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .collect();
    let mut created: Vec<NoteRow> = Vec::new();
    for item in parsed {
        let key = rules::normalize_note(&item.content);
        if key.is_empty() || !existing.insert(key.clone()) {
            continue;
        }
        let row: NoteRow = sqlx::query_as(&format!(
            "INSERT INTO blog_notes (blog_id, content, normalized, kind, url, title, author, source, issued, isbn, doi, page, created_by_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) RETURNING {NOTE_COLS}"
        ))
        .bind(blog.id)
        .bind(&item.content)
        .bind(key)
        .bind(&item.kind)
        .bind(rules::truncate_clean_url(item.url.as_deref()))
        .bind(rules::truncate_clean(item.title.as_deref(), rules::MAX_TITLE))
        .bind(rules::truncate_clean(item.author.as_deref(), rules::MAX_AUTHOR))
        .bind(rules::truncate_clean(item.source.as_deref(), rules::MAX_SOURCE))
        .bind(rules::truncate_clean(item.issued.as_deref(), rules::MAX_ISSUED))
        .bind(rules::truncate_clean(item.isbn.as_deref(), rules::MAX_ISBN))
        .bind(rules::truncate_clean(item.doi.as_deref(), rules::MAX_DOI))
        .bind(rules::truncate_clean(item.page.as_deref(), rules::MAX_PAGE))
        .bind(user)
        .fetch_one(&mut *tx)
        .await?;
        created.push(row);
    }
    tx.commit().await?;
    revalidate_blog(&ctx);
    notes_out(&ctx, &created, None).await
}

// ---- bibliografie -------------------------------------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct CitationRow {
    post_title: String,
    post_slug: String,
    locale: String,
    blog_slug: String,
    blog_title: String,
    idx: i32,
    content: String,
    title: Option<String>,
    author: Option<String>,
    isbn: Option<String>,
    doi: Option<String>,
    page: Option<String>,
    source: Option<String>,
    issued: Option<String>,
    kind: Option<String>,
    url: Option<String>,
}

/// L'URL finisce in un `href`: righe create prima della validazione possono avere schemi pericolosi.
fn safe_url(url: Option<String>) -> Option<String> {
    url.filter(|u| u.starts_with("http://") || u.starts_with("https://"))
}

/// Raggruppa le citazioni per testo identico, nell'ordine di arrivo.
fn group(rows: Vec<CitationRow>, with_blog: bool) -> Vec<Value> {
    let mut order: Vec<String> = Vec::new();
    let mut entries: HashMap<String, Value> = HashMap::new();
    for r in rows {
        let key = rules::group_key(&r.content);
        let mut cite = json!({
            "post_title": r.post_title, "post_slug": r.post_slug,
            "permalink": noct_posts::rules::permalink(&r.blog_slug, &r.post_slug),
            "locale": r.locale, "idx": r.idx,
        });
        if with_blog {
            cite["blog_slug"] = json!(r.blog_slug);
            cite["blog_title"] = json!(r.blog_title);
        }
        let entry = entries.entry(key.clone()).or_insert_with(|| {
            order.push(key.clone());
            json!({
                "content": r.content, "kind": r.kind, "url": safe_url(r.url.clone()), "title": r.title,
                "author": r.author, "isbn": r.isbn, "doi": r.doi, "page": r.page, "source": r.source,
                "issued": r.issued, "citations": [],
            })
        });
        if let Some(list) = entry["citations"].as_array_mut() {
            list.push(cite);
        }
    }
    order.into_iter().filter_map(|k| entries.remove(&k)).collect()
}

const CITATION_SELECT: &str =
    "SELECT p.title AS post_title, p.slug AS post_slug, p.locale, b.slug AS blog_slug, b.title AS blog_title,
        pn.idx, pn.content, pn.title, pn.author, pn.isbn, pn.doi, pn.page, pn.source, pn.issued, bn.kind, bn.url
     FROM posts p JOIN blogs b ON b.id = p.blog_id JOIN post_notes pn ON pn.post_id = p.id
     LEFT JOIN blog_notes bn ON bn.id = pn.note_id";

pub async fn get_blog_bibliography(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let blog = blog_of(&ctx)?;
    let rows: Vec<CitationRow> = sqlx::query_as(&format!(
        "{CITATION_SELECT} WHERE p.blog_id = $1 AND {VISIBLE_SQL} ORDER BY p.published_at DESC NULLS LAST, p.id, pn.idx"
    ))
    .bind(blog.id)
    .fetch_all(&ctx.state.db)
    .await?;
    Ok(group(rows, false))
}

/// Vista autore (qualunque stato dei post) sui blog di cui l'utente è proprietario o collaboratore,
/// esclusi quelli in cancellazione.
pub async fn get_my_bibliography(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let user = user_id(&ctx)?;
    let rows: Vec<CitationRow> = sqlx::query_as(&format!(
        "{CITATION_SELECT} WHERE b.deleted_at IS NULL
           AND (b.owner_id = $1 OR EXISTS (SELECT 1 FROM blog_memberships m WHERE m.blog_id = b.id AND m.user_id = $1))
         ORDER BY p.created_at DESC, p.id, pn.idx"
    ))
    .bind(user)
    .fetch_all(&ctx.state.db)
    .await?;
    Ok(group(rows, true))
}
