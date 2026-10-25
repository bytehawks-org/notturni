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

// ---- sitemap ---------------------------------------------------------------------------------------------------------

const MAX_SITEMAP_LIMIT: i64 = 5000;
const POST_INDEXABLE: &str = "b.search_indexing_enabled AND COALESCE(p.search_indexing_enabled, true)";

#[derive(FromRow)]
struct EntryRow {
    path: String,
    updated_at: Option<DateTime<Utc>>,
    grp: Option<uuid::Uuid>,
    total: i64,
}

#[derive(FromRow)]
struct AltRow {
    grp: uuid::Uuid,
    locale: String,
    path: String,
}

async fn table_exists(ctx: &RouteCtx, name: &str) -> Result<bool, ApiError> {
    Ok(sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL").bind(name).fetch_one(&ctx.state.db).await?)
}

/// Le traduzioni indicizzabili dei post `groups`, come `{locale, path}` per gruppo.
async fn post_alternates(
    ctx: &RouteCtx,
    groups: &[uuid::Uuid],
    blog: Option<&str>,
) -> Result<std::collections::HashMap<uuid::Uuid, Vec<Value>>, ApiError> {
    let rows: Vec<AltRow> = sqlx::query_as(&format!(
        "SELECT p.translation_group_id AS grp, p.locale, '/' || b.slug || '/' || p.slug AS path
         FROM posts p JOIN blogs b ON b.id = p.blog_id
         WHERE p.translation_group_id = ANY($1) AND {LISTABLE_BLOG} AND {VISIBLE_SQL} AND {POST_INDEXABLE}
           AND ($2::text IS NULL OR b.slug = $2)
         ORDER BY p.locale"
    ))
    .bind(groups)
    .bind(blog)
    .fetch_all(&ctx.state.db)
    .await?;
    let mut map: std::collections::HashMap<uuid::Uuid, Vec<Value>> = std::collections::HashMap::new();
    for r in rows {
        map.entry(r.grp).or_default().push(json!({"locale": r.locale, "path": r.path}));
    }
    // una sola lingua non ha alternative da dichiarare
    map.retain(|_, v| v.len() > 1);
    Ok(map)
}

fn entry(path: &str, updated_at: Option<DateTime<Utc>>, alternates: Vec<Value>) -> Value {
    json!({"path": path, "updated_at": updated_at.map(|d| d.to_rfc3339()), "alternates": alternates})
}

/// Il portale è chiuso agli indici dei motori di ricerca (impostazione del modulo `branding`, se presente).
async fn portal_noindex(ctx: &RouteCtx) -> Result<bool, ApiError> {
    if !table_exists(ctx, "site_branding").await? {
        return Ok(false);
    }
    Ok(sqlx::query_scalar("SELECT COALESCE((SELECT noindex FROM site_branding WHERE id = 1), false)")
        .fetch_one(&ctx.state.db)
        .await?)
}

/// Pagina della sitemap globale. L'ordine è stabile (home, pagine del sito, Spazi, post) così le pagine
/// successive non si spostano tra una richiesta e l'altra.
pub async fn sitemap_page(ctx: RouteCtx) -> Result<Value, ApiError> {
    let int = |name: &str, default: i64, max: i64| -> Result<i64, ApiError> {
        match ctx.query.get(name) {
            Some(v) => v
                .parse::<i64>()
                .map(|n| n.clamp(0, max))
                .map_err(|_| ApiError::bad_request(format!("`{name}` non valido."))),
            None => Ok(default),
        }
    };
    let (offset, limit) = (int("offset", 0, i64::MAX / 2)?, int("limit", MAX_SITEMAP_LIMIT, MAX_SITEMAP_LIMIT)?.max(1));
    let noindex = portal_noindex(&ctx).await?;
    if noindex {
        return Ok(json!({"total": 0, "noindex": true, "entries": []}));
    }
    let pages = if table_exists(&ctx, "platform_pages").await? {
        "SELECT 1 AS ord, '/p/' || slug AS path, max(updated_at) AS updated_at, NULL::uuid AS grp FROM platform_pages
         WHERE is_published GROUP BY slug UNION ALL "
    } else {
        ""
    };
    let rows: Vec<EntryRow> = sqlx::query_as(&format!(
        "WITH entries AS (
           SELECT 0 AS ord, '/' AS path, NULL::timestamptz AS updated_at, NULL::uuid AS grp
           UNION ALL {pages}
           SELECT 2, '/' || b.slug, b.updated_at, NULL::uuid FROM blogs b WHERE {LISTABLE_BLOG} AND b.search_indexing_enabled
           UNION ALL
           SELECT 3, '/' || b.slug || '/' || p.slug, p.updated_at, p.translation_group_id
           FROM posts p JOIN blogs b ON b.id = p.blog_id WHERE {LISTABLE_BLOG} AND {VISIBLE_SQL} AND {POST_INDEXABLE}
         )
         SELECT path, updated_at, grp, count(*) OVER () AS total FROM entries ORDER BY ord, path OFFSET $1 LIMIT $2"
    ))
    .bind(offset)
    .bind(limit)
    .fetch_all(&ctx.state.db)
    .await?;
    let total = rows.first().map_or(0, |r| r.total);
    let groups: Vec<uuid::Uuid> = rows.iter().filter_map(|r| r.grp).collect();
    let alts = post_alternates(&ctx, &groups, None).await?;
    // le traduzioni di una pagina del sito stanno sullo stesso indirizzo, scelte con `?locale=`
    let mut page_alts: std::collections::HashMap<String, Vec<Value>> = std::collections::HashMap::new();
    if pages.is_empty() {
        // nessuna pagina del sito
    } else {
        let slugs: Vec<String> = rows.iter().filter_map(|r| r.path.strip_prefix("/p/").map(str::to_string)).collect();
        let tr: Vec<(String, String)> = sqlx::query_as(
            "SELECT slug, locale FROM platform_pages WHERE is_published AND slug = ANY($1) ORDER BY locale",
        )
        .bind(&slugs)
        .fetch_all(&ctx.state.db)
        .await?;
        for (slug, locale) in tr {
            page_alts
                .entry(slug.clone())
                .or_default()
                .push(json!({"locale": locale, "path": format!("/p/{slug}?locale={locale}")}));
        }
        page_alts.retain(|_, v| v.len() > 1);
    }
    let entries: Vec<Value> = rows
        .iter()
        .map(|r| {
            let alternates = match r.grp {
                Some(g) => alts.get(&g).cloned().unwrap_or_default(),
                None => r.path.strip_prefix("/p/").and_then(|s| page_alts.remove(s)).unwrap_or_default(),
            };
            entry(&r.path, r.updated_at, alternates)
        })
        .collect();
    Ok(json!({"total": total, "noindex": false, "entries": entries}))
}

/// Sitemap di un solo Spazio. Un crawler anonimo non vede Spazi privati, sospesi, in pausa o in cancellazione, né
/// quelli che hanno escluso i motori di ricerca: per tutti loro non esiste (404).
pub async fn space_sitemap(ctx: RouteCtx) -> Result<Value, ApiError> {
    let slug = ctx.path.get("slug").cloned().unwrap_or_default();
    if portal_noindex(&ctx).await? {
        return Err(ApiError::not_found("Spazio non trovato."));
    }
    let blog: Option<(uuid::Uuid, DateTime<Utc>)> = sqlx::query_as(&format!(
        "SELECT b.id, b.updated_at FROM blogs b WHERE b.slug = $1 AND {LISTABLE_BLOG} AND b.search_indexing_enabled"
    ))
    .bind(&slug)
    .fetch_optional(&ctx.state.db)
    .await?;
    let Some((blog_id, updated_at)) = blog else { return Err(ApiError::not_found("Spazio non trovato.")) };
    let posts: Vec<EntryRow> = sqlx::query_as(&format!(
        "SELECT '/' || b.slug || '/' || p.slug AS path, p.updated_at, p.translation_group_id AS grp, 0::bigint AS total
         FROM posts p JOIN blogs b ON b.id = p.blog_id WHERE b.id = $1 AND {VISIBLE_SQL} AND {POST_INDEXABLE}
         ORDER BY p.published_at DESC NULLS LAST, p.slug"
    ))
    .bind(blog_id)
    .fetch_all(&ctx.state.db)
    .await?;
    let groups: Vec<uuid::Uuid> = posts.iter().filter_map(|r| r.grp).collect();
    let alts = post_alternates(&ctx, &groups, Some(&slug)).await?;
    let mut entries = vec![entry(&format!("/{slug}"), Some(updated_at), Vec::new())];
    for r in &posts {
        entries.push(entry(&r.path, r.updated_at, r.grp.and_then(|g| alts.get(&g).cloned()).unwrap_or_default()));
    }
    if table_exists(&ctx, "blog_pages").await? {
        let pages: Vec<(String, DateTime<Utc>)> = sqlx::query_as(
            "SELECT slug, max(updated_at) FROM blog_pages WHERE blog_id = $1 AND is_published GROUP BY slug ORDER BY slug",
        )
        .bind(blog_id)
        .fetch_all(&ctx.state.db)
        .await?;
        entries.extend(pages.into_iter().map(|(s, at)| entry(&format!("/{slug}/pagina/{s}"), Some(at), Vec::new())));
    }
    if table_exists(&ctx, "publications").await? {
        let pubs: Vec<(String, DateTime<Utc>)> = sqlx::query_as(
            "SELECT pu.name, pu.updated_at FROM publications pu WHERE pu.blog_id = $1
               AND EXISTS (SELECT 1 FROM post_publications pp JOIN posts p ON p.id = pp.post_id
                           WHERE pp.publication_id = pu.id AND p.status = 'published' AND p.published_at <= now())
             ORDER BY pu.name",
        )
        .bind(blog_id)
        .fetch_all(&ctx.state.db)
        .await?;
        entries.extend(pubs.into_iter().map(|(n, at)| entry(&format!("/{slug}/pub/{n}"), Some(at), Vec::new())));
    }
    Ok(json!({"slug": slug, "entries": entries}))
}
