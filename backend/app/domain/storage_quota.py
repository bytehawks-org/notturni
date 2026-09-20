"""Applicazione dello spazio massimo per blog (`platform_config.max_blog_storage_mb`,
impostabile da un Super Admin in `/admin/impostazioni`): controllato prima di
ogni upload di media/cover image di un blog, sommando lo spazio già occupato
(`app/core/storage.py::blog_storage_bytes`, stesso conteggio della card
"Spazio" della panoramica del blog) al peso del nuovo file."""

import asyncio

from fastapi import HTTPException, status
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.core.storage import blog_storage_bytes
from app.models.blog import Blog, BlogMembership
from app.models.platform_config import PlatformConfig


async def assert_blog_storage_quota(session: AsyncSession, *, blog: Blog, config: PlatformConfig, extra_bytes: int) -> None:
    """Solleva 413 se `extra_bytes` in più farebbero superare
    `config.max_blog_storage_mb`. Nessun controllo (no-op) se il limite non è
    impostato (null = nessun limite, comportamento invariato di default)."""
    if config.max_blog_storage_mb is None:
        return
    limit_bytes = config.max_blog_storage_mb * 1024 * 1024
    member_ids = (await session.execute(select(BlogMembership.user_id).where(BlogMembership.blog_id == blog.id))).scalars().all()
    user_ids = [str(blog.owner_id)] + [str(uid) for uid in member_ids if uid != blog.owner_id]
    try:
        current_bytes = await asyncio.to_thread(blog_storage_bytes, user_ids=user_ids, blog_id=str(blog.id))
    except Exception:  # noqa: BLE001 — storage irraggiungibile: non bloccare l'upload per un errore di conteggio
        return
    if current_bytes + extra_bytes > limit_bytes:
        raise HTTPException(
            status.HTTP_413_CONTENT_TOO_LARGE,
            f"Spazio massimo per questo blog superato ({config.max_blog_storage_mb} MB).",
        )
