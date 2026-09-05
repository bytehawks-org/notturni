"""Inviti a collaborare su un blog (todo/BLOG.md #3): lato invitato
(/received-invitations/...) e lato proprietario (/{slug}/invitations...)."""

import uuid
from datetime import datetime, timezone

from fastapi import Depends, HTTPException, status
from pydantic import BaseModel
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user
from app.api.v1.blogs._common import (
    INVITABLE_ROLES,
    InvitationOut,
    _INVITATION_LOADS,
    _invitation_out,
    _require_blog_owner,
)
from app.api.v1.blogs._router import router
from app.core.database import get_session
from app.domain.authorization import get_membership
from app.models.blog import BlogInvitation, BlogInvitationStatus, BlogMembership, BlogRole
from app.models.user import User


class InvitationCreateRequest(BaseModel):
    username: str
    role: BlogRole


@router.get("/received-invitations", response_model=list[InvitationOut])
async def list_received_invitations(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[InvitationOut]:
    """Inviti a collaborare ricevuti dall'utente corrente, ancora in attesa di
    risposta (todo/BLOG.md #3)."""
    result = await session.execute(
        select(BlogInvitation)
        .where(
            BlogInvitation.invited_user_id == current_user.id,
            BlogInvitation.status == BlogInvitationStatus.PENDING,
        )
        .options(*_INVITATION_LOADS)
        .order_by(BlogInvitation.created_at.desc())
    )
    return [_invitation_out(inv) for inv in result.scalars().all()]


async def _get_received_invitation_or_404(
    session: AsyncSession, invitation_id: uuid.UUID, user: User
) -> BlogInvitation:
    result = await session.execute(
        select(BlogInvitation)
        .where(BlogInvitation.id == invitation_id)
        .options(*_INVITATION_LOADS)
    )
    inv = result.scalar_one_or_none()
    if inv is None or inv.invited_user_id != user.id:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Invito non trovato.")
    if inv.status != BlogInvitationStatus.PENDING:
        raise HTTPException(status.HTTP_409_CONFLICT, "Invito già gestito.")
    return inv


@router.post("/received-invitations/{invitation_id}/accept", response_model=InvitationOut)
async def accept_invitation(
    invitation_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> InvitationOut:
    inv = await _get_received_invitation_or_404(session, invitation_id, current_user)

    existing = await get_membership(session, user_id=current_user.id, blog_id=inv.blog_id)
    if existing is None:
        session.add(
            BlogMembership(user_id=current_user.id, blog_id=inv.blog_id, role=inv.role)
        )
    else:
        # già membro (es. invito duplicato via altra strada): allinea il ruolo.
        existing.role = inv.role
    inv.status = BlogInvitationStatus.ACCEPTED
    inv.responded_at = datetime.now(timezone.utc)
    await session.commit()
    await session.refresh(inv)
    return _invitation_out(inv)


@router.post("/received-invitations/{invitation_id}/decline", response_model=InvitationOut)
async def decline_invitation(
    invitation_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> InvitationOut:
    inv = await _get_received_invitation_or_404(session, invitation_id, current_user)
    inv.status = BlogInvitationStatus.DECLINED
    inv.responded_at = datetime.now(timezone.utc)
    await session.commit()
    await session.refresh(inv)
    return _invitation_out(inv)


@router.get("/{slug}/invitations", response_model=list[InvitationOut])
async def list_blog_invitations(
    slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[InvitationOut]:
    blog = await _require_blog_owner(session, current_user, slug)
    result = await session.execute(
        select(BlogInvitation)
        .where(BlogInvitation.blog_id == blog.id)
        .options(*_INVITATION_LOADS)
        .order_by(BlogInvitation.created_at.desc())
    )
    return [_invitation_out(inv) for inv in result.scalars().all()]


@router.post(
    "/{slug}/invitations", response_model=InvitationOut, status_code=status.HTTP_201_CREATED
)
async def create_blog_invitation(
    slug: str,
    payload: InvitationCreateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> InvitationOut:
    blog = await _require_blog_owner(session, current_user, slug)
    if payload.role not in INVITABLE_ROLES:
        raise HTTPException(
            status.HTTP_400_BAD_REQUEST,
            "Si può invitare solo come co_autore o mediatore.",
        )

    invited = await session.execute(select(User).where(User.username == payload.username))
    invited_user = invited.scalar_one_or_none()
    if invited_user is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Utente non trovato.")
    if invited_user.id == blog.owner_id:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Sei già il proprietario del blog.")

    if await get_membership(session, user_id=invited_user.id, blog_id=blog.id) is not None:
        raise HTTPException(status.HTTP_409_CONFLICT, "L'utente è già un collaboratore.")

    existing = await session.execute(
        select(BlogInvitation).where(
            BlogInvitation.blog_id == blog.id,
            BlogInvitation.invited_user_id == invited_user.id,
        )
    )
    inv = existing.scalar_one_or_none()
    if inv is not None and inv.status == BlogInvitationStatus.PENDING:
        raise HTTPException(status.HTTP_409_CONFLICT, "C'è già un invito in attesa per questo utente.")

    if inv is None:
        inv = BlogInvitation(
            blog_id=blog.id,
            invited_user_id=invited_user.id,
            invited_by_id=current_user.id,
            role=payload.role,
        )
        session.add(inv)
    else:
        # riusa la riga di un invito rifiutato/revocato in precedenza
        inv.role = payload.role
        inv.invited_by_id = current_user.id
        inv.status = BlogInvitationStatus.PENDING
        inv.responded_at = None

    await session.commit()
    result = await session.execute(
        select(BlogInvitation).where(BlogInvitation.id == inv.id).options(*_INVITATION_LOADS)
    )
    return _invitation_out(result.scalar_one())


@router.delete("/{slug}/invitations/{invitation_id}", status_code=status.HTTP_204_NO_CONTENT)
async def revoke_blog_invitation(
    slug: str,
    invitation_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    blog = await _require_blog_owner(session, current_user, slug)
    inv = await session.get(BlogInvitation, invitation_id)
    if inv is None or inv.blog_id != blog.id:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Invito non trovato.")
    if inv.status == BlogInvitationStatus.PENDING:
        inv.status = BlogInvitationStatus.REVOKED
        inv.responded_at = datetime.now(timezone.utc)
        await session.commit()
