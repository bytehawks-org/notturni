from collections.abc import Callable

import pytest
from httpx import AsyncClient
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.custom_domain import CustomDomain
from app.models.user import User, VerificationTier
from tests.conftest import AuthedUser


@pytest.fixture(autouse=True)
def _no_real_dns(monkeypatch: pytest.MonkeyPatch) -> None:
    """Nessun test qui deve toccare il DNS reale — solo `test_verify_domain_*`
    sotto sovrascrivono di nuovo il monkeypatch per controllare l'esito."""

    async def _default_verify(domain: str, expected_token: str) -> bool:
        return True

    monkeypatch.setattr("app.api.v1.users.custom_domains_domain.verify_domain_dns", _default_verify)


async def test_set_domain_returns_pending_instructions(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user("dominio-set")
    res = await client.post("/api/v1/users/me/domain", json={"domain": "esempio.eu"}, headers=user.headers)
    assert res.status_code == 200, res.text
    body = res.json()
    assert body["domain"] == "esempio.eu"
    assert body["status"] == "pending"
    assert body["txt_record_name"]
    assert body["txt_record_value"]


async def test_pending_claim_does_not_block_other_users(
    client: AsyncClient, make_user: Callable, db_session: AsyncSession
) -> None:
    """Un claim non verificato non deve "prenotare" il dominio indefinitamente
    per un altro account (bug corretto: la query di unicità includeva anche
    righe pending/failed, non solo verified)."""
    first: AuthedUser = await make_user("dominio-primo")
    res1 = await client.post("/api/v1/users/me/domain", json={"domain": "conteso.eu"}, headers=first.headers)
    assert res1.status_code == 200
    assert res1.json()["status"] == "pending"

    second: AuthedUser = await make_user("dominio-secondo")
    res2 = await client.post("/api/v1/users/me/domain", json={"domain": "conteso.eu"}, headers=second.headers)
    assert res2.status_code == 200, res2.text
    assert res2.json()["status"] == "pending"


async def test_verified_claim_blocks_other_users(
    client: AsyncClient, make_user: Callable, db_session: AsyncSession
) -> None:
    first: AuthedUser = await make_user("dominio-verif-primo")
    await client.post("/api/v1/users/me/domain", json={"domain": "verificato.eu"}, headers=first.headers)
    verify_res = await client.post("/api/v1/users/me/domain/verify", headers=first.headers)
    assert verify_res.status_code == 200
    assert verify_res.json()["status"] == "verified"

    second: AuthedUser = await make_user("dominio-verif-secondo")
    res = await client.post("/api/v1/users/me/domain", json={"domain": "verificato.eu"}, headers=second.headers)
    assert res.status_code == 409


async def test_replacing_verified_domain_resets_tier_and_public_handle(
    client: AsyncClient, make_user: Callable, db_session: AsyncSession
) -> None:
    """Sostituire un dominio già verificato con uno nuovo (ancora da
    verificare) non deve lasciare il profilo pubblico a mostrare il vecchio
    dominio/badge bronzo come se fosse ancora attivo."""
    user: AuthedUser = await make_user("dominio-sostituito")
    await client.post("/api/v1/users/me/domain", json={"domain": "vecchio.eu"}, headers=user.headers)
    await client.post("/api/v1/users/me/domain/verify", headers=user.headers)

    profile_before = await client.get("/api/v1/users/me", headers=user.headers)
    assert profile_before.json()["verification_tier"] == "bronze"
    assert profile_before.json()["custom_domain"] == "vecchio.eu"

    replace_res = await client.post("/api/v1/users/me/domain", json={"domain": "nuovo.eu"}, headers=user.headers)
    assert replace_res.status_code == 200
    assert replace_res.json()["status"] == "pending"

    profile_after = await client.get("/api/v1/users/me", headers=user.headers)
    assert profile_after.json()["verification_tier"] == "none"
    assert profile_after.json()["custom_domain"] is None

    public = await client.get("/api/v1/users/vecchio.eu")
    assert public.status_code == 404


async def test_verify_domain_failure_marks_status_failed(
    client: AsyncClient, make_user: Callable, monkeypatch: pytest.MonkeyPatch
) -> None:
    async def _fail(domain: str, expected_token: str) -> bool:
        return False

    monkeypatch.setattr("app.api.v1.users.custom_domains_domain.verify_domain_dns", _fail)

    user: AuthedUser = await make_user("dominio-fallito")
    await client.post("/api/v1/users/me/domain", json={"domain": "irraggiungibile.eu"}, headers=user.headers)
    res = await client.post("/api/v1/users/me/domain/verify", headers=user.headers)
    assert res.status_code == 400


async def test_gdpr_anonymize_clears_custom_domain_and_tier(
    client: AsyncClient, make_user: Callable, db_session: AsyncSession
) -> None:
    user: AuthedUser = await make_user("dominio-gdpr")
    await client.post("/api/v1/users/me/domain", json={"domain": "cancellato.eu"}, headers=user.headers)
    await client.post("/api/v1/users/me/domain/verify", headers=user.headers)

    result = await db_session.execute(select(User).where(User.username == "dominio-gdpr"))
    user_id = result.scalar_one().id

    del_res = await client.request(
        "DELETE", "/api/v1/users/me", json={"confirm_username": "dominio-gdpr"}, headers=user.headers
    )
    assert del_res.status_code == 204, del_res.text

    result = await db_session.execute(select(User).where(User.id == user_id))
    db_user = result.scalar_one()
    assert db_user.verification_tier == VerificationTier.NONE
    assert db_user.verified_domain is None

    domain_rows = (
        await db_session.execute(select(CustomDomain).where(CustomDomain.user_id == db_user.id))
    ).scalars().all()
    assert domain_rows == []
