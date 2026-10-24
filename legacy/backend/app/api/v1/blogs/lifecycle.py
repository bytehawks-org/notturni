"""Ciclo di vita del blog (todo/UX_REDESIGN.md B3, mockup 5c "danger zone"):
trasferimento di proprietà, export, cancellazione con tolleranza, ripristino."""

from datetime import datetime, timedelta, timezone

from fastapi import Depends, HTTPException, Request, Response, status
from pydantic import BaseModel
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user
from app.api.v1.blogs._common import BlogOut, _get_blog_or_404, _to_blog_out
from app.api.v1.blogs._router import router
from app.core.database import get_session
from app.core.revalidation import blog_tag, feed_tag, revalidate_frontend
from app.domain import audit
from app.domain.blog_lifecycle import BLOG_DELETE_GRACE_DAYS, build_blog_export_zip
from app.domain.blog_rules import assert_can_create_blog
from app.models.blog import Blog, BlogMembership, BlogRole
from app.models.user import User


class TransferRequest(BaseModel):
    username: str


class DeleteRequest(BaseModel):
    confirm_slug: str


async def _require_owner(session: AsyncSession, user: User, slug: str) -> Blog:
    blog = await _get_blog_or_404(session, slug)
    if blog.owner_id != user.id:
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Solo il proprietario può farlo.")
    return blog


@router.post("/{slug}/transfer", response_model=BlogOut)
async def transfer_ownership(
    slug: str,
    payload: TransferRequest,
    request: Request,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> BlogOut:
    """Passa la proprietà a un coautore attuale (mockup 5c "Transfer
    ownership"): il nuovo proprietario deve avere una membership `co_autore`
    e non aver raggiunto il limite di blog; chi cede resta come coautore."""
    blog = await _require_owner(session, current_user, slug)
    target = (await session.execute(select(User).where(User.username == payload.username))).scalar_one_or_none()
    if target is None or not target.is_active:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Utente non trovato.")
    if target.id == current_user.id:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Sei già il proprietario.")
    membership = (
        await session.execute(
            select(BlogMembership).where(BlogMembership.blog_id == blog.id, BlogMembership.user_id == target.id)
        )
    ).scalar_one_or_none()
    if membership is None or membership.role != BlogRole.CO_AUTORE:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Il nuovo proprietario deve essere un coautore del blog.")
    try:
        await assert_can_create_blog(session, target.id)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, f"Il nuovo proprietario: {exc}") from exc

    old_owner_id = blog.owner_id
    blog.owner_id = target.id
    await session.delete(membership)
    session.add(BlogMembership(user_id=old_owner_id, blog_id=blog.id, role=BlogRole.CO_AUTORE))
    await audit.record(
        session,
        action="blog.ownership_transferred",
        actor=current_user,
        target_type="blog",
        target_id=blog.id,
        blog_id=blog.id,
        request=request,
        payload={"slug": blog.slug, "to": target.username},
    )
    await session.commit()
    await session.refresh(blog)
    return _to_blog_out(blog, current_user)


@router.get("/{slug}/export")
async def export_blog(
    slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> Response:
    """ZIP con tutto il contenuto del blog (mockup 5c "Export everything"),
    generato al volo — vedi app/domain/blog_lifecycle.py per il formato.
    Solo il proprietario. Le immagini non sono incluse come binari (restano
    raggiungibili agli URL elencati in media.json)."""
    blog = await _require_owner(session, current_user, slug)
    content = await build_blog_export_zip(session, blog)
    return Response(
        content=content,
        media_type="application/zip",
        headers={"Content-Disposition": f'attachment; filename="notturni-{blog.slug}.zip"'},
    )


@router.delete("/{slug}", response_model=BlogOut)
async def delete_blog(
    slug: str,
    payload: DeleteRequest,
    request: Request,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> BlogOut:
    """Cancellazione con tolleranza (mockup 5c/3d): richiede di ridigitare lo
    slug; imposta `deleted_at`, il blog sparisce dalle pagine pubbliche e dai
    feed ma resta ripristinabile per BLOG_DELETE_GRACE_DAYS giorni, poi il
    worker di manutenzione lo elimina davvero (contenuti e storage)."""
    blog = await _require_owner(session, current_user, slug)
    if payload.confirm_slug.strip() != blog.slug:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Lo slug digitato non corrisponde.")
    if blog.deleted_at is None:
        blog.deleted_at = datetime.now(timezone.utc)
        await audit.record(
            session,
            action="blog.deleted",
            actor=current_user,
            target_type="blog",
            target_id=blog.id,
            blog_id=blog.id,
            request=request,
            payload={"slug": blog.slug, "purge_after": (blog.deleted_at + timedelta(days=BLOG_DELETE_GRACE_DAYS)).isoformat()},
        )
        await session.commit()
        await session.refresh(blog)
        await revalidate_frontend([blog_tag(blog.slug), feed_tag()])
    return _to_blog_out(blog, current_user)


@router.post("/{slug}/restore", response_model=BlogOut)
async def restore_blog(
    slug: str,
    request: Request,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> BlogOut:
    """Annulla una cancellazione entro il periodo di tolleranza."""
    blog = await _require_owner(session, current_user, slug)
    if blog.deleted_at is not None:
        blog.deleted_at = None
        await audit.record(
            session, action="blog.restored", actor=current_user, target_type="blog", target_id=blog.id, blog_id=blog.id, request=request, payload={"slug": blog.slug}
        )
        await session.commit()
        await session.refresh(blog)
        await revalidate_frontend([blog_tag(blog.slug), feed_tag()])
    return _to_blog_out(blog, current_user)
