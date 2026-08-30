import uuid
from datetime import datetime, timezone

from fastapi import APIRouter, Depends, HTTPException, status
from pydantic import BaseModel
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_token
from app.core.database import get_session
from app.domain.api_tokens import generate_api_token
from app.models.api_token import ApiToken, ApiTokenOwnerType

router = APIRouter()


class TokenCreateRequest(BaseModel):
    name: str


class TokenCreateResponse(BaseModel):
    id: uuid.UUID
    name: str
    token: str
    token_prefix: str


class TokenOut(BaseModel):
    id: uuid.UUID
    name: str
    token_prefix: str
    owner_type: ApiTokenOwnerType
    created_at: datetime
    last_used_at: datetime | None
    expires_at: datetime | None
    revoked_at: datetime | None

    model_config = {"from_attributes": True}


@router.post("", response_model=TokenCreateResponse, status_code=status.HTTP_201_CREATED)
async def create_token(
    payload: TokenCreateRequest,
    current: ApiToken = Depends(get_current_token),
    session: AsyncSession = Depends(get_session),
) -> TokenCreateResponse:
    """Emette un nuovo token con lo stesso owner_type (e, se applicabile, lo
    stesso utente) del token usato per l'autenticazione della richiesta."""
    plaintext, prefix, token_hash = generate_api_token()
    new_token = ApiToken(
        name=payload.name,
        owner_type=current.owner_type,
        user_id=current.user_id,
        token_prefix=prefix,
        token_hash=token_hash,
    )
    session.add(new_token)
    await session.commit()
    await session.refresh(new_token)
    return TokenCreateResponse(id=new_token.id, name=new_token.name, token=plaintext, token_prefix=prefix)


@router.get("", response_model=list[TokenOut])
async def list_tokens(
    current: ApiToken = Depends(get_current_token),
    session: AsyncSession = Depends(get_session),
) -> list[ApiToken]:
    stmt = select(ApiToken).where(ApiToken.owner_type == current.owner_type)
    if current.owner_type == ApiTokenOwnerType.USER:
        stmt = stmt.where(ApiToken.user_id == current.user_id)
    result = await session.execute(stmt)
    return list(result.scalars().all())


@router.delete("/{token_id}", status_code=status.HTTP_204_NO_CONTENT)
async def revoke_token(
    token_id: uuid.UUID,
    current: ApiToken = Depends(get_current_token),
    session: AsyncSession = Depends(get_session),
) -> None:
    result = await session.execute(select(ApiToken).where(ApiToken.id == token_id))
    target = result.scalar_one_or_none()

    if target is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Token non trovato.")

    is_own_token = target.owner_type == current.owner_type and (
        current.owner_type == ApiTokenOwnerType.CORE or target.user_id == current.user_id
    )
    if not is_own_token:
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Non è possibile revocare questo token.")

    target.revoked_at = datetime.now(timezone.utc)
    await session.commit()
