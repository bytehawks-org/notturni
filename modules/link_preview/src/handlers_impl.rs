//! `GET /link-preview?url=`: cache calda (Redis) → tabella persistente → fetch dal vivo.

use crate::{LinkPreview, fetch};
use chrono::{DateTime, Duration, Utc};
use noct_core::{ApiError, RouteCtx};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::time::Duration as StdDuration;

const HOT_TTL: StdDuration = StdDuration::from_secs(6 * 3600);
/// Un'anteprima con dati reali resta valida una settimana; un fallimento molto meno, per non
/// restare bloccati su un host momentaneamente giù né martellarlo a ogni richiesta.
const STALE_AFTER_OK: Duration = Duration::days(7);
const STALE_AFTER_EMPTY: Duration = Duration::hours(6);

/// (title, description, image, fetch_ok, updated_at)
type CachedRow = (Option<String>, Option<String>, Option<String>, bool, DateTime<Utc>);

fn url_hash(url: &str) -> String {
    hex::encode(Sha256::digest(url.trim().as_bytes()))
}

fn preview_json(p: &fetch::Preview) -> serde_json::Value {
    json!({"url": p.url, "title": p.title, "description": p.description, "image": p.image})
}

pub async fn get_link_preview(ctx: RouteCtx) -> Result<LinkPreview, ApiError> {
    let raw = ctx
        .query
        .get("url")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::unprocessable("Il parametro `url` è obbligatorio."))?;
    let allow_private = ctx.config_bool("allow_private_hosts").unwrap_or(false);
    let url = fetch::parse_url(raw).map_err(ApiError::bad_request)?;
    if fetch::resolve_public(&url, allow_private).await.is_none() {
        return Err(ApiError::bad_request("URL non raggiungibile per l'anteprima."));
    }

    let st = &ctx.state;
    let hash = url_hash(raw);
    let cache_key = format!("link-preview:{hash}");
    // la cache calda è un'ottimizzazione: se non risponde si prosegue senza
    let hot = st.services.cache.get(&cache_key).await.ok().flatten();
    if let Some(v) = hot.and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok()) {
        let field = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
        return Ok(LinkPreview {
            url: raw.to_string(),
            title: field("title"),
            description: field("description"),
            image: field("image"),
        });
    }

    let row: Option<CachedRow> = sqlx::query_as(
        "SELECT title, description, image, fetch_ok, updated_at FROM link_preview_cache WHERE url_hash = $1",
    )
    .bind(&hash)
    .fetch_optional(&st.db)
    .await?;
    if let Some((title, description, image, ok, updated_at)) = row {
        let fresh = Utc::now() - updated_at < if ok { STALE_AFTER_OK } else { STALE_AFTER_EMPTY };
        if fresh {
            let preview = LinkPreview { url: raw.to_string(), title, description, image };
            warm(&ctx, &cache_key, &preview).await;
            return Ok(preview);
        }
    }

    let live = fetch::fetch(raw, allow_private).await;
    sqlx::query(
        "INSERT INTO link_preview_cache (url_hash, url, title, description, image, fetch_ok)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (url_hash) DO UPDATE SET title = EXCLUDED.title, description = EXCLUDED.description,
           image = EXCLUDED.image, fetch_ok = EXCLUDED.fetch_ok, updated_at = now()",
    )
    .bind(&hash)
    .bind(raw)
    .bind(&live.title)
    .bind(&live.description)
    .bind(&live.image)
    .bind(!live.is_empty())
    .execute(&st.db)
    .await?;
    let preview = LinkPreview { url: live.url, title: live.title, description: live.description, image: live.image };
    warm(&ctx, &cache_key, &preview).await;
    Ok(preview)
}

async fn warm(ctx: &RouteCtx, key: &str, preview: &LinkPreview) {
    let value = preview_json(&fetch::Preview {
        url: preview.url.clone(),
        title: preview.title.clone(),
        description: preview.description.clone(),
        image: preview.image.clone(),
    })
    .to_string();
    if let Err(e) = ctx.state.services.cache.set(key, &value, Some(HOT_TTL)).await {
        tracing::warn!("cache calda dell'anteprima link non disponibile: {e}");
    }
}
