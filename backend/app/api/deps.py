import uuid
from dataclasses import dataclass
from datetime import datetime, timezone

from fastapi import Depends, HTTPException, status
from fastapi.security import HTTPAuthorizationCredentials, HTTPBearer
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.core.database import get_session
from app.core.security import decode_access_token
from app.domain.api_tokens import TOKEN_PREFIX, hash_token
from app.models.api_token import ApiToken, ApiTokenOwnerType
from app.models.audit_log import AuditActorType
from app.models.user import PlatformRole, User

bearer_scheme = HTTPBearer()
optional_bearer_scheme = HTTPBearer(auto_error=False)


async def get_current_token(
    credentials: HTTPAuthorizationCredentials = Depends(bearer_scheme),
    session: AsyncSession = Depends(get_session),
) -> ApiToken:
    result = await session.execute(
        select(ApiToken).where(ApiToken.token_hash == hash_token(credentials.credentials))
    )
    token = result.scalar_one_or_none()

    if token is None or token.revoked_at is not None:
        raise HTTPException(status.HTTP_401_UNAUTHORIZED, "Token non valido o revocato.")
    if token.expires_at is not None and token.expires_at < datetime.now(timezone.utc):
        raise HTTPException(status.HTTP_401_UNAUTHORIZED, "Token scaduto.")

    token.last_used_at = datetime.now(timezone.utc)
    await session.commit()
    return token


async def get_current_user(
    credentials: HTTPAuthorizationCredentials = Depends(bearer_scheme),
    session: AsyncSession = Depends(get_session),
) -> User:
    """Autentica una richiesta tramite l'access token JWT emesso al login
    (password o SSO) — distinto da get_current_token, che valida gli API
    token opachi del motore core / accesso diretto degli utenti."""
    try:
        user_id = decode_access_token(credentials.credentials)
    except ValueError as exc:
        raise HTTPException(status.HTTP_401_UNAUTHORIZED, str(exc)) from exc

    user = await session.get(User, user_id)
    if user is None or not user.is_active:
        raise HTTPException(status.HTTP_401_UNAUTHORIZED, "Utente non valido.")
    return user


async def get_optional_current_user(
    credentials: HTTPAuthorizationCredentials | None = Depends(optional_bearer_scheme),
    session: AsyncSession = Depends(get_session),
) -> User | None:
    """Come get_current_user, ma senza errore se manca l'header: usata dove
    l'accesso anonimo è ammesso (es. commenti su blog che li consentono)."""
    if credentials is None:
        return None
    try:
        user_id = decode_access_token(credentials.credentials)
    except ValueError:
        return None
    user = await session.get(User, user_id)
    if user is None or not user.is_active:
        return None
    return user


@dataclass
class TokenActor:
    """Attore normalizzato per gli endpoint self-service di `/api/v1/tokens`:
    owner_type/user_id determinano di chi sono i token letti o creati,
    actor_type/actor_label alimentano l'audit log (CLAUDE.md § audit)."""

    owner_type: ApiTokenOwnerType
    user_id: uuid.UUID | None
    actor_type: AuditActorType
    actor_label: str


async def get_token_actor(
    credentials: HTTPAuthorizationCredentials = Depends(bearer_scheme),
    session: AsyncSession = Depends(get_session),
) -> TokenActor:
    """Accetta sia un ApiToken opaco (`noct_...`, per il bootstrap/rotazione
    machine-to-machine) sia un access token JWT di sessione — indispensabile
    per emettere il *primo* token utente dalla dashboard, dove per definizione
    non esiste ancora nessun ApiToken con cui autenticare la richiesta."""
    if credentials.credentials.startswith(TOKEN_PREFIX):
        token = await get_current_token(credentials, session)
        return TokenActor(
            owner_type=token.owner_type,
            user_id=token.user_id,
            actor_type=(
                AuditActorType.USER_TOKEN
                if token.owner_type == ApiTokenOwnerType.USER
                else AuditActorType.CORE_TOKEN
            ),
            actor_label=token.name,
        )

    user = await get_current_user(credentials, session)
    return TokenActor(
        owner_type=ApiTokenOwnerType.USER,
        user_id=user.id,
        actor_type=AuditActorType.USER,
        actor_label=user.username,
    )


PLATFORM_ADMIN_ROLES = {PlatformRole.SUPER_ADMIN, PlatformRole.AMMINISTRATORE}
# Il ruolo Moderatore (CLAUDE.md #5) vede solo la moderazione commenti
# trasversale (ROADMAP.md §1): non la gestione utenti/blog/post riservata a
# PLATFORM_ADMIN_ROLES.
PLATFORM_MODERATION_ROLES = PLATFORM_ADMIN_ROLES | {PlatformRole.MODERATORE}


async def require_platform_admin(current_user: User = Depends(get_current_user)) -> User:
    """Per gli endpoint riservati alla gestione di piattaforma (es. pagine
    statiche del sito principale): CLAUDE.md #1, ruoli Super Admin/Amministratore."""
    if current_user.platform_role not in PLATFORM_ADMIN_ROLES:
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Richiesto ruolo di amministratore.")
    return current_user


async def require_platform_moderator(current_user: User = Depends(get_current_user)) -> User:
    """Per il pannello di moderazione commenti trasversale (dashboard/
    moderazione): Super Admin/Amministratore, più il ruolo Moderatore,
    finora definito ma senza nessuna capacità reale collegata."""
    if current_user.platform_role not in PLATFORM_MODERATION_ROLES:
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Richiesto ruolo di amministratore o moderatore.")
    return current_user
