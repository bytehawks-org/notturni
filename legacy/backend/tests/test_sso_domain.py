"""Il flow HTTP di SSO (/auth/sso/{provider}/login e /callback) non è
testabile end-to-end senza credenziali OAuth reali di un provider (vedi
backend/API.md). Qui si testa direttamente la logica di account linking, che
è la parte con regole di business reali."""

import pyotp
from sqlalchemy.ext.asyncio import AsyncSession

from app.domain.mfa import generate_totp_secret
from app.domain.sso import ExternalProfile, SsoLinkPending, complete_pending_link, link_or_create_user
from app.models.sso_identity import SsoProvider
from app.models.user import MfaMethod


async def test_new_user_created_from_sso(db_session: AsyncSession) -> None:
    profile = ExternalProfile(provider=SsoProvider.GOOGLE, provider_user_id="g-1", email="nuovo@example.com")
    user = await link_or_create_user(db_session, profile)
    assert user.email == "nuovo@example.com"
    assert user.hashed_password is None


async def test_same_identity_resolves_to_same_user(db_session: AsyncSession) -> None:
    profile = ExternalProfile(provider=SsoProvider.GOOGLE, provider_user_id="g-2", email="due@example.com")
    first = await link_or_create_user(db_session, profile)
    second = await link_or_create_user(db_session, profile)
    assert first.id == second.id


async def test_existing_user_without_mfa_links_immediately(db_session: AsyncSession) -> None:
    from app.domain.auth import register_user

    existing = await register_user(
        db_session, username="senzamfa", email="senzamfa@example.com", password="Password123!"
    )

    profile = ExternalProfile(
        provider=SsoProvider.GITHUB, provider_user_id="gh-1", email="senzamfa@example.com"
    )
    linked = await link_or_create_user(db_session, profile)
    assert linked.id == existing.id


async def test_existing_user_with_mfa_requires_verification(db_session: AsyncSession) -> None:
    from app.domain.auth import register_user

    existing = await register_user(
        db_session, username="conmfa", email="conmfa@example.com", password="Password123!"
    )
    existing.mfa_enabled = True
    existing.mfa_method = MfaMethod.TOTP
    existing.mfa_totp_secret = generate_totp_secret()
    await db_session.commit()

    profile = ExternalProfile(
        provider=SsoProvider.MICROSOFT, provider_user_id="ms-1", email="conmfa@example.com"
    )

    raised = False
    try:
        await link_or_create_user(db_session, profile)
    except SsoLinkPending as pending:
        raised = True
        assert pending.user.id == existing.id
        # simula la verifica MFA superata, poi completa il collegamento
        assert pyotp.TOTP(existing.mfa_totp_secret).verify(pyotp.TOTP(existing.mfa_totp_secret).now())
        completed = await complete_pending_link(db_session, pending.user, pending.profile)
        assert completed.id == existing.id

    assert raised, "doveva sollevare SsoLinkPending per un utente con MFA attiva"

    # ora l'identità risulta collegata: un secondo tentativo la trova subito
    relinked = await link_or_create_user(db_session, profile)
    assert relinked.id == existing.id


async def test_reserved_username_not_generated_from_email(db_session: AsyncSession) -> None:
    profile = ExternalProfile(provider=SsoProvider.GOOGLE, provider_user_id="g-99", email="admin@example.com")
    user = await link_or_create_user(db_session, profile)
    assert user.username != "admin"
