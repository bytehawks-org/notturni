import re
from dataclasses import dataclass

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.domain.usernames import RESERVED_USERNAMES
from app.models.sso_identity import SsoIdentity, SsoProvider
from app.models.user import User

_USERNAME_SANITIZE = re.compile(r"[^a-z0-9_]")


@dataclass(frozen=True)
class ExternalProfile:
    """Dati minimi ricavati dall'userinfo del provider OAuth2/OIDC."""

    provider: SsoProvider
    provider_user_id: str
    email: str


class SsoLinkPending(Exception):
    """L'email del payload SSO corrisponde a un utente esistente con MFA
    attiva: CLAUDE.md #3 richiede la verifica 2FA prima di collegare la nuova
    identità. Il chiamante deve completare l'MFA e poi invocare
    complete_pending_link()."""

    def __init__(self, user: User, profile: ExternalProfile) -> None:
        super().__init__("Verifica 2FA richiesta prima di collegare l'identità SSO.")
        self.user = user
        self.profile = profile


async def _find_identity(session: AsyncSession, profile: ExternalProfile) -> SsoIdentity | None:
    result = await session.execute(
        select(SsoIdentity).where(
            SsoIdentity.provider == profile.provider,
            SsoIdentity.provider_user_id == profile.provider_user_id,
        )
    )
    return result.scalar_one_or_none()


async def _generate_username(session: AsyncSession, email: str) -> str:
    base = _USERNAME_SANITIZE.sub("", email.split("@")[0].lower()) or "utente"
    if base in RESERVED_USERNAMES:
        base = f"{base}_"
    candidate = base
    suffix = 0
    while (await session.execute(select(User).where(User.username == candidate))).scalar_one_or_none():
        suffix += 1
        candidate = f"{base}{suffix}"
    return candidate


async def _link_identity(session: AsyncSession, user: User, profile: ExternalProfile) -> None:
    session.add(
        SsoIdentity(
            user_id=user.id,
            provider=profile.provider,
            provider_user_id=profile.provider_user_id,
            email=profile.email,
        )
    )
    await session.commit()


async def link_or_create_user(session: AsyncSession, profile: ExternalProfile) -> User:
    """Risolve il login SSO in un utente, applicando l'account linking di
    CLAUDE.md #3. Solleva SsoLinkPending se serve prima verificare l'MFA."""
    identity = await _find_identity(session, profile)
    if identity is not None:
        user = await session.get(User, identity.user_id)
        assert user is not None
        return user

    result = await session.execute(select(User).where(User.email == profile.email))
    existing_user = result.scalar_one_or_none()

    if existing_user is not None:
        if existing_user.mfa_enabled:
            raise SsoLinkPending(existing_user, profile)
        await _link_identity(session, existing_user, profile)
        return existing_user

    new_user = User(
        username=await _generate_username(session, profile.email),
        email=profile.email,
        hashed_password=None,
    )
    session.add(new_user)
    await session.flush()
    await _link_identity(session, new_user, profile)
    await session.refresh(new_user)
    return new_user


async def complete_pending_link(session: AsyncSession, user: User, profile: ExternalProfile) -> User:
    """Da chiamare dopo che l'utente ha superato la verifica MFA richiesta da
    un SsoLinkPending sollevato in precedenza."""
    await _link_identity(session, user, profile)
    return user
