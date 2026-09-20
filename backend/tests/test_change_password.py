"""POST /api/v1/users/me/password: cambio password autenticato (password
attuale richiesta, a differenza del reset "password dimenticata")."""

from collections.abc import Callable

from httpx import AsyncClient
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.user import User
from app.models.user_session import UserSession
from tests.conftest import AuthedUser


async def test_wrong_current_password_rejected(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user("pwd-wrong")
    res = await client.post(
        "/api/v1/users/me/password",
        json={"current_password": "NonÈQuesta123!", "new_password": "NuovaPassword123!"},
        headers=user.headers,
    )
    assert res.status_code == 400, res.text


async def test_weak_new_password_rejected(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user("pwd-weak")
    res = await client.post(
        "/api/v1/users/me/password",
        json={"current_password": user.password, "new_password": "corta"},
        headers=user.headers,
    )
    assert res.status_code == 400, res.text


async def test_sso_only_account_without_password_rejected(
    client: AsyncClient, make_user: Callable, db_session: AsyncSession
) -> None:
    user: AuthedUser = await make_user("pwd-sso-only")
    db_user = (await db_session.execute(select(User).where(User.username == user.username))).scalar_one()
    db_user.hashed_password = None
    await db_session.commit()

    res = await client.post(
        "/api/v1/users/me/password",
        json={"current_password": user.password, "new_password": "NuovaPassword123!"},
        headers=user.headers,
    )
    assert res.status_code == 400, res.text


async def test_change_password_success_revokes_sessions_and_allows_new_login(
    client: AsyncClient, make_user: Callable, db_session: AsyncSession
) -> None:
    user: AuthedUser = await make_user("pwd-ok")
    db_user = (await db_session.execute(select(User).where(User.username == user.username))).scalar_one()

    # Il login di make_user ha già creato una UserSession per questo utente:
    # dopo il cambio password deve sparire (revoca totale, non solo il token
    # della richiesta corrente).
    before = (
        await db_session.execute(select(UserSession).where(UserSession.user_id == db_user.id))
    ).scalars().all()
    assert len(before) >= 1

    res = await client.post(
        "/api/v1/users/me/password",
        json={"current_password": user.password, "new_password": "NuovaPassword123!"},
        headers=user.headers,
    )
    assert res.status_code == 200, res.text
    assert res.json() == {"status": "ok"}

    after = (
        await db_session.execute(select(UserSession).where(UserSession.user_id == db_user.id))
    ).scalars().all()
    assert after == []

    old_login = await client.post(
        "/api/v1/auth/login", json={"email": user.email, "password": user.password}
    )
    assert old_login.status_code == 401, old_login.text

    new_login = await client.post(
        "/api/v1/auth/login", json={"email": user.email, "password": "NuovaPassword123!"}
    )
    assert new_login.status_code == 200, new_login.text
