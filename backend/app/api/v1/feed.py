"""Feed aggregato multi-blog, per la homepage della piattaforma (notturni.eu:
raccolta degli articoli nella lingua dell'utente, stile dev.to — CLAUDE.md #2).

Router separato da posts.py apposta: qui i post attraversano blog diversi
(non uno scoped da slug/id come nel resto dell'API), ed è comunque un modo
per non rischiare ambiguità di routing con GET /posts/{post_id} (un
ipotetico /posts/feed nello stesso router condividerebbe la stessa forma di
path e dipenderebbe dall'ordine di registrazione per essere risolto
correttamente)."""

from datetime import datetime, timezone

from fastapi import APIRouter, Depends
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.v1.posts import PostOut, _post_out
from app.core.database import get_session
from app.models.blog import Blog
from app.models.post import Post, PostStatus

router = APIRouter()

MAX_FEED_LIMIT = 50
DEFAULT_FEED_LIMIT = 20


@router.get("/posts", response_model=list[PostOut])
async def list_feed(
    locale: str | None = None,
    limit: int = DEFAULT_FEED_LIMIT,
    offset: int = 0,
    session: AsyncSession = Depends(get_session),
) -> list[PostOut]:
    """Pubblico, nessuna autenticazione: solo post effettivamente pubblicati
    (pubblicati e con `published_at` raggiunto), dal più recente. `locale`
    filtra per lingua; omesso, ritorna tutte le lingue insieme."""
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

    result = await session.execute(stmt)
    return [_post_out(post, blog) for post, blog in result.all()]
