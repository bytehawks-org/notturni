import uuid

import pytest
from fastapi import HTTPException
from httpx import AsyncClient

from app.domain.rate_limit import enforce_rate_limit


async def test_enforce_rate_limit_blocks_beyond_limit() -> None:
    key = f"test:{uuid.uuid4()}"
    for _ in range(3):
        await enforce_rate_limit(key, limit=3, window_seconds=5, message="troppe richieste")

    with pytest.raises(HTTPException) as exc_info:
        await enforce_rate_limit(key, limit=3, window_seconds=5, message="troppe richieste")
    assert exc_info.value.status_code == 429


async def test_enforce_rate_limit_fails_open_when_redis_unreachable(monkeypatch: pytest.MonkeyPatch) -> None:
    class _BrokenRedis:
        async def incr(self, key: str) -> int:
            raise ConnectionError("redis non raggiungibile")

    monkeypatch.setattr("app.domain.rate_limit.get_redis", lambda: _BrokenRedis())

    # non deve sollevare nulla: un Redis rotto disattiva il limite, non blocca la richiesta
    await enforce_rate_limit(f"test:{uuid.uuid4()}", limit=1, window_seconds=5, message="troppe richieste")


async def test_login_rate_limited_per_email(client: AsyncClient) -> None:
    email = f"ratelimit-{uuid.uuid4()}@example.com"
    payload = {"email": email, "password": "wrong-password"}

    # LOGIN_EMAIL_RATE_LIMIT=5 (app/api/v1/auth.py): le prime richieste falliscono
    # per credenziali errate (401), non ancora per rate limit
    for _ in range(5):
        res = await client.post("/api/v1/auth/login", json=payload)
        assert res.status_code == 401

    blocked = await client.post("/api/v1/auth/login", json=payload)
    assert blocked.status_code == 429
