"""Collaboratori di un blog: elenco/ruolo/rimozione lato proprietario, e
scelta dell'alias di firma lato collaboratore (todo/BLOG.md #3, #4)."""

import uuid
from datetime import datetime

from fastapi import Depends, HTTPException, status
from pydantic import BaseModel
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user
from app.api.v1.blogs._common import (
    INVITABLE_ROLES,
    BlogOut,
    MembershipBlogOut,
    _get_blog_or_404,
    _require_blog_owner,
)
from app.api.v1.blogs._router import router
from app.core.database import get_session
from app.domain.authorization import get_membership
from app.models.blog import BlogMembership, BlogRole
from app.models.user import User


class MemberOut(BaseModel):
    user_id: uuid.UUID
    username: str
    role: BlogRole
    author_display_name: str | None
    created_at: datetime

    model_config = {"from_attributes": True}


class MemberRoleUpdateRequest(BaseModel):
    role: BlogRole


class MyMembershipUpdateRequest(BaseModel):
    # "" azzera (torna alla precedenza: default del blog → alias profilo →
    # username); assente lascia invariato.
    author_display_name: str | None = None


@router.get("/{slug}/members", response_model=list[MemberOut])
async def list_blog_members(
    slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[MemberOut]:
    blog = await _require_blog_owner(session, current_user, slug)
    result = await session.execute(
        select(BlogMembership, User)
        .join(User, User.id == BlogMembership.user_id)
        .where(BlogMembership.blog_id == blog.id)
        .order_by(User.username)
    )
    return [
        MemberOut(
            user_id=m.user_id,
            username=u.username,
            role=m.role,
            author_display_name=m.author_display_name,
            created_at=m.created_at,
        )
        for m, u in result.all()
    ]


@router.patch("/{slug}/members/{user_id}", response_model=MemberOut)
async def update_blog_member(
    slug: str,
    user_id: uuid.UUID,
    payload: MemberRoleUpdateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> MemberOut:
    blog = await _require_blog_owner(session, current_user, slug)
    if payload.role not in INVITABLE_ROLES:
        raise HTTPException(
            status.HTTP_400_BAD_REQUEST,
            "Ruolo assegnabile solo co_autore o mediatore da questa interfaccia.",
        )
    membership = await get_membership(session, user_id=user_id, blog_id=blog.id)
    if membership is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Collaboratore non trovato.")
    membership.role = payload.role
    await session.commit()
    user = await session.get(User, user_id)
    assert user is not None
    return MemberOut(
        user_id=membership.user_id,
        username=user.username,
        role=membership.role,
        author_display_name=membership.author_display_name,
        created_at=membership.created_at,
    )


@router.delete("/{slug}/members/{user_id}", status_code=status.HTTP_204_NO_CONTENT)
async def remove_blog_member(
    slug: str,
    user_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    blog = await _require_blog_owner(session, current_user, slug)
    membership = await get_membership(session, user_id=user_id, blog_id=blog.id)
    if membership is not None:
        await session.delete(membership)
        await session.commit()


@router.patch("/{slug}/my-membership", response_model=MembershipBlogOut)
async def update_my_membership(
    slug: str,
    payload: MyMembershipUpdateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> MembershipBlogOut:
    """Il collaboratore sceglie l'alias con cui firmare i post su questo blog
    (todo/BLOG.md #4)."""
    blog = await _get_blog_or_404(session, slug)
    membership = await get_membership(session, user_id=current_user.id, blog_id=blog.id)
    if membership is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Non sei un collaboratore di questo blog.")
    if payload.author_display_name is not None:
        membership.author_display_name = payload.author_display_name or None
    await session.commit()
    await session.refresh(membership)
    return MembershipBlogOut(
        blog=BlogOut.model_validate(blog),
        role=membership.role,
        author_display_name=membership.author_display_name,
    )
