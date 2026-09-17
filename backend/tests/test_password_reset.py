"""Flusso "password dimenticata" (blocco sicurezza/hardening alpha)."""

from collections.abc import Callable

import pytest
from httpx import AsyncClient
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.user import User
from app.models.user_session import UserSession
from tests.conftest import AuthedUser


@pytest.fixture
def captured_reset_codes(monkeypatch: pytest.MonkeyPatch) -> list[tuple[str, str]]:
    """Come `captured_emails`/`captured_email_change_codes` ma per il modulo
    dedicato al reset password — bindings diversi, va patchato separatamente."""
    sent: list[tuple[str, str]] = []

    def _fake_publish(email: str, code: str, *, purpose: str = "login") -> None:
        sent.append((email, code))

    monkeypatch.setattr("app.domain.password_reset.publish_email_otp", _fake_publish)
    return sent


async def test_forgot_password_unknown_email_is_silent(
    client: AsyncClient, captured_reset_codes: list[tuple[str, str]]
) -> None:
    res = await client.post("/api/v1/auth/password/forgot", json={"email": "nessuno@example.com"})
    assert res.status_code == 202
    assert captured_reset_codes == []


async def test_reset_password_full_flow_invalidates_sessions(
    client: AsyncClient,
    make_user: Callable,
    captured_reset_codes: list[tuple[str, str]],
    db_session: AsyncSession,
) -> None:
    user: AuthedUser = await make_user("forgot1")

    res = await client.post("/api/v1/auth/password/forgot", json={"email": user.email})
    assert res.status_code == 202
    assert len(captured_reset_codes) == 1
    sent_to, code = captured_reset_codes[0]
    assert sent_to == user.email

    wrong = await client.post(
        "/api/v1/auth/password/reset",
        json={"email": user.email, "code": "000000", "new_password": "nuovapassword123"},
    )
    assert wrong.status_code == 400

    too_short = await client.post(
        "/api/v1/auth/password/reset",
        json={"email": user.email, "code": code, "new_password": "corta"},
    )
    assert too_short.status_code == 400

    ok = await client.post(
        "/api/v1/auth/password/reset",
        json={"email": user.email, "code": code, "new_password": "nuovapassword123"},
    )
    assert ok.status_code == 204

    # il codice è monouso
    reuse = await client.post(
        "/api/v1/auth/password/reset",
        json={"email": user.email, "code": code, "new_password": "altrapassword456"},
    )
    assert reuse.status_code == 400

    old_login = await client.post(
        "/api/v1/auth/login", json={"email": user.email, "password": user.password}
    )
    assert old_login.status_code == 401

    user_row = (await db_session.execute(select(User).where(User.username == user.username))).scalar_one()
    remaining = await db_session.execute(select(UserSession).where(UserSession.user_id == user_row.id))
    assert remaining.scalars().all() == []

    new_login = await client.post(
        "/api/v1/auth/login", json={"email": user.email, "password": "nuovapassword123"}
    )
    assert new_login.status_code == 200
