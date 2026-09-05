import logging
from datetime import datetime, timedelta, timezone

from sqlalchemy import func, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.core.config import settings
from app.core.security import (
    create_access_token,
    generate_opaque_secret,
    hash_password,
    sha256_hex,
    verify_password,
)
from app.domain.usernames import validate_username
from app.models.user import PlatformRole, User
from app.models.user_session import UserSession

REFRESH_TOKEN_PREFIX = "noct_sess_"

logger = logging.getLogger(__name__)


class AuthError(ValueError):
    """Credenziali non valide o azione di autenticazione non consentita."""


async def register_user(session: AsyncSession, *, username: str, email: str, password: str) -> User:
    validate_username(username)

    existing = await session.execute(
        select(User).where((User.username == username) | (User.email == email))
    )
    if existing.scalar_one_or_none() is not None:
        raise AuthError("Username o email già in uso.")

    user_count = await session.scalar(select(func.count()).select_from(User))
    is_first_user = user_count == 0

    # Modalità "solo" (istanza a singolo proprietario, vedi CLAUDE.md #2): la
    # registrazione si chiude dopo il primo utente, che diventa direttamente
    # Super Admin del proprio sito — nessun intervento manuale sul DB.
    if settings.deployment_mode == "solo" and not is_first_user:
        raise AuthError(
            "Registrazione non disponibile: questa istanza è configurata per un singolo proprietario."
        )
    platform_role = (
        PlatformRole.SUPER_ADMIN
        if settings.deployment_mode == "solo" and is_first_user
        else PlatformRole.UTENTE
    )

    user = User(
        username=username,
        email=email,
        hashed_password=hash_password(password),
        platform_role=platform_role,
    )
    session.add(user)
    await session.commit()
    await session.refresh(user)
    return user


async def authenticate_password(session: AsyncSession, *, email: str, password: str) -> User:
    result = await session.execute(select(User).where(User.email == email))
    user = result.scalar_one_or_none()

    if user is None or user.hashed_password is None or not verify_password(password, user.hashed_password):
        raise AuthError("Email o password non corrette.")
    if not user.is_active:
        raise AuthError("Utente disattivato.")
    return user


async def issue_session(session: AsyncSession, user: User) -> tuple[str, str]:
    """Crea una nuova sessione: ritorna (access_token, refresh_token in chiaro)."""
    refresh_plaintext = f"{REFRESH_TOKEN_PREFIX}{generate_opaque_secret()}"
    user_session = UserSession(
        user_id=user.id,
        refresh_token_hash=sha256_hex(refresh_plaintext),
        expires_at=datetime.now(timezone.utc) + timedelta(days=settings.jwt_refresh_token_ttl_days),
    )
    session.add(user_session)
    await session.commit()
    return create_access_token(user.id), refresh_plaintext


async def rotate_refresh_token(session: AsyncSession, refresh_token: str) -> tuple[str, str]:
    """Verifica il refresh token, lo revoca e ne emette uno nuovo (rotation)."""
    result = await session.execute(
        select(UserSession).where(UserSession.refresh_token_hash == sha256_hex(refresh_token))
    )
    user_session = result.scalar_one_or_none()

    if user_session is None or user_session.revoked_at is not None:
        raise AuthError("Refresh token non valido o revocato.")
    if user_session.expires_at < datetime.now(timezone.utc):
        raise AuthError("Refresh token scaduto.")

    user = await session.get(User, user_session.user_id)
    if user is None or not user.is_active:
        raise AuthError("Utente non valido.")

    user_session.revoked_at = datetime.now(timezone.utc)
    user_session.last_used_at = user_session.revoked_at
    return await issue_session(session, user)


async def revoke_session(session: AsyncSession, refresh_token: str) -> None:
    result = await session.execute(
        select(UserSession).where(UserSession.refresh_token_hash == sha256_hex(refresh_token))
    )
    user_session = result.scalar_one_or_none()
    if user_session is not None and user_session.revoked_at is None:
        user_session.revoked_at = datetime.now(timezone.utc)
        await session.commit()


async def bootstrap_super_admin(session: AsyncSession) -> None:
    """Crea il Super Admin da NOCT_SUPER_ADMIN_USERNAME/EMAIL/PASSWORD se
    configurato, eseguita ad ogni avvio del backend (CLAUDE.md #5: username e
    password specificati alla creazione delle risorse, non più solo via
    auto-promozione in modalità "solo" o UPDATE manuale al DB). Idempotente:
    non fa nulla se una delle tre variabili manca o se username/email
    risultano già in uso — non solleva mai, per non bloccare l'avvio del
    backend su un conflitto di provisioning (es. redeploy con un .env
    cambiato dopo che l'account è già stato creato o rinominato a mano)."""
    if not (
        settings.super_admin_username and settings.super_admin_email and settings.super_admin_password
    ):
        return

    existing = await session.execute(
        select(User).where(
            (User.username == settings.super_admin_username) | (User.email == settings.super_admin_email)
        )
    )
    if existing.scalar_one_or_none() is not None:
        return

    user = User(
        username=settings.super_admin_username,
        email=settings.super_admin_email,
        hashed_password=hash_password(settings.super_admin_password),
        platform_role=PlatformRole.SUPER_ADMIN,
    )
    session.add(user)
    await session.commit()
    logger.info("Super Admin creato da NOCT_SUPER_ADMIN_*: %s", settings.super_admin_username)
