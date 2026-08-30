from collections.abc import Callable

import pyotp
import pytest
from httpx import AsyncClient

from app.core.config import settings
from tests.conftest import AuthedUser


async def test_register_login_me(client: AsyncClient) -> None:
    reg = await client.post(
        "/api/v1/auth/register",
        json={"username": "giulia", "email": "giulia@example.com", "password": "Password123!"},
    )
    assert reg.status_code == 201
    assert reg.json()["mfa_enabled"] is False

    login = await client.post(
        "/api/v1/auth/login", json={"email": "giulia@example.com", "password": "Password123!"}
    )
    assert login.status_code == 200
    access_token = login.json()["access_token"]

    me = await client.get("/api/v1/auth/me", headers={"Authorization": f"Bearer {access_token}"})
    assert me.status_code == 200
    assert me.json()["username"] == "giulia"


async def test_register_duplicate_conflict(client: AsyncClient, make_user: Callable) -> None:
    await make_user("giulia")
    res = await client.post(
        "/api/v1/auth/register",
        json={"username": "giulia", "email": "altra@example.com", "password": "Password123!"},
    )
    assert res.status_code == 409


async def test_register_reserved_username_rejected(client: AsyncClient) -> None:
    res = await client.post(
        "/api/v1/auth/register",
        json={"username": "admin", "email": "admin2@example.com", "password": "Password123!"},
    )
    assert res.status_code == 400


async def test_login_wrong_password(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user("giulia")
    res = await client.post(
        "/api/v1/auth/login", json={"email": user.email, "password": "password-sbagliata"}
    )
    assert res.status_code == 401


async def test_refresh_rotation_and_reuse_rejected(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user()

    refresh_res = await client.post("/api/v1/auth/refresh", json={"refresh_token": user.refresh_token})
    assert refresh_res.status_code == 200
    new_refresh = refresh_res.json()["refresh_token"]
    assert new_refresh != user.refresh_token

    reuse_res = await client.post("/api/v1/auth/refresh", json={"refresh_token": user.refresh_token})
    assert reuse_res.status_code == 401

    # il nuovo refresh invece funziona ancora
    ok_res = await client.post("/api/v1/auth/refresh", json={"refresh_token": new_refresh})
    assert ok_res.status_code == 200


async def test_logout_revokes_session(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user()

    logout_res = await client.post("/api/v1/auth/logout", json={"refresh_token": user.refresh_token})
    assert logout_res.status_code == 204

    refresh_res = await client.post("/api/v1/auth/refresh", json={"refresh_token": user.refresh_token})
    assert refresh_res.status_code == 401


async def test_totp_mfa_full_flow(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user()

    setup_res = await client.post("/api/v1/auth/mfa/totp/setup", headers=user.headers)
    assert setup_res.status_code == 200
    secret = setup_res.json()["secret"]

    valid_code = pyotp.TOTP(secret).now()
    confirm_res = await client.post(
        "/api/v1/auth/mfa/totp/confirm", json={"code": valid_code}, headers=user.headers
    )
    assert confirm_res.status_code == 204

    # login ora richiede il secondo fattore
    login_res = await client.post("/api/v1/auth/login", json={"email": user.email, "password": user.password})
    assert login_res.status_code == 200
    challenge_body = login_res.json()
    assert challenge_body["mfa_required"] is True
    assert challenge_body["method"] == "totp"

    wrong_res = await client.post(
        "/api/v1/auth/mfa/verify", json={"challenge": challenge_body["challenge"], "code": "000000"}
    )
    assert wrong_res.status_code == 401

    verify_res = await client.post(
        "/api/v1/auth/mfa/verify",
        json={"challenge": challenge_body["challenge"], "code": pyotp.TOTP(secret).now()},
    )
    assert verify_res.status_code == 200
    assert "access_token" in verify_res.json()


async def test_email_mfa_full_flow(
    client: AsyncClient, make_user: Callable, captured_emails: list[tuple[str, str]]
) -> None:
    user: AuthedUser = await make_user()

    setup_res = await client.post("/api/v1/auth/mfa/email/setup", headers=user.headers)
    assert setup_res.status_code == 202
    assert len(captured_emails) == 1
    email, code = captured_emails[0]
    assert email == user.email

    confirm_res = await client.post(
        "/api/v1/auth/mfa/email/confirm", json={"code": code}, headers=user.headers
    )
    assert confirm_res.status_code == 204

    login_res = await client.post("/api/v1/auth/login", json={"email": user.email, "password": user.password})
    assert login_res.status_code == 200
    challenge_body = login_res.json()
    assert challenge_body["method"] == "email"
    # il login con MFA email ha già accodato un nuovo codice
    assert len(captured_emails) == 2
    _, login_code = captured_emails[-1]

    verify_res = await client.post(
        "/api/v1/auth/mfa/verify", json={"challenge": challenge_body["challenge"], "code": login_code}
    )
    assert verify_res.status_code == 200


async def test_mfa_disable(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user()

    setup_res = await client.post("/api/v1/auth/mfa/totp/setup", headers=user.headers)
    secret = setup_res.json()["secret"]
    await client.post("/api/v1/auth/mfa/totp/confirm", json={"code": pyotp.TOTP(secret).now()}, headers=user.headers)

    disable_res = await client.post("/api/v1/auth/mfa/disable", headers=user.headers)
    assert disable_res.status_code == 204

    login_res = await client.post("/api/v1/auth/login", json={"email": user.email, "password": user.password})
    assert login_res.status_code == 200
    assert "access_token" in login_res.json()


async def test_solo_mode_first_user_becomes_super_admin_second_blocked(
    client: AsyncClient, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(settings, "deployment_mode", "solo")

    first = await client.post(
        "/api/v1/auth/register",
        json={"username": "solouno", "email": "solouno@example.com", "password": "Password123!"},
    )
    assert first.status_code == 201
    assert first.json()["platform_role"] == "super_admin"

    second = await client.post(
        "/api/v1/auth/register",
        json={"username": "solodue", "email": "solodue@example.com", "password": "Password123!"},
    )
    assert second.status_code == 409
