"""Ricerca globale sul portale (CLAUDE.md #2, blocco "ricerca globale"):
cerca tra i post di *tutti* i blog pubblici, a differenza della ricerca
per singolo blog (`GET /blogs/{blog_slug}/search`, app/api/v1/posts.py) che
resta ristretta al sottodominio del blog. Router separato dagli altri per lo
stesso motivo di feed.py: attraversa blog diversi, non uno scoped da slug."""

from datetime import datetime, timezone

from fastapi import APIRouter, Depends
from sqlalchemy import or_, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_optional_current_user
from app.api.v1.posts import PostOut, _posts_out
from app.core.database import get_session
from app.domain.authorization import blog_publicly_listable_clause
from app.models.blog import Blog
from app.models.post import Post, PostStatus
from app.models.user import User

router = APIRouter()

MAX_SEARCH_LIMIT = 50
DEFAULT_SEARCH_LIMIT = 20


@router.get("/posts", response_model=list[PostOut])
async def search_posts(
    q: str,
    limit: int = DEFAULT_SEARCH_LIMIT,
    offset: int = 0,
    # accettato per coerenza con feed/blogs (autenticazione opzionale, non
    # ancora usata per personalizzare i risultati) — nessun comportamento
    # diverso oggi tra visitatore anonimo e utente autenticato.
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[PostOut]:
    """Pubblico: cerca `q` in titolo e contenuto tra i post pubblicati di
    ogni blog pubblico, stessi vincoli di visibilità del feed della homepage
    (`GET /feed/posts`). Per la ricerca dei blog stessi per nome, vedi
    `GET /blogs?q=` (già esistente, directory pubblica)."""
    q = q.strip()
    if not q:
        return []
    limit = min(max(limit, 1), MAX_SEARCH_LIMIT)
    offset = max(offset, 0)

    needle = f"%{q}%"
    stmt = (
        select(Post, Blog)
        .join(Blog, Post.blog_id == Blog.id)
        .where(
            Post.status == PostStatus.PUBLISHED,
            Post.published_at <= datetime.now(timezone.utc),
            Post.is_hidden.is_(False),
            blog_publicly_listable_clause(),
            or_(Post.title.ilike(needle), Post.content.ilike(needle)),
        )
        .order_by(Post.published_at.desc())
        .limit(limit)
        .offset(offset)
    )
    result = await session.execute(stmt)
    return await _posts_out(session, [(post, blog) for post, blog in result.all()])
