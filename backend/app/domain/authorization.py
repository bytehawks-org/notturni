import uuid
from datetime import datetime, timezone

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.blog import Blog, BlogMembership, BlogRole, BlogVisibility
from app.models.post import Post, PostStatus

WRITE_ROLES = {BlogRole.AUTORE, BlogRole.CO_AUTORE}
MODERATE_ROLES = {BlogRole.MEDIATORE}
REVIEW_ROLES = {BlogRole.REVISORE}


async def get_membership(
    session: AsyncSession, *, user_id: uuid.UUID, blog_id: uuid.UUID
) -> BlogMembership | None:
    result = await session.execute(
        select(BlogMembership).where(
            BlogMembership.user_id == user_id, BlogMembership.blog_id == blog_id
        )
    )
    return result.scalar_one_or_none()


async def get_membership_role(
    session: AsyncSession, *, user_id: uuid.UUID, blog_id: uuid.UUID
) -> BlogRole | None:
    result = await session.execute(
        select(BlogMembership.role).where(
            BlogMembership.user_id == user_id, BlogMembership.blog_id == blog_id
        )
    )
    return result.scalar_one_or_none()


async def can_write_posts(session: AsyncSession, *, user_id: uuid.UUID, blog: Blog) -> bool:
    # Sospensione da admin di piattaforma (dashboard/blog): blocca la
    # scrittura anche per il proprietario, finché non viene riattivato.
    if blog.is_suspended:
        return False
    if blog.owner_id == user_id:
        return True
    # todo/BLOG.md #2: un blog privato è un diario — scrive solo il proprietario,
    # a prescindere da eventuali membership (co-autore/mediatore).
    if blog.visibility == BlogVisibility.PRIVATE:
        return False
    role = await get_membership_role(session, user_id=user_id, blog_id=blog.id)
    return role in WRITE_ROLES


async def can_view_blog(
    session: AsyncSession, *, user_id: uuid.UUID | None, blog: Blog
) -> bool:
    """todo/BLOG.md #2. ``PUBLIC``: tutti. ``MEMBERS``: qualsiasi utente
    autenticato sulla piattaforma. ``PRIVATE``: solo il proprietario o chi ha
    una membership sul blog. Un blog sospeso (vedi can_write_posts) è
    irraggiungibile per chiunque, proprietario incluso — la sospensione si
    vede/gestisce solo dal dashboard (dashboard/blog, riservato ad
    Amministratore/Super Admin)."""
    if blog.is_suspended:
        return False
    if blog.visibility == BlogVisibility.PUBLIC:
        return True
    if user_id is None:
        return False
    if blog.visibility == BlogVisibility.MEMBERS:
        return True
    # PRIVATE
    if blog.owner_id == user_id:
        return True
    return await get_membership_role(session, user_id=user_id, blog_id=blog.id) is not None


async def can_moderate_comments(session: AsyncSession, *, user_id: uuid.UUID, blog: Blog) -> bool:
    if blog.owner_id == user_id:
        return True
    role = await get_membership_role(session, user_id=user_id, blog_id=blog.id)
    return role in MODERATE_ROLES


async def can_review_posts(session: AsyncSession, *, user_id: uuid.UUID, blog: Blog) -> bool:
    """Ruolo Revisore (CLAUDE.md #1): approva/rimanda in bozza i post in
    pending_review. Il proprietario può sempre farlo anche senza membership."""
    if blog.owner_id == user_id:
        return True
    role = await get_membership_role(session, user_id=user_id, blog_id=blog.id)
    return role in REVIEW_ROLES


def is_publicly_visible(post: Post) -> bool:
    """Pubblicato E (nessuna pianificazione futura oppure già raggiunta) E non
    nascosto da un admin di piattaforma (`Post.is_hidden`, dashboard/moderazione)."""
    if post.is_hidden:
        return False
    if post.status != PostStatus.PUBLISHED:
        return False
    if post.published_at is None:
        return True
    return post.published_at <= datetime.now(timezone.utc)
