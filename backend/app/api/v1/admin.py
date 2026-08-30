import uuid
from datetime import datetime

from fastapi import APIRouter, Depends, HTTPException, status
from pydantic import BaseModel
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import require_platform_admin
from app.core.database import get_session
from app.models.user import PlatformRole, User

router = APIRouter()

# Solo un Super Admin può promuovere/retrocedere da e verso questi ruoli:
# un Amministratore non deve poter creare altri amministratori o super admin.
PRIVILEGED_ROLES = {PlatformRole.AMMINISTRATORE, PlatformRole.SUPER_ADMIN}


class AdminUserOut(BaseModel):
    id: uuid.UUID
    username: str
    email: str
    platform_role: PlatformRole
    is_active: bool
    mfa_enabled: bool
    created_at: datetime

    model_config = {"from_attributes": True}


class AdminUserUpdateRequest(BaseModel):
    platform_role: PlatformRole | None = None
    is_active: bool | None = None


@router.get("/users", response_model=list[AdminUserOut])
async def list_users(
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> list[User]:
    result = await session.execute(select(User).order_by(User.created_at))
    return list(result.scalars().all())


@router.patch("/users/{user_id}", response_model=AdminUserOut)
async def update_user(
    user_id: uuid.UUID,
    payload: AdminUserUpdateRequest,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> User:
    target = await session.get(User, user_id)
    if target is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Utente non trovato.")

    if payload.platform_role is not None:
        touches_privileged_tier = (
            payload.platform_role in PRIVILEGED_ROLES or target.platform_role in PRIVILEGED_ROLES
        )
        if touches_privileged_tier and current_user.platform_role != PlatformRole.SUPER_ADMIN:
            raise HTTPException(
                status.HTTP_403_FORBIDDEN,
                "Solo un Super Admin può assegnare o rimuovere ruoli di amministrazione.",
            )
        target.platform_role = payload.platform_role

    if payload.is_active is not None:
        if target.id == current_user.id and not payload.is_active:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, "Non puoi disattivare il tuo stesso account.")
        target.is_active = payload.is_active

    await session.commit()
    await session.refresh(target)
    return target
