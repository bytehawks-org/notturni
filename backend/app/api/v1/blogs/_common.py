"""Schemi e helper condivisi dai sotto-moduli del package `blogs`."""

import uuid
from datetime import datetime

from fastapi import HTTPException, status
from pydantic import BaseModel
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession
from sqlalchemy.orm import selectinload

from app.domain.authorization import can_view_blog, can_write_posts
from app.domain.i18n import DEFAULT_LOCALE
from app.models.blog import (
    Blog,
    BlogInvitation,
    BlogInvitationStatus,
    BlogRole,
    BlogVisibility,
)
from app.models.user import User

# Il todo limita gli inviti a co-autore e mediatore; autore/revisore restano
# assegnabili solo per via diretta a DB, non da questa interfaccia.
INVITABLE_ROLES = {BlogRole.CO_AUTORE, BlogRole.MEDIATORE}


class BlogCreateRequest(BaseModel):
    slug: str
    title: str
    default_locale: str = DEFAULT_LOCALE
    subtitle: str | None = None
    description: str | None = None
    visibility: BlogVisibility = BlogVisibility.PUBLIC
    # CLAUDE.md #4: il frontend pre-compila questo campo con lo username di
    # chi crea il blog (resta modificabile) — vedi _resolve_author_display_name
    # in app/api/v1/posts.py per come si combina con l'alias di membership e
    # con la preferenza di profilo quando è vuoto.
    default_author_display_name: str | None = None


class BlogUpdateRequest(BaseModel):
    title: str | None = None
    # "" azzera (torna a nessun sottotitolo/descrizione), assente lascia
    # invariato — stesso schema di default_author_display_name.
    subtitle: str | None = None
    description: str | None = None
    visibility: BlogVisibility | None = None
    allow_anonymous_comments: bool | None = None
    # todo/EDITOR.md: @menzioni nei post trasformate in link (attive di default).
    mentions_enabled: bool | None = None
    # CLAUDE.md #1: pagine statiche del blog, opt-in e disattive di default.
    static_pages_enabled: bool | None = None
    # "" per tornare al default (username di chi scrive), qualsiasi altro
    # valore lo imposta; assente lascia invariato — stesso schema di
    # Post.cover_image_url in PATCH /posts/{id}.
    default_author_display_name: str | None = None


class BlogOut(BaseModel):
    id: uuid.UUID
    slug: str
    title: str
    subtitle: str | None
    description: str | None
    visibility: BlogVisibility
    custom_domain: str | None
    allow_anonymous_comments: bool
    mentions_enabled: bool
    static_pages_enabled: bool
    default_locale: str
    default_author_display_name: str | None
    # CLAUDE.md #8: presente solo per il proprietario stesso (usato lato
    # frontend per calcolare `isOwner`) — chiunque altro lo riceve a `null`,
    # perché è l'unico campo di Blog che punterebbe direttamente all'id
    # dell'utente reale dietro un blog che può mostrarsi con un alias
    # (Blog.default_author_display_name). Vedi _to_blog_out.
    owner_id: uuid.UUID | None
    created_at: datetime

    model_config = {"from_attributes": True}


class FollowerOut(BaseModel):
    username: str

    model_config = {"from_attributes": True}


class MembershipBlogOut(BaseModel):
    blog: BlogOut
    role: BlogRole
    author_display_name: str | None

    model_config = {"from_attributes": True}


class InvitationOut(BaseModel):
    id: uuid.UUID
    blog_slug: str
    blog_title: str
    role: BlogRole
    status: BlogInvitationStatus
    invited_username: str
    invited_by_username: str
    created_at: datetime
    responded_at: datetime | None

    model_config = {"from_attributes": True}


def _invitation_out(inv: BlogInvitation) -> InvitationOut:
    return InvitationOut(
        id=inv.id,
        blog_slug=inv.blog.slug,
        blog_title=inv.blog.title,
        role=inv.role,
        status=inv.status,
        invited_username=inv.invited_user.username,
        invited_by_username=inv.invited_by.username,
        created_at=inv.created_at,
        responded_at=inv.responded_at,
    )


_INVITATION_LOADS = (
    selectinload(BlogInvitation.blog),
    selectinload(BlogInvitation.invited_user),
    selectinload(BlogInvitation.invited_by),
)


async def _get_blog_or_404(session: AsyncSession, slug: str) -> Blog:
    result = await session.execute(select(Blog).where(Blog.slug == slug))
    blog = result.scalar_one_or_none()
    if blog is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")
    return blog


async def _require_blog_write_access(session: AsyncSession, user: User, blog: Blog) -> None:
    if not await can_write_posts(session, user_id=user.id, blog=blog):
        raise HTTPException(
            status.HTTP_403_FORBIDDEN,
            "Serve essere proprietario del blog o avere ruolo autore/co-autore.",
        )


async def _require_blog_viewable(
    session: AsyncSession, user: User | None, blog: Blog
) -> None:
    """todo/BLOG.md #2: un blog `members`/`private` non deve rivelare nulla di
    sé a chi non può vederlo — 404 come se non esistesse, non 403."""
    if not await can_view_blog(session, user_id=user.id if user else None, blog=blog):
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")


async def _require_blog_owner(session: AsyncSession, user: User, slug: str) -> Blog:
    blog = await _get_blog_or_404(session, slug)
    if blog.owner_id != user.id:
        raise HTTPException(
            status.HTTP_403_FORBIDDEN, "Solo il proprietario può gestire i collaboratori."
        )
    return blog


def _to_blog_out(blog: Blog, viewer: User | None) -> BlogOut:
    """CLAUDE.md #8: `owner_id` è l'unico campo di Blog che punta all'id
    reale di chi lo gestisce — nascosto a chiunque non sia il proprietario
    stesso, per non permettere di correlare un blog che usa un alias
    (Blog.default_author_display_name) con l'identità reale dietro di esso."""
    out = BlogOut.model_validate(blog)
    if viewer is None or viewer.id != blog.owner_id:
        out.owner_id = None
    return out
