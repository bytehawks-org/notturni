"""Helper condiviso per le viste "tutti i miei blog" (todo/UX_REDESIGN.md):
elenco/media/pubblicazioni/link/bibliografia aggregati su tutti i blog di cui
l'utente è proprietario o collaboratore, non uno scoped da slug come il resto
dell'API blog. Nome del file scelto per non collidere con il package
`app/api/v1/blogs/`."""

import uuid

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.blog import Blog, BlogMembership


async def my_blog_ids(session: AsyncSession, user_id: uuid.UUID) -> list[uuid.UUID]:
    """Id dei blog di proprietà o su cui l'utente ha una membership attiva,
    esclusi quelli in cancellazione (`Blog.deleted_at`, vedi
    app/domain/blog_lifecycle.py) — le viste aggregate non devono mostrare
    contenuti di un blog che il proprietario ha già mandato in cancellazione."""
    owned = select(Blog.id).where(Blog.owner_id == user_id, Blog.deleted_at.is_(None))
    member_of = (
        select(BlogMembership.blog_id)
        .join(Blog, Blog.id == BlogMembership.blog_id)
        .where(BlogMembership.user_id == user_id, Blog.deleted_at.is_(None))
    )
    result = await session.execute(owned.union(member_of))
    return list(result.scalars().all())
