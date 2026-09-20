"""GET /api/v1/auth/username-available: controllo pubblico di disponibilità
username, usato dal form di registrazione prima dell'invio."""

from collections.abc import Callable

from httpx import AsyncClient

from tests.conftest import AuthedUser


async def test_reserved_username_is_invalid_format(client: AsyncClient) -> None:
    res = await client.get("/api/v1/auth/username-available", params={"username": "admin"})
    assert res.status_code == 200, res.text
    assert res.json() == {"available": False, "reason": "invalid_format"}


async def test_bad_format_username_is_invalid_format(client: AsyncClient) -> None:
    res = await client.get("/api/v1/auth/username-available", params={"username": "-nope-"})
    assert res.status_code == 200, res.text
    assert res.json()["available"] is False
    assert res.json()["reason"] == "invalid_format"


async def test_existing_username_is_taken(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user("presounico")
    res = await client.get("/api/v1/auth/username-available", params={"username": user.username})
    assert res.status_code == 200, res.text
    assert res.json() == {"available": False, "reason": "taken"}


async def test_free_username_is_available(client: AsyncClient) -> None:
    res = await client.get("/api/v1/auth/username-available", params={"username": "libero-davvero"})
    assert res.status_code == 200, res.text
    assert res.json() == {"available": True, "reason": None}


async def test_username_available_rate_limited(client: AsyncClient) -> None:
    last = None
    for _ in range(31):
        last = await client.get("/api/v1/auth/username-available", params={"username": "libero-davvero"})
        if last.status_code == 429:
            break
    assert last.status_code == 429, last.text
