//! Opt-in dei crawler (motori di ricerca e IA/LLM separatamente, a livello di blog e di post) ed
//! elenchi per `robots.txt` e `sitemap.xml`.
//!
//! Un blog escluso non può essere «riaperto» dall'override di un post. Gli elenchi derivano solo da
//! blog pubblici non sospesi né in pausa né cancellati e da post pubblicamente visibili: un blog
//! `members` o `private` non è comunque raggiungibile da un crawler anonimo.

use crate::VISIBLE_SQL;
use chrono::{DateTime, Utc};
use noct_core::{ApiError, RouteCtx};
use serde_json::{Value, json};
use sqlx::FromRow;

/// User-agent dei crawler noti usati per addestrare modelli o per il retrieval in tempo reale di
/// assistenti IA. Elenco non esaustivo, da estendere a mano; gli spider dei motori di ricerca
/// tradizionali restano ammessi salvo `search_indexing_enabled = false`.
pub const AI_CRAWLER_USER_AGENTS: &[&str] = &[
    "GPTBot",
    "ChatGPT-User",
    "OAI-SearchBot",
    "CCBot",
    "Google-Extended",
    "anthropic-ai",
    "ClaudeBot",
    "Claude-Web",
    "PerplexityBot",
    "Bytespider",
    "Applebot-Extended",
    "Amazonbot",
    "Diffbot",
    "cohere-ai",
    "FacebookBot",
];

const LISTABLE_BLOG: &str =
    "b.visibility = 'public' AND NOT b.is_suspended AND NOT b.is_paused AND b.deleted_at IS NULL";

/// Il blog vieta il tipo di crawling → il post non può riaprirlo; altrimenti decide il post, se ha un
/// override, e in sua assenza il blog.
pub fn effective(blog: bool, post: Option<bool>) -> bool {
    blog && post.unwrap_or(blog)
}

#[derive(FromRow)]
struct PostRow {
    blog_slug: String,
    slug: String,
    blog_search: bool,
    blog_ai: bool,
    post_search: Option<bool>,
    post_ai: Option<bool>,
    updated_at: DateTime<Utc>,
}

async fn visible_posts(ctx: &RouteCtx) -> Result<Vec<PostRow>, ApiError> {
    Ok(sqlx::query_as(&format!(
        "SELECT b.slug AS blog_slug, p.slug, b.search_indexing_enabled AS blog_search, b.ai_crawling_enabled AS blog_ai,
                p.search_indexing_enabled AS post_search, p.ai_crawling_enabled AS post_ai, p.updated_at
         FROM posts p JOIN blogs b ON b.id = p.blog_id WHERE {LISTABLE_BLOG} AND {VISIBLE_SQL}
         ORDER BY b.slug, p.slug, p.locale"
    ))
    .fetch_all(&ctx.state.db)
    .await?)
}

pub async fn crawl_directives(ctx: RouteCtx) -> Result<Value, ApiError> {
    let blogs: Vec<(String, bool, bool)> = sqlx::query_as(&format!(
        "SELECT b.slug, b.search_indexing_enabled, b.ai_crawling_enabled FROM blogs b WHERE {LISTABLE_BLOG} ORDER BY b.slug"
    ))
    .fetch_all(&ctx.state.db)
    .await?;
    let (mut search, mut ai) = (Vec::new(), Vec::new());
    for (slug, s, a) in &blogs {
        if !s {
            search.push(format!("/{slug}"));
        }
        if !a {
            ai.push(format!("/{slug}"));
        }
    }
    for p in visible_posts(&ctx).await? {
        // già escluso per intero a livello di blog: nessun percorso in più
        if !p.blog_search && !p.blog_ai {
            continue;
        }
        let path = format!("/{}/{}", p.blog_slug, p.slug);
        if !effective(p.blog_search, p.post_search) {
            search.push(path.clone());
        }
        if !effective(p.blog_ai, p.post_ai) {
            ai.push(path);
        }
    }
    search.sort();
    search.dedup();
    // un contenuto già escluso dai motori di ricerca non ha senso lasciarlo aperto ai soli crawler di IA
    ai.extend(search.iter().cloned());
    ai.sort();
    ai.dedup();
    Ok(json!({"search_disallow": search, "ai_disallow": ai, "ai_user_agents": AI_CRAWLER_USER_AGENTS}))
}

pub async fn sitemap_entries(ctx: RouteCtx) -> Result<Value, ApiError> {
    let blogs: Vec<(String, DateTime<Utc>)> = sqlx::query_as(&format!(
        "SELECT b.slug, b.updated_at FROM blogs b WHERE {LISTABLE_BLOG} AND b.search_indexing_enabled ORDER BY b.slug"
    ))
    .fetch_all(&ctx.state.db)
    .await?;
    let posts: Vec<Value> = visible_posts(&ctx)
        .await?
        .into_iter()
        .filter(|p| p.blog_search && effective(p.blog_search, p.post_search))
        .map(|p| json!({"permalink": format!("/{}/{}", p.blog_slug, p.slug), "updated_at": p.updated_at.to_rfc3339()}))
        .collect();
    Ok(json!({
        "blogs": blogs.into_iter().map(|(slug, at)| json!({"slug": slug, "updated_at": at.to_rfc3339()})).collect::<Vec<_>>(),
        "posts": posts,
    }))
}
