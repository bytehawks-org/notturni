//! Esportazione completa di un blog (`GET /blogs/{slug}/export`): uno zip in formati aperti, generato al
//! volo. Un Markdown per post (front matter con i metadati), `blog.json`, `media.json` e `links.json`;
//! i moduli attivi aggiungono i loro file (pagine, commenti, categorie, note) con
//! [`noct_core::AdminSection::blog_export`]. I binari delle immagini restano sullo storage: `media.json`
//! elenca gli URL.

use chrono::{DateTime, Utc};
use noct_core::{ApiError, RouteCtx};
use serde_json::{Value, json};
use std::io::Write;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

/// Front matter YAML minimo: i valori con `:` o altri caratteri speciali vanno tra virgolette.
pub fn front_matter(fields: &[(&str, Value)]) -> String {
    let mut out = String::from("---\n");
    for (key, value) in fields {
        match value {
            Value::Null => {}
            Value::String(s) if s.chars().any(|c| matches!(c, ':' | '#' | '"' | '\n')) || s.is_empty() => {
                out.push_str(&format!("{key}: {}\n", serde_json::to_string(s).unwrap_or_default()));
            }
            Value::String(s) => out.push_str(&format!("{key}: {s}\n")),
            other => out.push_str(&format!("{key}: {other}\n")),
        }
    }
    out.push_str("---\n");
    out
}

fn zip_error(e: impl std::fmt::Display) -> ApiError {
    ApiError::internal(format!("zip: {e}"))
}

/// Nome di file sicuro dentro lo zip: niente separatori né percorsi relativi.
fn file_part(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '_' }).collect()
}

type PostRow =
    (uuid::Uuid, String, String, String, String, String, String, Option<DateTime<Utc>>, Vec<String>, DateTime<Utc>);

#[derive(sqlx::FromRow)]
struct Head {
    title: String,
    subtitle: Option<String>,
    description: Option<String>,
    visibility: String,
    default_locale: String,
    extra_locales: Vec<String>,
    default_author_display_name: Option<String>,
    created_at: DateTime<Utc>,
}

pub async fn export_blog(ctx: RouteCtx) -> Result<axum::response::Response, ApiError> {
    use axum::{
        http::{HeaderValue, StatusCode, header},
        response::IntoResponse,
    };
    let st = &ctx.state;
    let blog = ctx.blog.as_ref().ok_or_else(|| ApiError::internal("scope `blog` assente"))?;
    let actor = ctx.actor.as_ref().ok_or_else(|| ApiError::unauthorized("Autenticazione richiesta."))?;
    if blog.owner_id != actor.user_id {
        return Err(ApiError::forbidden("Solo il proprietario può esportare il blog."));
    }
    let head: Head = sqlx::query_as(
        "SELECT title, subtitle, description, visibility::text AS visibility, default_locale, extra_locales,
                default_author_display_name, created_at FROM blogs WHERE id = $1",
    )
    .bind(blog.id)
    .fetch_one(&st.db)
    .await?;
    let posts: Vec<PostRow> = sqlx::query_as(
        "SELECT id, title, slug, locale, status::text, content, author_display_name, published_at,
                manual_tags, created_at
         FROM posts WHERE blog_id = $1 ORDER BY created_at, id",
    )
    .bind(blog.id)
    .fetch_all(&st.db)
    .await?;
    let tags: Vec<(uuid::Uuid, String)> = sqlx::query_as(
        "SELECT pt.post_id, t.name FROM post_tags pt JOIN tags t ON t.id = pt.tag_id
         JOIN posts p ON p.id = pt.post_id WHERE p.blog_id = $1 ORDER BY t.name",
    )
    .bind(blog.id)
    .fetch_all(&st.db)
    .await?;
    let media: Vec<(uuid::Uuid, i32, String, String, Vec<String>)> = sqlx::query_as(
        "SELECT m.post_id, m.position, m.url, m.alt_text, m.categories FROM post_media m
         JOIN posts p ON p.id = m.post_id WHERE p.blog_id = $1 ORDER BY m.post_id, m.position",
    )
    .bind(blog.id)
    .fetch_all(&st.db)
    .await?;
    let links: Vec<(uuid::Uuid, i32, String, String)> = sqlx::query_as(
        "SELECT l.post_id, l.position, l.url, l.link_text FROM post_links l
         JOIN posts p ON p.id = l.post_id WHERE p.blog_id = $1 ORDER BY l.post_id, l.position",
    )
    .bind(blog.id)
    .fetch_all(&st.db)
    .await?;

    let mut zip = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut put = |name: &str, data: &[u8]| -> Result<(), ApiError> {
        zip.start_file(name, options).map_err(zip_error)?;
        zip.write_all(data).map_err(zip_error)
    };
    let pretty = |v: &Value| serde_json::to_vec_pretty(v).map_err(ApiError::internal);
    put(
        "blog.json",
        &pretty(&json!({
            "slug": blog.slug, "title": head.title, "subtitle": head.subtitle, "description": head.description,
            "visibility": head.visibility, "default_locale": head.default_locale, "extra_locales": head.extra_locales,
            "default_author_display_name": head.default_author_display_name,
            "created_at": head.created_at.to_rfc3339(), "exported_at": Utc::now().to_rfc3339(),
        }))?,
    )?;
    for (id, title, slug, locale, status, content, author, published_at, manual, created) in &posts {
        let mut all_tags: Vec<String> = manual.clone();
        all_tags.extend(tags.iter().filter(|(p, _)| p == id).map(|(_, t)| t.clone()));
        all_tags.sort();
        all_tags.dedup();
        let mut body = front_matter(&[
            ("title", json!(title)),
            ("slug", json!(slug)),
            ("locale", json!(locale)),
            ("status", json!(status)),
            ("published_at", json!(published_at.map(|d| d.to_rfc3339()))),
            ("author", json!(author)),
            ("tags", json!(all_tags)),
            ("created_at", json!(created.to_rfc3339())),
        ]);
        body.push('\n');
        body.push_str(content.trim_end());
        body.push('\n');
        put(&format!("posts/{}-{}.md", file_part(locale), file_part(slug)), body.as_bytes())?;
    }
    put(
        "media.json",
        &pretty(&Value::Array(
            media
                .into_iter()
                .map(|(p, pos, url, alt, cats)| json!({"post_id": p, "position": pos, "url": url, "alt_text": alt, "categories": cats}))
                .collect(),
        ))?,
    )?;
    put(
        "links.json",
        &pretty(&Value::Array(
            links
                .into_iter()
                .map(|(p, pos, url, text)| json!({"post_id": p, "position": pos, "url": url, "link_text": text}))
                .collect(),
        ))?,
    )?;
    for (name, data) in st.admin_sections.blog_export(st, blog.id).await? {
        put(&name, &data)?;
    }
    let bytes = zip.finish().map_err(zip_error)?.into_inner();
    let disposition = HeaderValue::from_str(&format!("attachment; filename=\"notturni-{}.zip\"", blog.slug))
        .map_err(ApiError::internal)?;
    let mut res = (StatusCode::OK, bytes).into_response();
    res.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/zip"));
    res.headers_mut().insert(header::CONTENT_DISPOSITION, disposition);
    Ok(res)
}
