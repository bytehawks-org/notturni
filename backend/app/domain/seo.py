"""Opt-in per crawler (punto 6 della richiesta): motori di ricerca e IA/LLM
separatamente, a livello di blog e di singolo post.

Un blog escluso non può essere "riaperto" da un override di post: l'override
del post si applica solo se il blog stesso permette il crawling di quel tipo
(vedi effective_search_indexing/effective_ai_crawling). Le liste di
esclusione per robots.txt (GET /api/v1/seo/crawl-directives, vedi
app/api/v1/health.py) sono derivate solo da blog `public` non sospesi e post
pubblicamente visibili (`is_publicly_visible`): un blog `members`/`private`
non è comunque raggiungibile da un crawler anonimo, escluderlo esplicitamente
non aggiungerebbe nulla.
"""

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.domain.authorization import publicly_visible_clause
from app.domain.permalinks import build_permalink
from app.models.blog import Blog, BlogVisibility
from app.models.post import Post

# User-agent dei crawler noti usati per addestrare modelli di IA/LLM o per il
# retrieval in tempo reale di assistenti IA — elenco non esaustivo (aggiornato
# a mano), da estendere quando ne emergono di nuovi. Distinto dagli spider dei
# motori di ricerca tradizionali (Googlebot, Bingbot, ...), sempre ammessi a
# meno di search_indexing_enabled=False.
AI_CRAWLER_USER_AGENTS = [
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
]


def effective_search_indexing(post: Post, blog: Blog) -> bool:
    if not blog.search_indexing_enabled:
        return False
    return blog.search_indexing_enabled if post.search_indexing_enabled is None else post.search_indexing_enabled


def effective_ai_crawling(post: Post, blog: Blog) -> bool:
    if not blog.ai_crawling_enabled:
        return False
    return blog.ai_crawling_enabled if post.ai_crawling_enabled is None else post.ai_crawling_enabled


async def build_crawl_directives(session: AsyncSession) -> dict[str, list[str]]:
    """Percorsi (relativi, es. `/blog/post`) da disallow in robots.txt.

    `ai_disallow` include sempre anche tutto `search_disallow`: un contenuto
    già escluso dai motori di ricerca non ha senso lasciarlo aperto solo ai
    crawler di IA — evita anche di dover replicare `search_disallow` lato
    frontend per ogni user-agent IA dichiarato esplicitamente (un gruppo
    user-agent specifico in robots.txt sostituisce interamente `User-agent: *`
    per quel bot, non lo integra)."""
    search_disallow: list[str] = []
    ai_disallow: list[str] = []

    blogs_result = await session.execute(
        select(Blog).where(Blog.visibility == BlogVisibility.PUBLIC, Blog.is_suspended.is_(False))
    )
    blogs = list(blogs_result.scalars().all())
    blogs_by_id = {blog.id: blog for blog in blogs}

    for blog in blogs:
        path = f"/{blog.slug}"
        if not blog.search_indexing_enabled:
            search_disallow.append(path)
        if not blog.ai_crawling_enabled:
            ai_disallow.append(path)

    if not blogs_by_id:
        return {"search_disallow": [], "ai_disallow": []}

    posts_result = await session.execute(
        select(Post).where(Post.blog_id.in_(blogs_by_id.keys()), publicly_visible_clause())
    )
    for post in posts_result.scalars().all():
        blog = blogs_by_id[post.blog_id]
        if not blog.search_indexing_enabled and not blog.ai_crawling_enabled:
            # già escluso per intero a livello di blog, nessun path aggiuntivo utile
            continue
        permalink = build_permalink(blog.slug, post)
        if not effective_search_indexing(post, blog):
            search_disallow.append(permalink)
        if not effective_ai_crawling(post, blog):
            ai_disallow.append(permalink)

    search_disallow = sorted(set(search_disallow))
    ai_disallow = sorted(set(ai_disallow) | set(search_disallow))
    return {"search_disallow": search_disallow, "ai_disallow": ai_disallow}


async def build_sitemap_entries(session: AsyncSession) -> dict[str, list[dict[str, str]]]:
    """Voci per `sitemap.xml` (`frontend/src/app/sitemap.ts`): solo blog/post
    che i motori di ricerca possono comunque indicizzare
    (`search_indexing_enabled` effettivo) — un contenuto in `search_disallow`
    non ha senso elencarlo anche nella sitemap."""
    blogs_result = await session.execute(
        select(Blog).where(
            Blog.visibility == BlogVisibility.PUBLIC,
            Blog.is_suspended.is_(False),
            Blog.search_indexing_enabled.is_(True),
        )
    )
    blogs = list(blogs_result.scalars().all())
    blogs_by_id = {blog.id: blog for blog in blogs}

    blog_entries = [
        {"slug": blog.slug, "updated_at": blog.updated_at.isoformat()} for blog in blogs
    ]
    if not blogs_by_id:
        return {"blogs": blog_entries, "posts": []}

    posts_result = await session.execute(
        select(Post).where(Post.blog_id.in_(blogs_by_id.keys()), publicly_visible_clause())
    )
    post_entries = []
    for post in posts_result.scalars().all():
        blog = blogs_by_id[post.blog_id]
        if not effective_search_indexing(post, blog):
            continue
        post_entries.append(
            {"permalink": build_permalink(blog.slug, post), "updated_at": post.updated_at.isoformat()}
        )
    return {"blogs": blog_entries, "posts": post_entries}
