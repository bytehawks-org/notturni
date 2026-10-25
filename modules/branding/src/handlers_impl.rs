//! Handler `custom` del modulo. Le immagini stanno nel bucket pubblico degli avatar sotto `branding/`.

use crate::{
    SeoSettingsIn, icons,
    rules::{self, SeoText},
};
use bytes::Bytes;
use noct_audit::{Entry, user_actor};
use noct_core::{ApiError, AppState, RouteCtx, revalidate};
use noct_media::upload::{self, FAVICON_TYPES, Upload};
use serde_json::{Map, Value, json};
use sqlx::{FromRow, PgConnection};
use std::collections::BTreeMap;
use uuid::Uuid;

const DEFAULT_SITE_NAME: &str = "Notturni";
const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;
const MIN_SOCIAL_WIDTH: u32 = 600;

#[derive(FromRow, Default)]
struct Branding {
    site_name: Option<String>,
    icon_key: Option<String>,
    icon_sizes: Option<String>,
    social_image_key: Option<String>,
    noindex: bool,
    google_verification: Option<String>,
    bing_verification: Option<String>,
    yandex_verification: Option<String>,
    twitter_site: Option<String>,
    theme_color: Option<String>,
    extra_meta: Value,
}

const BRANDING_SELECT: &str = "SELECT site_name, icon_key, icon_sizes, social_image_key, noindex, google_verification,
    bing_verification, yandex_verification, twitter_site, theme_color, extra_meta FROM site_branding WHERE id = 1";

#[derive(FromRow)]
struct SeoRow {
    locale: String,
    title: Option<String>,
    description: Option<String>,
    keywords: Option<String>,
}

fn admin(ctx: &RouteCtx) -> Result<Uuid, ApiError> {
    ctx.actor.as_ref().map(|a| a.user_id).ok_or_else(|| ApiError::unauthorized("Autenticazione richiesta."))
}

async fn load(conn: &mut PgConnection) -> Result<Branding, ApiError> {
    let row: Option<Branding> = sqlx::query_as(BRANDING_SELECT).fetch_optional(conn).await?;
    Ok(row.unwrap_or_default())
}

fn bucket(st: &AppState) -> &str {
    &st.config.storage.bucket_avatars
}

fn url(st: &AppState, key: &str) -> String {
    st.services.storage.public_url(bucket(st), key)
}

fn sizes(b: &Branding) -> Vec<u32> {
    b.icon_sizes.as_deref().unwrap_or_default().split(',').filter_map(|s| s.trim().parse().ok()).collect()
}

/// `(lato, URL)` delle PNG dell'icona.
fn icon_urls(st: &AppState, b: &Branding) -> Vec<(u32, String)> {
    match &b.icon_key {
        Some(prefix) => sizes(b).into_iter().map(|s| (s, url(st, &format!("{prefix}-{s}.png")))).collect(),
        None => Vec::new(),
    }
}

fn revalidate_site(st: &AppState) {
    st.services.revalidate.spawn(vec![revalidate::site_seo()]);
}

async fn audit(
    conn: &mut PgConnection,
    ctx: &RouteCtx,
    user: Uuid,
    action: &str,
    payload: Value,
) -> Result<(), ApiError> {
    let who = user_actor(&mut *conn, user).await.map_err(ApiError::internal)?;
    noct_audit::record(conn, Entry { action, actor: who, target: None, blog_id: None, meta: &ctx.meta, payload })
        .await
        .map_err(ApiError::internal)
}

// ---- lettura -------------------------------------------------------------------------------------------------------------

pub async fn get_seo_site(ctx: RouteCtx) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let mut conn = st.db.acquire().await?;
    let b = load(&mut conn).await?;
    let rows: Vec<SeoRow> = sqlx::query_as("SELECT locale, title, description, keywords FROM site_seo ORDER BY locale")
        .fetch_all(&mut *conn)
        .await?;
    let default = noct_platform::get(&st.db).await?.default_locale;
    let asked = ctx.query.get("locale").map(|l| l.to_ascii_lowercase());
    let available: Vec<String> = rows.iter().map(|r| r.locale.clone()).collect();
    let picked = rules::pick_locale(&available, asked.as_deref(), &default);
    let row = picked.as_ref().and_then(|l| rows.iter().find(|r| &r.locale == l));
    let icons = icon_urls(st, &b);
    let apple = icons.iter().find(|(s, _)| *s == 180).map(|(_, u)| u.clone());
    let extra: Vec<Value> = b
        .extra_meta
        .as_array()
        .map(|a| a.iter().filter(|m| m.get("name").is_some() && m.get("content").is_some()).cloned().collect())
        .unwrap_or_default();
    Ok(json!({
        "locale": picked.unwrap_or(default),
        "site_name": b.site_name.clone().unwrap_or_else(|| DEFAULT_SITE_NAME.to_string()),
        "title": row.and_then(|r| r.title.clone()),
        "description": row.and_then(|r| r.description.clone()),
        "keywords": row.and_then(|r| r.keywords.clone()),
        "noindex": b.noindex,
        "google_verification": b.google_verification,
        "bing_verification": b.bing_verification,
        "yandex_verification": b.yandex_verification,
        "twitter_site": b.twitter_site,
        "theme_color": b.theme_color,
        "extra_meta": extra,
        "icons": icons.iter().map(|(s, u)| json!({"url": u, "sizes": format!("{s}x{s}"), "type": "image/png"})).collect::<Vec<_>>(),
        "apple_touch_icon": apple,
        "social_image_url": b.social_image_key.as_deref().map(|k| url(st, k)),
    }))
}

async fn admin_out(st: &AppState, conn: &mut PgConnection) -> Result<Value, ApiError> {
    let b = load(conn).await?;
    let rows: Vec<SeoRow> = sqlx::query_as("SELECT locale, title, description, keywords FROM site_seo ORDER BY locale")
        .fetch_all(conn)
        .await?;
    let locales: BTreeMap<&str, Value> = rows
        .iter()
        .map(|r| (r.locale.as_str(), json!({"title": r.title, "description": r.description, "keywords": r.keywords})))
        .collect();
    let icons: Map<String, Value> = icon_urls(st, &b).into_iter().map(|(s, u)| (s.to_string(), json!(u))).collect();
    Ok(json!({
        "branding": {
            "site_name": b.site_name, "noindex": b.noindex, "google_verification": b.google_verification,
            "bing_verification": b.bing_verification, "yandex_verification": b.yandex_verification,
            "twitter_site": b.twitter_site, "theme_color": b.theme_color, "extra_meta": b.extra_meta,
            "icons": icons, "social_image_url": b.social_image_key.as_deref().map(|k| url(st, k)),
        },
        "locales": locales,
    }))
}

pub async fn get_seo_admin(ctx: RouteCtx) -> Result<Value, ApiError> {
    admin(&ctx)?;
    let mut conn = ctx.state.db.acquire().await?;
    admin_out(&ctx.state, &mut conn).await
}

// ---- impostazioni --------------------------------------------------------------------------------------------------------

pub async fn put_seo_admin(ctx: RouteCtx, body: SeoSettingsIn) -> Result<Value, ApiError> {
    let user = admin(&ctx)?;
    let st = &ctx.state;
    let bad = ApiError::bad_request;
    let site_name = rules::plain("site_name", body.site_name.as_deref().unwrap_or_default(), 100).map_err(bad)?;
    let google = rules::verification("google_verification", body.google_verification.as_deref()).map_err(bad)?;
    let bing = rules::verification("bing_verification", body.bing_verification.as_deref()).map_err(bad)?;
    let yandex = rules::verification("yandex_verification", body.yandex_verification.as_deref()).map_err(bad)?;
    let twitter = rules::twitter_site(body.twitter_site.as_deref()).map_err(bad)?;
    let color = rules::theme_color(body.theme_color.as_deref()).map_err(bad)?;
    let extra = rules::extra_meta(&body.extra_meta.clone().unwrap_or(Value::Null)).map_err(bad)?;
    let locales: Vec<(String, SeoText)> = rules::locales(&body.locales.clone().unwrap_or(Value::Null)).map_err(bad)?;
    let extra_json: Vec<Value> = extra.iter().map(|(n, c)| json!({"name": n, "content": c})).collect();

    let mut tx = st.db.begin().await?;
    sqlx::query(
        "INSERT INTO site_branding (id, site_name, noindex, google_verification, bing_verification, yandex_verification,
                twitter_site, theme_color, extra_meta, updated_by)
         VALUES (1, $1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (id) DO UPDATE SET site_name = EXCLUDED.site_name, noindex = EXCLUDED.noindex,
            google_verification = EXCLUDED.google_verification, bing_verification = EXCLUDED.bing_verification,
            yandex_verification = EXCLUDED.yandex_verification, twitter_site = EXCLUDED.twitter_site,
            theme_color = EXCLUDED.theme_color, extra_meta = EXCLUDED.extra_meta, updated_by = EXCLUDED.updated_by,
            updated_at = now()",
    )
    .bind(&site_name)
    .bind(body.noindex.unwrap_or(false))
    .bind(&google)
    .bind(&bing)
    .bind(&yandex)
    .bind(&twitter)
    .bind(&color)
    .bind(Value::Array(extra_json))
    .bind(user)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM site_seo").execute(&mut *tx).await?;
    for (locale, t) in &locales {
        sqlx::query("INSERT INTO site_seo (locale, title, description, keywords) VALUES ($1, $2, $3, $4)")
            .bind(locale)
            .bind(&t.title)
            .bind(&t.description)
            .bind(&t.keywords)
            .execute(&mut *tx)
            .await?;
    }
    audit(
        &mut tx,
        &ctx,
        user,
        "branding.settings_updated",
        json!({"noindex": body.noindex.unwrap_or(false), "locales": locales.iter().map(|(l, _)| l).collect::<Vec<_>>(),
               "extra_meta": extra.len()}),
    )
    .await?;
    let out = admin_out(st, &mut tx).await?;
    tx.commit().await?;
    revalidate_site(st);
    Ok(out)
}

// ---- immagini ------------------------------------------------------------------------------------------------------------

async fn read_image(ctx: &RouteCtx) -> Result<Upload, ApiError> {
    let ct = ctx.headers.get(axum::http::header::CONTENT_TYPE).and_then(|v| v.to_str().ok());
    let up = upload::read_file(ct, ctx.body.clone()).await?;
    upload::check_image(&up, FAVICON_TYPES, MAX_IMAGE_BYTES, "PNG, JPEG o WEBP", "2 MiB")?;
    Ok(up)
}

async fn delete_objects(st: &AppState, keys: Vec<String>) {
    for key in keys {
        if let Err(e) = st.services.storage.delete(bucket(st), &key).await {
            tracing::warn!(error = %e, key, "cancellazione dell'immagine del portale non riuscita");
        }
    }
}

fn icon_keys(b: &Branding) -> Vec<String> {
    match &b.icon_key {
        Some(prefix) => sizes(b).into_iter().map(|s| format!("{prefix}-{s}.png")).collect(),
        None => Vec::new(),
    }
}

pub async fn upload_branding_icon(ctx: RouteCtx) -> Result<Value, ApiError> {
    let user = admin(&ctx)?;
    let st = &ctx.state;
    let up = read_image(&ctx).await?;
    let data = up.data.clone();
    let derived = tokio::task::spawn_blocking(move || icons::derive(&data))
        .await
        .map_err(ApiError::internal)?
        .map_err(|e| ApiError::bad_request(e.message()))?;
    let prefix = format!("branding/icon-{}", Uuid::new_v4());
    for (side, png) in &derived {
        st.services
            .storage
            .put(bucket(st), &format!("{prefix}-{side}.png"), Bytes::from(png.clone()), "image/png")
            .await
            .map_err(ApiError::internal)?;
    }
    let list = derived.iter().map(|(s, _)| s.to_string()).collect::<Vec<_>>().join(",");
    let mut tx = st.db.begin().await?;
    let old = load(&mut tx).await?;
    sqlx::query(
        "INSERT INTO site_branding (id, icon_key, icon_sizes, updated_by) VALUES (1, $1, $2, $3)
         ON CONFLICT (id) DO UPDATE SET icon_key = EXCLUDED.icon_key, icon_sizes = EXCLUDED.icon_sizes,
            updated_by = EXCLUDED.updated_by, updated_at = now()",
    )
    .bind(&prefix)
    .bind(&list)
    .bind(user)
    .execute(&mut *tx)
    .await?;
    audit(&mut tx, &ctx, user, "branding.icon_updated", json!({"sizes": list})).await?;
    let out = admin_out(st, &mut tx).await?;
    tx.commit().await?;
    delete_objects(st, icon_keys(&old)).await;
    revalidate_site(st);
    Ok(out)
}

pub async fn delete_branding_icon(ctx: RouteCtx) -> Result<Value, ApiError> {
    let user = admin(&ctx)?;
    let st = &ctx.state;
    let mut tx = st.db.begin().await?;
    let old = load(&mut tx).await?;
    sqlx::query(
        "UPDATE site_branding SET icon_key = NULL, icon_sizes = NULL, updated_by = $1, updated_at = now() WHERE id = 1",
    )
    .bind(user)
    .execute(&mut *tx)
    .await?;
    audit(&mut tx, &ctx, user, "branding.icon_removed", json!({})).await?;
    let out = admin_out(st, &mut tx).await?;
    tx.commit().await?;
    delete_objects(st, icon_keys(&old)).await;
    revalidate_site(st);
    Ok(out)
}

pub async fn upload_branding_social(ctx: RouteCtx) -> Result<Value, ApiError> {
    let user = admin(&ctx)?;
    let st = &ctx.state;
    let up = read_image(&ctx).await?;
    let ext = upload::check_image(&up, FAVICON_TYPES, MAX_IMAGE_BYTES, "PNG, JPEG o WEBP", "2 MiB")?;
    let data = up.data.clone();
    let width = tokio::task::spawn_blocking(move || {
        image::ImageReader::new(std::io::Cursor::new(data))
            .with_guessed_format()
            .ok()
            .and_then(|r| r.into_dimensions().ok())
    })
    .await
    .map_err(ApiError::internal)?
    .map(|(w, _)| w)
    .ok_or_else(|| ApiError::bad_request("L'immagine non si riesce a leggere."))?;
    if width < MIN_SOCIAL_WIDTH {
        return Err(ApiError::bad_request(format!("L'immagine deve essere larga almeno {MIN_SOCIAL_WIDTH} pixel.")));
    }
    let key = format!("branding/social-{}.{ext}", Uuid::new_v4());
    st.services.storage.put(bucket(st), &key, up.data.clone(), &up.content_type).await.map_err(ApiError::internal)?;
    let mut tx = st.db.begin().await?;
    let old = load(&mut tx).await?;
    sqlx::query(
        "INSERT INTO site_branding (id, social_image_key, updated_by) VALUES (1, $1, $2)
         ON CONFLICT (id) DO UPDATE SET social_image_key = EXCLUDED.social_image_key, updated_by = EXCLUDED.updated_by,
            updated_at = now()",
    )
    .bind(&key)
    .bind(user)
    .execute(&mut *tx)
    .await?;
    audit(&mut tx, &ctx, user, "branding.social_updated", json!({"width": width})).await?;
    let out = admin_out(st, &mut tx).await?;
    tx.commit().await?;
    delete_objects(st, old.social_image_key.into_iter().collect()).await;
    revalidate_site(st);
    Ok(out)
}

pub async fn delete_branding_social(ctx: RouteCtx) -> Result<Value, ApiError> {
    let user = admin(&ctx)?;
    let st = &ctx.state;
    let mut tx = st.db.begin().await?;
    let old = load(&mut tx).await?;
    sqlx::query("UPDATE site_branding SET social_image_key = NULL, updated_by = $1, updated_at = now() WHERE id = 1")
        .bind(user)
        .execute(&mut *tx)
        .await?;
    audit(&mut tx, &ctx, user, "branding.social_removed", json!({})).await?;
    let out = admin_out(st, &mut tx).await?;
    tx.commit().await?;
    delete_objects(st, old.social_image_key.into_iter().collect()).await;
    revalidate_site(st);
    Ok(out)
}
