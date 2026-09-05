from collections.abc import Callable

import pytest
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.core.config import settings
from app.core.security import verify_password
from app.domain.auth import bootstrap_super_admin
from app.models.user import PlatformRole, User
from tests.conftest import AuthedUser


def _set_super_admin_env(
    monkeypatch: pytest.MonkeyPatch,
    *,
    username="bootadmin",
    email="bootadmin@example.com",
    password="Password123!",
) -> None:
    monkeypatch.setattr(settings, "super_admin_username", username)
    monkeypatch.setattr(settings, "super_admin_email", email)
    monkeypatch.setattr(settings, "super_admin_password", password)


async def test_bootstrap_creates_super_admin_when_configured(
    db_session: AsyncSession, monkeypatch: pytest.MonkeyPatch
) -> None:
    _set_super_admin_env(monkeypatch)

    await bootstrap_super_admin(db_session)

    result = await db_session.execute(select(User).where(User.username == "bootadmin"))
    user = result.scalar_one()
    assert user.email == "bootadmin@example.com"
    assert user.platform_role == PlatformRole.SUPER_ADMIN
    assert user.is_active is True
    assert verify_password("Password123!", user.hashed_password)


async def test_bootstrap_noop_when_not_fully_configured(
    db_session: AsyncSession, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(settings, "super_admin_username", "bootadmin")
    monkeypatch.setattr(settings, "super_admin_email", "bootadmin@example.com")
    monkeypatch.setattr(settings, "super_admin_password", None)  # manca solo questa

    await bootstrap_super_admin(db_session)

    result = await db_session.execute(select(User).where(User.username == "bootadmin"))
    assert result.scalar_one_or_none() is None


async def test_bootstrap_idempotent(db_session: AsyncSession, monkeypatch: pytest.MonkeyPatch) -> None:
    _set_super_admin_env(monkeypatch)

    await bootstrap_super_admin(db_session)
    await bootstrap_super_admin(db_session)  # riavvio successivo del backend

    result = await db_session.execute(select(User).where(User.username == "bootadmin"))
    assert len(result.scalars().all()) == 1


async def test_bootstrap_skips_silently_on_username_conflict(
    db_session: AsyncSession, monkeypatch: pytest.MonkeyPatch, make_user: Callable
) -> None:
    """Un account con lo stesso username ma un'altra email (es. creato prima
    di configurare il bootstrap, o rinominato a mano) non deve far fallire
    l'avvio del backend né essere alterato."""
    existing: AuthedUser = await make_user("bootadmin")
    _set_super_admin_env(monkeypatch, email="diverso@example.com")

    await bootstrap_super_admin(db_session)

    result = await db_session.execute(select(User).where(User.username == "bootadmin"))
    users = result.scalars().all()
    assert len(users) == 1
    assert users[0].platform_role != PlatformRole.SUPER_ADMIN
    assert users[0].email == existing.email
