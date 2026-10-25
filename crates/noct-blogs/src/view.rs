//! Lettura e serializzazione di `BlogOut`, condivisa con gli altri crate che restituiscono
//! un blog (media: copertina e favicon).

use crate::BlogOut;
use noct_core::{ApiError, AppState, Entity};
use serde_json::Value;
use uuid::Uuid;

/// Colonne di `BlogOut`. `favicon_url` contiene ancora la chiave dell'oggetto: l'URL si
/// risolve in [`render`].
pub const BLOG_COLUMNS: &str = "b.id, b.slug, b.title, b.subtitle, b.description, b.kind::text AS kind, b.display_mode::text AS display_mode, b.visibility::text AS visibility, \
    b.default_locale, b.extra_locales, b.mentions_enabled, b.search_indexing_enabled, b.ai_crawling_enabled, \
    b.is_paused, b.is_suspended, b.deleted_at, b.default_author_display_name, b.cover_image_url, \
    b.cover_image_is_sensitive, b.cover_image_categories, b.cover_image_alt_text, \
    b.favicon_object_key AS favicon_url, b.owner_id, b.created_at";

/// I blog `ids`, nello stesso ordine (gli id mancanti sono omessi).
pub async fn fetch(st: &AppState, ids: &[Uuid]) -> Result<Vec<BlogOut>, ApiError> {
    let mut rows: Vec<BlogOut> = sqlx::query_as(&format!("SELECT {BLOG_COLUMNS} FROM blogs b WHERE b.id = ANY($1)"))
        .bind(ids)
        .fetch_all(&st.db)
        .await?;
    rows.sort_by_key(|r| ids.iter().position(|i| *i == r.id).unwrap_or(usize::MAX));
    Ok(rows)
}

/// Forma pubblica di un elenco di blog: `owner_id` solo per il proprietario (un blog con
/// alias non deve permettere di risalire all'identità reale), URL della favicon risolto, campi
/// dei moduli attivi aggiunti con una query per modulo.
pub async fn render(st: &AppState, rows: Vec<BlogOut>, viewer: Option<Uuid>) -> Result<Vec<Value>, ApiError> {
    let ids: Vec<Uuid> = rows.iter().map(|b| b.id).collect();
    let mut extra = st.extensions.enrich(&st.db, Entity::Blog, &ids).await?;
    let mut out = Vec::with_capacity(rows.len());
    for mut b in rows {
        if viewer.is_none() || viewer != b.owner_id {
            b.owner_id = None;
        }
        if let Some(key) = b.favicon_url.take() {
            b.favicon_url = Some(st.services.storage.public_url(&st.config.storage.bucket_avatars, &key));
        }
        let id = b.id;
        let mut v = serde_json::to_value(&b).map_err(ApiError::internal)?;
        if let (Some(obj), Some(fields)) = (v.as_object_mut(), extra.remove(&id)) {
            obj.extend(fields);
        }
        out.push(v);
    }
    Ok(out)
}

/// Un solo blog già autorizzato.
pub async fn render_one(st: &AppState, id: Uuid, viewer: Option<Uuid>) -> Result<Value, ApiError> {
    let rows = fetch(st, &[id]).await?;
    render(st, rows, viewer).await?.pop().ok_or_else(|| ApiError::not_found("Blog non trovato."))
}
