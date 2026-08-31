"""Feed aggregato multi-blog, per la homepage della piattaforma (notturni.eu:
raccolta degli articoli nella lingua dell'utente, stile dev.to — CLAUDE.md #2).

Router separato da posts.py apposta: qui i post attraversano blog diversi
(non uno scoped da slug/id come nel resto dell'API), ed è comunque un modo
per non rischiare ambiguità di routing con GET /posts/{post_id} (un
ipotetico /posts/feed nello stesso router condividerebbe la stessa forma di
path e dipenderebbe dall'ordine di registrazione per essere risolto
correttamente)."""

from datetime import datetime, timedelta, timezone

from fastapi import APIRouter, Depends
from pydantic import BaseModel
from sqlalchemy import func, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.v1.posts import PostOut, _post_out
from app.core.database import get_session
from app.models.blog import Blog
from app.models.category import Category
from app.models.post import Post, PostStatus
from app.models.tag import Tag, post_tags

router = APIRouter()

MAX_FEED_LIMIT = 50
DEFAULT_FEED_LIMIT = 20
MAX_TRENDING_LIMIT = 30
MAX_TRENDING_DAYS = 90


@router.get("/posts", response_model=list[PostOut])
async def list_feed(
    locale: str | None = None,
    tag: str | None = None,
    category: str | None = None,
    limit: int = DEFAULT_FEED_LIMIT,
    offset: int = 0,
    session: AsyncSession = Depends(get_session),
) -> list[PostOut]:
    """Pubblico, nessuna autenticazione: solo post effettivamente pubblicati
    (pubblicati e con `published_at` raggiunto), dal più recente. `locale`
    filtra per lingua; `tag` filtra per tag (nome normalizzato, es. "poesia"
    non "#Poesia"); `category` filtra per slug di categoria (la categoria è
    per-blog, quindi blog diversi con una categoria omonima compaiono
    insieme, come già avviene per i tag); omessi, nessun filtro."""
    limit = min(max(limit, 1), MAX_FEED_LIMIT)
    offset = max(offset, 0)

    stmt = (
        select(Post, Blog)
        .join(Blog, Post.blog_id == Blog.id)
        .where(Post.status == PostStatus.PUBLISHED, Post.published_at <= datetime.now(timezone.utc))
        .order_by(Post.published_at.desc())
        .limit(limit)
        .offset(offset)
    )
    if locale is not None:
        stmt = stmt.where(Post.locale == locale)
    if tag is not None:
        stmt = stmt.join(post_tags, Post.id == post_tags.c.post_id).join(
            Tag, Tag.id == post_tags.c.tag_id
        ).where(Tag.name == tag)
    if category is not None:
        stmt = stmt.join(Category, Category.id == Post.category_id).where(Category.slug == category)

    result = await session.execute(stmt)
    return [await _post_out(session, post, blog) for post, blog in result.all()]


class TrendingTagOut(BaseModel):
    tag: str
    post_count: int


@router.get("/trending", response_model=list[TrendingTagOut])
async def list_trending_tags(
    days: int = 7,
    limit: int = 10,
    session: AsyncSession = Depends(get_session),
) -> list[TrendingTagOut]:
    """Tag più usati tra i post pubblicati negli ultimi `days` giorni (default
    7, massimo 90), dal più frequente. Non ci sono ancora contatori di
    like/condivisioni in piattaforma (vedi ROADMAP.md) — questa è l'unica
    base disponibile per una sezione "di tendenza" nella homepage."""
    days = min(max(days, 1), MAX_TRENDING_DAYS)
    limit = min(max(limit, 1), MAX_TRENDING_LIMIT)
    since = datetime.now(timezone.utc) - timedelta(days=days)

    post_count = func.count(post_tags.c.post_id)
    stmt = (
        select(Tag.name, post_count)
        .join(post_tags, Tag.id == post_tags.c.tag_id)
        .join(Post, Post.id == post_tags.c.post_id)
        .where(
            Post.status == PostStatus.PUBLISHED,
            Post.published_at >= since,
            Post.published_at <= datetime.now(timezone.utc),
        )
        .group_by(Tag.name)
        .order_by(post_count.desc())
        .limit(limit)
    )
    result = await session.execute(stmt)
    return [TrendingTagOut(tag=name, post_count=count) for name, count in result.all()]
