"""Configurazione di presentazione del blog (palette/tipografia/layout)."""

from typing import Any

from fastapi import Depends, HTTPException, status
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession
from sqlalchemy.orm import selectinload

from app.api.deps import get_current_user, get_optional_current_user
from app.api.v1.blogs._common import _require_blog_viewable
from app.api.v1.blogs._router import router
from app.core.database import get_session
from app.core.revalidation import blog_tag, revalidate_frontend
from app.domain.blog_config import DEFAULT_BLOG_CONFIG, validate_blog_config
from app.models.blog import Blog
from app.models.blog_config import BlogConfig
from app.models.user import User


@router.get("/{slug}/config")
async def get_blog_config(
    slug: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> dict[str, Any]:
    """Configurazione di presentazione (palette/tipografia/layout). Serve a
    renderizzare la pagina pubblica del blog; segue la visibilità del blog
    (todo/BLOG.md #2). Se il proprietario non ha ancora salvato nulla,
    ritorna il default della piattaforma."""
    result = await session.execute(
        select(Blog).where(Blog.slug == slug).options(selectinload(Blog.config))
    )
    blog = result.scalar_one_or_none()
    if blog is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")
    await _require_blog_viewable(session, current_user, blog)
    return blog.config.config if blog.config is not None else DEFAULT_BLOG_CONFIG


@router.put("/{slug}/config")
async def update_blog_config(
    slug: str,
    payload: dict[str, Any],
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> dict[str, Any]:
    result = await session.execute(
        select(Blog).where(Blog.slug == slug).options(selectinload(Blog.config))
    )
    blog = result.scalar_one_or_none()
    if blog is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")
    if blog.owner_id != current_user.id:
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Solo il proprietario può modificare la configurazione.")

    try:
        validate_blog_config(payload)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    if blog.config is None:
        blog.config = BlogConfig(blog_id=blog.id, config=payload)
        session.add(blog.config)
    else:
        blog.config.config = payload

    await session.commit()
    # colori/tipografia/presentazione: si riflettono su tutte le pagine
    # pubbliche del blog.
    await revalidate_frontend([blog_tag(slug)])
    return payload
