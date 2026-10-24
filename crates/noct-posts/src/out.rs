//! Lettura e serializzazione di `PostOut`: una query per elenco (autori, alias di membership,
//! campi delle estensioni), mai una per riga.

use crate::rules::{self, UserNames};
use chrono::{DateTime, Utc};
use noct_core::{ApiError, AppState, Entity};
use serde_json::{Value, json};
use sqlx::FromRow;
use std::collections::HashMap;
use uuid::Uuid;

/// Post con i dati del blog che servono a serializzarlo.
#[derive(Debug, Clone, FromRow)]
pub struct PostRow {
    pub id: Uuid,
    pub blog_id: Uuid,
    pub author_id: Uuid,
    pub author_display_name: String,
    pub locale: String,
    pub translation_group_id: Uuid,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub cover_image_url: Option<String>,
    pub cover_image_is_sensitive: bool,
    pub cover_image_categories: Vec<String>,
    pub cover_image_alt_text: String,
    pub manual_tags: Vec<String>,
    pub status: String,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub is_hidden: bool,
    pub search_indexing_enabled: Option<bool>,
    pub ai_crawling_enabled: Option<bool>,
    pub blog_slug: String,
    pub blog_default_locale: String,
    pub blog_owner_id: Uuid,
    pub blog_mentions_enabled: bool,
    pub blog_search_indexing_enabled: bool,
    pub blog_ai_crawling_enabled: bool,
    pub blog_default_author_name: Option<String>,
}

impl PostRow {
    /// Colonne di `PostRow` (alias `p` per il post, `b` per il blog).
    pub const SELECT: &str = "SELECT p.id, p.blog_id, p.author_id, p.author_display_name, p.locale, \
        p.translation_group_id, p.title, p.slug, p.content, p.cover_image_url, p.cover_image_is_sensitive, \
        p.cover_image_categories, p.cover_image_alt_text, p.manual_tags, p.status::text AS status, p.published_at, \
        p.created_at, p.is_hidden, p.search_indexing_enabled, p.ai_crawling_enabled, \
        b.slug AS blog_slug, b.default_locale AS blog_default_locale, b.owner_id AS blog_owner_id, \
        b.mentions_enabled AS blog_mentions_enabled, b.search_indexing_enabled AS blog_search_indexing_enabled, \
        b.ai_crawling_enabled AS blog_ai_crawling_enabled, b.default_author_display_name AS blog_default_author_name \
        FROM posts p JOIN blogs b ON b.id = p.blog_id";

    /// Pubblicato, non pianificato nel futuro, non nascosto da un admin.
    pub fn is_publicly_visible(&self) -> bool {
        !self.is_hidden && self.status == "published" && self.published_at.is_none_or(|t| t <= Utc::now())
    }
}

/// Equivalente SQL di [`PostRow::is_publicly_visible`] (alias `p`).
pub const VISIBLE_SQL: &str =
    "NOT p.is_hidden AND p.status = 'published' AND (p.published_at IS NULL OR p.published_at <= now())";

#[derive(FromRow)]
struct AuthorRow {
    id: Uuid,
    username: String,
    display_name: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,
    style: String,
    avatar_object_key: Option<String>,
}

/// Forma pubblica di un elenco di post.
///
/// Il nome dell'autore è ricalcolato a ogni lettura (non preso dalla colonna): un alias di
/// membership o di blog cambiato dopo la pubblicazione si riflette subito su tutti i post.
pub async fn render(st: &AppState, rows: Vec<PostRow>) -> Result<Vec<Value>, ApiError> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let author_ids: Vec<Uuid> = rows.iter().map(|r| r.author_id).collect();
    let authors: HashMap<Uuid, AuthorRow> = sqlx::query_as::<_, AuthorRow>(
        "SELECT id, username, display_name, first_name, last_name, post_author_name_style::text AS style, avatar_object_key
         FROM users WHERE id = ANY($1)",
    )
    .bind(&author_ids)
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(|a| (a.id, a))
    .collect();
    let (m_users, m_blogs): (Vec<Uuid>, Vec<Uuid>) = rows.iter().map(|r| (r.author_id, r.blog_id)).unzip();
    let aliases: HashMap<(Uuid, Uuid), String> = sqlx::query_as::<_, (Uuid, Uuid, Option<String>)>(
        "SELECT user_id, blog_id, author_display_name FROM blog_memberships
         WHERE (user_id, blog_id) IN (SELECT * FROM UNNEST($1::uuid[], $2::uuid[]))",
    )
    .bind(&m_users)
    .bind(&m_blogs)
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .filter_map(|(u, b, alias)| alias.map(|a| ((u, b), a)))
    .collect();
    let mut extra = st.extensions.enrich(&st.db, Entity::Post, &ids).await?;

    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let author = authors.get(&r.author_id);
        let author_display_name = match author {
            Some(a) => rules::pick_display_name(
                aliases.get(&(r.author_id, r.blog_id)).map(String::as_str),
                r.blog_default_author_name.as_deref(),
                &UserNames {
                    username: a.username.clone(),
                    display_name: a.display_name.clone(),
                    first_name: a.first_name.clone(),
                    last_name: a.last_name.clone(),
                    style: a.style.clone(),
                },
            ),
            None => r.author_display_name.clone(),
        };
        let avatar = author
            .and_then(|a| a.avatar_object_key.as_deref())
            .map(|k| st.services.storage.public_url(&st.config.storage.bucket_avatars, k));
        let mut v = json!({
            "id": r.id,
            "blog_id": r.blog_id,
            "author_id": r.author_id,
            "author_display_name": author_display_name,
            "author_avatar_url": avatar,
            "locale": r.locale,
            "translation_group_id": r.translation_group_id,
            "title": r.title,
            "slug": r.slug,
            "content": r.content,
            "cover_image_url": r.cover_image_url,
            "cover_image_is_sensitive": r.cover_image_is_sensitive,
            "cover_image_categories": r.cover_image_categories,
            "cover_image_alt_text": r.cover_image_alt_text,
            "status": r.status,
            "published_at": r.published_at,
            "created_at": r.created_at,
            "is_hidden": r.is_hidden,
            "blog_slug": r.blog_slug,
            "permalink": rules::permalink(&r.blog_slug, &r.slug),
            "mentions_enabled": r.blog_mentions_enabled,
            "manual_tags": r.manual_tags,
            "tags": rules::effective_tags(&r.manual_tags, &r.content),
            "search_indexing_enabled": r.search_indexing_enabled,
            "ai_crawling_enabled": r.ai_crawling_enabled,
            "effective_search_indexing_enabled": rules::effective_flag(r.blog_search_indexing_enabled, r.search_indexing_enabled),
            "effective_ai_crawling_enabled": rules::effective_flag(r.blog_ai_crawling_enabled, r.ai_crawling_enabled),
        });
        if let (Some(obj), Some(fields)) = (v.as_object_mut(), extra.remove(&r.id)) {
            obj.extend(fields);
        }
        out.push(v);
    }
    Ok(out)
}
