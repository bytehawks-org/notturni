from collections.abc import Callable

import pyotp
import pytest
from httpx import AsyncClient

from app.core.config import settings
from app.api.v1.auth import CSRF_COOKIE_NAME, CSRF_HEADER_NAME
from tests.conftest import AuthedUser


def _csrf_headers(client: AsyncClient) -> dict[str, str]:
    """Il refresh token vive nel cookie httpOnly impostato al login: qui si
    legge solo il cookie CSRF (double-submit, non httpOnly per costruzione)
    per riecheggiarlo come header sulle richieste che modificano stato."""
    token = client.cookies.get(CSRF_COOKIE_NAME)
    assert token is not None
    return {CSRF_HEADER_NAME: token}


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


@pytest.mark.parametrize(
    "username",
    ["Giulia", "gi ulia", "gi@ulia", "-giulia", "giulia-", "gi--ulia", "ab", "x" * 33],
)
async def test_register_rejects_invalid_username_format(client: AsyncClient, username: str) -> None:
    res = await client.post(
        "/api/v1/auth/register",
        json={"username": username, "email": "u@example.com", "password": "Password123!"},
    )
    assert res.status_code == 400


@pytest.mark.parametrize("username", ["giulia", "gi-ulia", "gi_ulia_9", "abc"])
async def test_register_accepts_valid_username_format(client: AsyncClient, username: str) -> None:
    res = await client.post(
        "/api/v1/auth/register",
        json={"username": username, "email": f"{username}@example.com", "password": "Password123!"},
    )
    assert res.status_code == 201, res.text


async def test_login_wrong_password(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user("giulia")
    res = await client.post(
        "/api/v1/auth/login", json={"email": user.email, "password": "password-sbagliata"}
    )
    assert res.status_code == 401


async def test_refresh_rotation_and_reuse_rejected(client: AsyncClient, make_user: Callable) -> None:
    await make_user()
    old_refresh_cookie = client.cookies.get("noct_refresh_token")

    refresh_res = await client.post("/api/v1/auth/refresh", headers=_csrf_headers(client))
    assert refresh_res.status_code == 200
    new_refresh_cookie = client.cookies.get("noct_refresh_token")
    assert new_refresh_cookie != old_refresh_cookie

    # riusare il vecchio refresh token (già rotato) è rifiutato
    client.cookies.set("noct_refresh_token", old_refresh_cookie)
    reuse_res = await client.post("/api/v1/auth/refresh", headers=_csrf_headers(client))
    assert reuse_res.status_code == 401

    # il nuovo refresh (quello attuale) invece funziona ancora
    client.cookies.set("noct_refresh_token", new_refresh_cookie)
    ok_res = await client.post("/api/v1/auth/refresh", headers=_csrf_headers(client))
    assert ok_res.status_code == 200


async def test_refresh_without_csrf_header_rejected(client: AsyncClient, make_user: Callable) -> None:
    await make_user()
    res = await client.post("/api/v1/auth/refresh")
    assert res.status_code == 403


async def test_logout_revokes_session(client: AsyncClient, make_user: Callable) -> None:
    await make_user()
    csrf_headers = _csrf_headers(client)

    logout_res = await client.post("/api/v1/auth/logout", headers=csrf_headers)
    assert logout_res.status_code == 204

    refresh_res = await client.post("/api/v1/auth/refresh", headers=csrf_headers)
    assert refresh_res.status_code in (401, 403)


async def test_totp_mfa_full_flow(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user()

    setup_res = await client.post("/api/v1/auth/mfa/totp/setup", headers=user.headers)
    assert setup_res.status_code == 200
    secret = setup_res.json()["secret"]
    # QR generato interamente lato backend (mai un servizio di terze parti,
    # il secret non deve mai lasciare il backend) come SVG inline
    assert setup_res.json()["qr_code_data_uri"].startswith("data:image/svg+xml;base64,")

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
