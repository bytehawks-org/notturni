use crate::{
    CoverImageUpdate, MediaFileUpdate, MediaLibraryOut, MediaUploaded,
    upload::{self, FAVICON_TYPES, MAX_FAVICON_BYTES, MAX_MEDIA_BYTES, MEDIA_TYPES, Upload},
    variants::{self, PrepareError},
};
use axum::http::header::CONTENT_TYPE;
use chrono::{DateTime, Utc};
use noct_core::{ApiError, AppState, BlogRef, RouteCtx, body_map, membership_role, parse_uuid, revalidate};
use noct_posts::rules::validate_categories;
use serde_json::{Value, json};
use sqlx::{FromRow, Postgres, QueryBuilder};
use std::collections::HashMap;
use uuid::Uuid;

fn blog(ctx: &RouteCtx) -> Result<&BlogRef, ApiError> {
    ctx.blog.as_ref().ok_or_else(|| ApiError::internal("scope del blog assente"))
}

fn user(ctx: &RouteCtx) -> Result<Uuid, ApiError> {
    ctx.actor.as_ref().map(|a| a.user_id).ok_or_else(|| ApiError::unauthorized("Not authenticated"))
}

/// Solo il proprietario gestisce copertina e favicon.
fn owner_only(ctx: &RouteCtx) -> Result<&BlogRef, ApiError> {
    let b = blog(ctx)?;
    if b.owner_id != user(ctx)? {
        return Err(ApiError::forbidden("Solo il proprietario può farlo."));
    }
    Ok(b)
}

async fn read_upload(ctx: &RouteCtx) -> Result<Upload, ApiError> {
    let ct = ctx.headers.get(CONTENT_TYPE).and_then(|v| v.to_str().ok());
    upload::read_file(ct, ctx.body.clone()).await
}

/// Controlla lo spazio massimo per blog (`platform_config.max_blog_storage_mb`): somma dei
/// media già in libreria più il nuovo file. Nessun limite se non è impostato.
async fn assert_quota(st: &AppState, blog: Uuid, extra: usize) -> Result<(), ApiError> {
    let Some(limit_mb) = noct_platform::get(&st.db).await?.max_blog_storage_mb else {
        return Ok(());
    };
    let used: i64 =
        sqlx::query_scalar("SELECT COALESCE(sum(size_bytes), 0)::bigint FROM media_files WHERE blog_id = $1")
            .bind(blog)
            .fetch_one(&st.db)
            .await?;
    if used + extra as i64 > i64::from(limit_mb) * 1024 * 1024 {
        return Err(ApiError::new(413, format!("Spazio massimo per questo blog superato ({limit_mb} MB).")));
    }
    Ok(())
}

struct Stored {
    key: String,
    url: String,
    content_type: String,
    size: usize,
    is_sensitive: bool,
}

/// Valida, controlla la quota, salva sullo storage e classifica un'immagine di contenuto.
async fn store_content_image(ctx: &RouteCtx, up: &Upload) -> Result<Stored, ApiError> {
    let st = &ctx.state;
    let b = blog(ctx)?;
    let ext = upload::check_image(up, MEDIA_TYPES, MAX_MEDIA_BYTES, "PNG, JPEG, WEBP o GIF", "10 MiB")?;
    // varianti WebP responsive (le GIF restano com'è: potrebbero essere animate). Decodifica e
    // ridimensionamento pesano sulla CPU: fuori dal thread del runtime.
    let prepared = if ext == "gif" {
        None
    } else {
        let data = up.data.clone();
        match tokio::task::spawn_blocking(move || variants::prepare(&data)).await.map_err(ApiError::internal)? {
            Ok(p) => p,
            Err(PrepareError::TooLarge) => {
                return Err(ApiError::bad_request(format!(
                    "L'immagine supera i {} megapixel consentiti.",
                    variants::MAX_PIXELS / 1_000_000
                )));
            }
        }
    };
    let total = up.data.len() + prepared.as_ref().map_or(0, |p| p.variants.iter().map(|v| v.data.len()).sum());
    assert_quota(st, b.id, total).await?;
    let key =
        upload::media_key(&st.config.site_slug, b.owner_id, b.id, ext, prepared.as_ref().map(|p| (p.width, p.height)));
    let bucket = &st.config.storage.bucket_content;
    st.services.storage.put(bucket, &key, up.data.clone(), &up.content_type).await.map_err(ApiError::internal)?;
    if let Some(p) = prepared {
        for v in p.variants {
            if let Some(vkey) = variants::variant_key(&key, v.width) {
                st.services
                    .storage
                    .put(bucket, &vkey, v.data.into(), "image/webp")
                    .await
                    .map_err(ApiError::internal)?;
            }
        }
    }
    // la classificazione è del modulo opzionale `image_moderation`: spento, nessun controllo
    let is_sensitive = if st.module_enabled("image_moderation") {
        let threshold = noct_platform::get(&st.db).await?.moderation_threshold;
        st.services.moderation.classify(up.data.clone(), &up.filename, &up.content_type, Some(threshold)).await
    } else {
        false
    };
    Ok(Stored {
        url: st.services.storage.public_url(bucket, &key),
        key,
        content_type: up.content_type.clone(),
        size: total,
        is_sensitive,
    })
}

async fn insert_media(st: &AppState, blog: Uuid, uploader: Uuid, s: &Stored) -> Result<Uuid, ApiError> {
    Ok(sqlx::query_scalar(
        "INSERT INTO media_files (blog_id, uploader_id, object_key, url, content_type, size_bytes, is_sensitive)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(blog)
    .bind(uploader)
    .bind(&s.key)
    .bind(&s.url)
    .bind(&s.content_type)
    .bind(s.size as i64)
    .bind(s.is_sensitive)
    .fetch_one(&st.db)
    .await?)
}

pub async fn upload_media(ctx: RouteCtx) -> Result<MediaUploaded, ApiError> {
    let st = &ctx.state;
    let up = read_upload(&ctx).await?;
    let stored = store_content_image(&ctx, &up).await?;
    let media_id = insert_media(st, blog(&ctx)?.id, user(&ctx)?, &stored).await?;
    Ok(MediaUploaded { url: stored.url, is_sensitive: stored.is_sensitive, media_id: Some(media_id) })
}

// ---- libreria ---------------------------------------------------------------------------------------

#[derive(FromRow)]
struct MediaRow {
    id: Uuid,
    url: String,
    content_type: String,
    size_bytes: i64,
    alt_text: String,
    caption: Option<String>,
    categories: Vec<String>,
    is_sensitive: bool,
    uploader_username: Option<String>,
    created_at: DateTime<Utc>,
}

const MEDIA_SELECT: &str = "SELECT m.id, m.url, m.content_type, m.size_bytes, m.alt_text, m.caption, m.categories, \
    m.is_sensitive, u.username AS uploader_username, m.created_at \
    FROM media_files m LEFT JOIN users u ON u.id = m.uploader_id";

/// Post che usano ciascun URL, nel testo (`post_media`) o come copertina propria: un'immagine
/// usata solo come copertina non deve risultare libera (e quindi cancellabile).
async fn usages(st: &AppState, blog: &BlogRef, urls: &[String]) -> Result<HashMap<String, Vec<Value>>, ApiError> {
    if urls.is_empty() {
        return Ok(HashMap::new());
    }
    let rows: Vec<(String, Uuid, String, String)> = sqlx::query_as(
        "SELECT pm.url, p.id, p.slug, p.title FROM post_media pm JOIN posts p ON p.id = pm.post_id
         WHERE p.blog_id = $1 AND pm.url = ANY($2)
         UNION
         SELECT p.cover_image_url, p.id, p.slug, p.title FROM posts p
         WHERE p.blog_id = $1 AND p.cover_image_url = ANY($2)
         ORDER BY 1, 4, 2",
    )
    .bind(blog.id)
    .bind(urls)
    .fetch_all(&st.db)
    .await?;
    let mut out: HashMap<String, Vec<Value>> = HashMap::new();
    for (url, id, slug, title) in rows {
        out.entry(url).or_default().push(json!({
            "post_id": id, "post_slug": slug, "post_title": title, "permalink": format!("/{}/{slug}", blog.slug),
        }));
    }
    Ok(out)
}

async fn media_out(st: &AppState, blog: &BlogRef, rows: Vec<MediaRow>) -> Result<Vec<Value>, ApiError> {
    let urls: Vec<String> = rows.iter().map(|r| r.url.clone()).collect();
    let mut used = usages(st, blog, &urls).await?;
    let cover: Option<String> =
        sqlx::query_scalar("SELECT cover_image_url FROM blogs WHERE id = $1").bind(blog.id).fetch_one(&st.db).await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let used_in = used.remove(&r.url).unwrap_or_default();
            json!({
                "id": r.id, "url": r.url, "content_type": r.content_type, "size_bytes": r.size_bytes,
                "alt_text": r.alt_text, "caption": r.caption, "categories": r.categories,
                "is_sensitive": r.is_sensitive, "uploader_username": r.uploader_username,
                "created_at": r.created_at, "used_in": used_in,
                "used_as_blog_cover": cover.as_deref() == Some(r.url.as_str()),
            })
        })
        .collect())
}

async fn library(st: &AppState, blog: &BlogRef) -> Result<MediaLibraryOut, ApiError> {
    let rows: Vec<MediaRow> =
        sqlx::query_as(&format!("{MEDIA_SELECT} WHERE m.blog_id = $1 ORDER BY m.created_at DESC, m.id"))
            .bind(blog.id)
            .fetch_all(&st.db)
            .await?;
    let total_bytes = rows.iter().map(|r| r.size_bytes).sum();
    Ok(MediaLibraryOut { items: Value::Array(media_out(st, blog, rows).await?), total_bytes })
}

pub async fn list_media(ctx: RouteCtx) -> Result<MediaLibraryOut, ApiError> {
    let st = &ctx.state;
    let b = blog(&ctx)?;
    let uid = user(&ctx)?;
    if b.owner_id != uid && membership_role(st, uid, b.id).await?.is_none() {
        return Err(ApiError::forbidden("Serve essere proprietario o collaboratore del blog."));
    }
    library(st, b).await
}

pub async fn sync_media(ctx: RouteCtx) -> Result<MediaLibraryOut, ApiError> {
    let st = &ctx.state;
    let b = blog(&ctx)?;
    // una sola riga per URL; una citazione sensibile basta a marcare l'intera riga importata
    sqlx::query(
        "WITH refs AS (
             SELECT pm.url, pm.alt_text, pm.categories, pm.is_sensitive, p.created_at, pm.position
             FROM post_media pm JOIN posts p ON p.id = pm.post_id WHERE p.blog_id = $1
         ), sens AS (
             SELECT url, bool_or(is_sensitive) AS is_sensitive FROM refs GROUP BY url
         )
         INSERT INTO media_files (blog_id, uploader_id, object_key, url, alt_text, categories, is_sensitive)
         SELECT $1, NULL, NULL, f.url, f.alt_text, f.categories, sens.is_sensitive
         FROM (SELECT DISTINCT ON (url) url, alt_text, categories FROM refs ORDER BY url, created_at, position) f
         JOIN sens ON sens.url = f.url
         ON CONFLICT (url) DO NOTHING",
    )
    .bind(b.id)
    .execute(&st.db)
    .await?;
    library(st, b).await
}

async fn media_in_blog(st: &AppState, blog: &BlogRef, id: Uuid) -> Result<(String, Option<String>), ApiError> {
    sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT url, object_key FROM media_files WHERE id = $1 AND blog_id = $2",
    )
    .bind(id)
    .bind(blog.id)
    .fetch_optional(&st.db)
    .await?
    .ok_or_else(|| ApiError::not_found("Media non trovato."))
}

pub async fn update_media(ctx: RouteCtx, body: MediaFileUpdate) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let b = blog(&ctx)?;
    let id = parse_uuid(ctx.path.get("media_id").map(String::as_str).unwrap_or_default())?;
    media_in_blog(st, b, id).await?;
    let categories = match &body.categories {
        Some(c) => Some(validate_categories(c).map_err(ApiError::bad_request)?),
        None => None,
    };
    let cut = |s: &str| s.trim().chars().take(500).collect::<String>();
    let mut qb = QueryBuilder::<Postgres>::new("UPDATE media_files SET updated_at = now()");
    if let Some(v) = &body.alt_text {
        qb.push(", alt_text = ").push_bind(cut(v));
    }
    if let Some(v) = &body.caption {
        qb.push(", caption = ").push_bind(Some(cut(v)).filter(|c| !c.is_empty()));
    }
    if let Some(v) = categories {
        qb.push(", categories = ").push_bind(v);
    }
    qb.push(" WHERE id = ").push_bind(id);
    qb.build().execute(&st.db).await?;
    let row: MediaRow = sqlx::query_as(&format!("{MEDIA_SELECT} WHERE m.id = $1")).bind(id).fetch_one(&st.db).await?;
    media_out(st, b, vec![row]).await?.pop().ok_or_else(|| ApiError::not_found("Media non trovato."))
}

pub async fn delete_media(ctx: RouteCtx) -> Result<(), ApiError> {
    let st = &ctx.state;
    let b = blog(&ctx)?;
    let id = parse_uuid(ctx.path.get("media_id").map(String::as_str).unwrap_or_default())?;
    let (url, key) = media_in_blog(st, b, id).await?;
    let cover: Option<String> =
        sqlx::query_scalar("SELECT cover_image_url FROM blogs WHERE id = $1").bind(b.id).fetch_one(&st.db).await?;
    if cover.as_deref() == Some(url.as_str()) {
        return Err(ApiError::conflict("L'immagine è ancora la cover del blog: rimuovila prima da lì."));
    }
    if !usages(st, b, std::slice::from_ref(&url)).await?.is_empty() {
        return Err(ApiError::conflict("L'immagine è ancora usata in un post: rimuovila prima dal contenuto."));
    }
    sqlx::query("DELETE FROM media_files WHERE id = $1").bind(id).execute(&st.db).await?;
    if let Some(key) = key {
        let bucket = &st.config.storage.bucket_content;
        for k in std::iter::once(key.clone()).chain(variants::variant_keys(&key)) {
            if let Err(e) = st.services.storage.delete(bucket, &k).await {
                // la riga è già andata: un oggetto orfano non deve far fallire la richiesta
                tracing::warn!(error = %e, key = k, "cancellazione dell'oggetto su storage non riuscita");
            }
        }
    }
    Ok(())
}

// ---- copertina e favicon ----------------------------------------------------------------------------------

async fn images_response(ctx: &RouteCtx) -> Result<Value, ApiError> {
    let b = blog(ctx)?;
    ctx.state.services.revalidate.spawn(vec![revalidate::blog(&b.slug)]);
    noct_blogs::view::render_one(&ctx.state, b.id, Some(user(ctx)?)).await
}

pub async fn upload_cover(ctx: RouteCtx) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let b = owner_only(&ctx)?;
    let up = read_upload(&ctx).await?;
    let stored = store_content_image(&ctx, &up).await?;
    // come ogni altra immagine, anche la copertina entra nella libreria
    insert_media(st, b.id, user(&ctx)?, &stored).await?;
    sqlx::query(
        "UPDATE blogs SET cover_image_url = $2, cover_image_is_sensitive = $3, cover_image_categories = '{}',
                cover_image_alt_text = '', updated_at = now() WHERE id = $1",
    )
    .bind(b.id)
    .bind(&stored.url)
    .bind(stored.is_sensitive)
    .execute(&st.db)
    .await?;
    images_response(&ctx).await
}

pub async fn update_cover(ctx: RouteCtx, body: CoverImageUpdate) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let b = owner_only(&ctx)?;
    let has_cover: bool = sqlx::query_scalar("SELECT cover_image_url IS NOT NULL FROM blogs WHERE id = $1")
        .bind(b.id)
        .fetch_one(&st.db)
        .await?;
    if !has_cover {
        return Err(ApiError::bad_request("Il blog non ha una cover da annotare."));
    }
    let categories = validate_categories(&body.categories).map_err(ApiError::bad_request)?;
    let mut qb = QueryBuilder::<Postgres>::new("UPDATE blogs SET updated_at = now()");
    qb.push(", cover_image_is_sensitive = ").push_bind(!categories.is_empty());
    qb.push(", cover_image_categories = ").push_bind(categories);
    if body_map(&ctx.body).contains_key("alt_text") {
        qb.push(", cover_image_alt_text = ").push_bind(body.alt_text.clone().unwrap_or_default());
    }
    qb.push(" WHERE id = ").push_bind(b.id);
    qb.build().execute(&st.db).await?;
    images_response(&ctx).await
}

pub async fn delete_cover(ctx: RouteCtx) -> Result<Value, ApiError> {
    let b = owner_only(&ctx)?;
    // l'oggetto su storage resta: potrebbe essere citato altrove
    sqlx::query(
        "UPDATE blogs SET cover_image_url = NULL, cover_image_is_sensitive = false, cover_image_categories = '{}',
                cover_image_alt_text = '', updated_at = now() WHERE id = $1",
    )
    .bind(b.id)
    .execute(&ctx.state.db)
    .await?;
    images_response(&ctx).await
}

pub async fn upload_favicon(ctx: RouteCtx) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let b = owner_only(&ctx)?;
    let up = read_upload(&ctx).await?;
    let ext = upload::check_image(&up, FAVICON_TYPES, MAX_FAVICON_BYTES, "PNG, JPEG o WEBP", "512 KiB")?;
    let key = upload::favicon_key(b.id, ext);
    let bucket = &st.config.storage.bucket_avatars;
    st.services.storage.put(bucket, &key, up.data.clone(), &up.content_type).await.map_err(ApiError::internal)?;
    let old: Option<String> =
        sqlx::query_scalar("SELECT favicon_object_key FROM blogs WHERE id = $1").bind(b.id).fetch_one(&st.db).await?;
    sqlx::query("UPDATE blogs SET favicon_object_key = $2, updated_at = now() WHERE id = $1")
        .bind(b.id)
        .bind(&key)
        .execute(&st.db)
        .await?;
    // la favicon è piccola e dedicata: la precedente si cancella (a differenza della copertina)
    if let Some(old) = old
        && let Err(e) = st.services.storage.delete(bucket, &old).await
    {
        tracing::warn!(error = %e, old, "cancellazione della favicon precedente non riuscita");
    }
    images_response(&ctx).await
}

pub async fn delete_favicon(ctx: RouteCtx) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let b = owner_only(&ctx)?;
    let old: Option<String> =
        sqlx::query_scalar("SELECT favicon_object_key FROM blogs WHERE id = $1").bind(b.id).fetch_one(&st.db).await?;
    if let Some(old) = old {
        sqlx::query("UPDATE blogs SET favicon_object_key = NULL, updated_at = now() WHERE id = $1")
            .bind(b.id)
            .execute(&st.db)
            .await?;
        if let Err(e) = st.services.storage.delete(&st.config.storage.bucket_avatars, &old).await {
            tracing::warn!(error = %e, old, "cancellazione della favicon non riuscita");
        }
    }
    images_response(&ctx).await
}

/// Libreria di tutti i blog dell'utente (proprietario o collaboratore, esclusi quelli in
/// cancellazione), con il blog di appartenenza e i post che citano ogni immagine.
pub async fn my_media(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let st = &ctx.state;
    let uid = user(&ctx)?;
    const MINE: &str = "b.deleted_at IS NULL AND (b.owner_id = $1 OR EXISTS \
        (SELECT 1 FROM blog_memberships mb WHERE mb.blog_id = b.id AND mb.user_id = $1))";
    #[derive(FromRow)]
    struct Row {
        #[sqlx(flatten)]
        media: MediaRow,
        blog_slug: String,
        blog_title: String,
    }
    let rows: Vec<Row> = sqlx::query_as(&format!(
        "SELECT m.id, m.url, m.content_type, m.size_bytes, m.alt_text, m.caption, m.categories, m.is_sensitive,
                u.username AS uploader_username, m.created_at, b.slug AS blog_slug, b.title AS blog_title
         FROM media_files m JOIN blogs b ON b.id = m.blog_id LEFT JOIN users u ON u.id = m.uploader_id
         WHERE {MINE} ORDER BY m.created_at DESC, m.id"
    ))
    .bind(uid)
    .fetch_all(&st.db)
    .await?;
    let urls: Vec<String> = rows.iter().map(|r| r.media.url.clone()).collect();
    let usage: Vec<(String, Uuid, String, String, String)> = sqlx::query_as(&format!(
        "SELECT pm.url, p.id, p.slug, p.title, b.slug FROM post_media pm JOIN posts p ON p.id = pm.post_id
         JOIN blogs b ON b.id = p.blog_id WHERE {MINE} AND pm.url = ANY($2)
         UNION
         SELECT p.cover_image_url, p.id, p.slug, p.title, b.slug FROM posts p JOIN blogs b ON b.id = p.blog_id
         WHERE {MINE} AND p.cover_image_url = ANY($2)
         ORDER BY 1, 4, 2"
    ))
    .bind(uid)
    .bind(&urls)
    .fetch_all(&st.db)
    .await?;
    let mut used: HashMap<String, Vec<Value>> = HashMap::new();
    for (url, id, slug, title, blog_slug) in usage {
        used.entry(url).or_default().push(json!({
            "post_id": id, "post_slug": slug, "post_title": title, "permalink": format!("/{blog_slug}/{slug}"),
        }));
    }
    Ok(rows
        .into_iter()
        .map(|r| {
            let m = r.media;
            json!({
                "id": m.id, "url": m.url, "content_type": m.content_type, "size_bytes": m.size_bytes,
                "alt_text": m.alt_text, "caption": m.caption, "categories": m.categories,
                "is_sensitive": m.is_sensitive, "uploader_username": m.uploader_username,
                "created_at": m.created_at, "used_in": used.remove(&m.url).unwrap_or_default(),
                "blog_slug": r.blog_slug, "blog_title": r.blog_title,
            })
        })
        .collect())
}
