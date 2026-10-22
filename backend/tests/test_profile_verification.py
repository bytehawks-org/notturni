"""Cambio email verificato, cooldown username, dominio custom verificato via
DNS, badge di verifica, ID fediverse (blocco "profilo utente")."""

from collections.abc import Callable
from datetime import datetime, timedelta, timezone

import pytest
from httpx import AsyncClient
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.custom_domain import CustomDomain, CustomDomainStatus
from app.models.user import User, VerificationTier
from tests.conftest import AuthedUser
from tests.test_platform_config import _super


@pytest.fixture
def captured_email_change_codes(monkeypatch: pytest.MonkeyPatch) -> list[tuple[str, str]]:
    """Come `captured_emails` (tests/conftest.py) ma per il modulo dedicato al
    cambio email — bindings diversi, va patchato separatamente."""
    sent: list[tuple[str, str]] = []

    def _fake_publish(email: str, code: str) -> None:
        sent.append((email, code))

    monkeypatch.setattr("app.domain.email_change.publish_email_otp", _fake_publish)
    return sent


async def test_username_cooldown(client: AsyncClient, make_user: Callable, db_session: AsyncSession) -> None:
    user: AuthedUser = await make_user("cooldown1")

    first = await client.patch(
        "/api/v1/users/me", json={"username": "cooldown1b"}, headers=user.headers
    )
    assert first.status_code == 200, first.text
    assert first.json()["username"] == "cooldown1b"

    second = await client.patch(
        "/api/v1/users/me", json={"username": "cooldown1c"}, headers=user.headers
    )
    assert second.status_code == 409
    assert "5 giorni" in second.json()["detail"]

    # username invariato (stesso valore) non tocca il cooldown
    unchanged = await client.patch(
        "/api/v1/users/me", json={"username": "cooldown1b"}, headers=user.headers
    )
    assert unchanged.status_code == 200

    # backdating: dopo 5 giorni il cambio torna consentito
    result = await db_session.execute(select(User).where(User.username == "cooldown1b"))
    db_user = result.scalar_one()
    db_user.username_changed_at = datetime.now(timezone.utc) - timedelta(days=6)
    await db_session.commit()

    third = await client.patch(
        "/api/v1/users/me", json={"username": "cooldown1d"}, headers=user.headers
    )
    assert third.status_code == 200


async def test_me_endpoint_includes_email_public_profile_does_not(
    client: AsyncClient, make_user: Callable
) -> None:
    user: AuthedUser = await make_user("privacy1")

    me = await client.get("/api/v1/users/me", headers=user.headers)
    assert me.status_code == 200
    body = me.json()
    assert body["email"] == user.email
    assert body["username_changed_at"] is None
    assert body["next_username_change_allowed_at"] is None
    assert body["pending_email_change"] is None
    assert "atproto_did" in body and "activitypub_actor_id" in body

    public = await client.get(f"/api/v1/users/{user.username}")
    assert public.status_code == 200
    assert "email" not in public.json()


async def test_email_change_full_flow(
    client: AsyncClient, make_user: Callable, captured_email_change_codes: list[tuple[str, str]]
) -> None:
    user: AuthedUser = await make_user("emailchange1")
    new_email = "nuova-emailchange1@example.com"

    request_res = await client.post(
        "/api/v1/users/me/email/request", json={"new_email": new_email}, headers=user.headers
    )
    assert request_res.status_code == 202
    assert len(captured_email_change_codes) == 1
    sent_to, old_code = captured_email_change_codes[0]
    assert sent_to == user.email

    me = await client.get("/api/v1/users/me", headers=user.headers)
    assert me.json()["pending_email_change"] == {
        "new_email": new_email,
        "stage": "awaiting_old_confirmation",
    }

    wrong = await client.post(
        "/api/v1/users/me/email/verify-current", json={"code": "000000"}, headers=user.headers
    )
    assert wrong.status_code == 400

    confirm_old = await client.post(
        "/api/v1/users/me/email/verify-current", json={"code": old_code}, headers=user.headers
    )
    assert confirm_old.status_code == 202
    assert len(captured_email_change_codes) == 2
    sent_to_2, new_code = captured_email_change_codes[1]
    assert sent_to_2 == new_email

    confirm_new = await client.post(
        "/api/v1/users/me/email/verify-new", json={"code": new_code}, headers=user.headers
    )
    assert confirm_new.status_code == 200
    assert confirm_new.json()["email"] == new_email

    login_res = await client.post(
        "/api/v1/auth/login", json={"email": new_email, "password": user.password}
    )
    assert login_res.status_code == 200


async def test_email_change_rejects_email_already_in_use(
    client: AsyncClient, make_user: Callable, captured_email_change_codes: list[tuple[str, str]]
) -> None:
    await make_user("existing1")
    user: AuthedUser = await make_user("emailchange2")

    res = await client.post(
        "/api/v1/users/me/email/request",
        json={"new_email": "existing1@example.com"},
        headers=user.headers,
    )
    assert res.status_code == 400


async def test_custom_domain_verified_flow(
    client: AsyncClient, make_user: Callable, monkeypatch: pytest.MonkeyPatch
) -> None:
    user: AuthedUser = await make_user("domainflow1")

    set_res = await client.post(
        "/api/v1/users/me/domain", json={"domain": "esempio.dominio-test.it"}, headers=user.headers
    )
    assert set_res.status_code == 200, set_res.text
    body = set_res.json()
    assert body["status"] == "pending"
    assert body["txt_record_name"] == "_notturni-challenge.esempio.dominio-test.it"

    monkeypatch.setattr("app.domain.custom_domains.verify_domain_dns", _async_false)
    failed = await client.post("/api/v1/users/me/domain/verify", headers=user.headers)
    assert failed.status_code == 400

    monkeypatch.setattr("app.domain.custom_domains.verify_domain_dns", _async_true)
    verified = await client.post("/api/v1/users/me/domain/verify", headers=user.headers)
    assert verified.status_code == 200
    assert verified.json()["status"] == "verified"

    me = await client.get("/api/v1/users/me", headers=user.headers)
    me_body = me.json()
    assert me_body["verification_tier"] == "bronze"
    assert me_body["custom_domain"] == "esempio.dominio-test.it"

    public = await client.get(f"/api/v1/users/{user.username}")
    assert public.json()["custom_domain"] == "esempio.dominio-test.it"
    assert public.json()["verification_tier"] == "bronze"

    delete_res = await client.delete("/api/v1/users/me/domain", headers=user.headers)
    assert delete_res.status_code == 204

    me_after = await client.get("/api/v1/users/me", headers=user.headers)
    assert me_after.json()["verification_tier"] == "none"
    assert me_after.json()["custom_domain"] is None


async def test_custom_domain_rejects_platform_subdomain(
    client: AsyncClient, make_user: Callable
) -> None:
    user: AuthedUser = await make_user("domainflow2")
    res = await client.post(
        "/api/v1/users/me/domain", json={"domain": "blog.notturni.eu"}, headers=user.headers
    )
    assert res.status_code == 400


async def test_custom_domain_unique_across_users(
    client: AsyncClient, make_user: Callable, db_session: AsyncSession
) -> None:
    owner: AuthedUser = await make_user("domainowner1")
    other: AuthedUser = await make_user("domainowner2")

    result = await db_session.execute(select(User).where(User.username == owner.username))
    owner_row = result.scalar_one()
    db_session.add(
        CustomDomain(
            user_id=owner_row.id,
            domain="conteso.test",
            verification_token="x",
            status=CustomDomainStatus.VERIFIED,
        )
    )
    await db_session.commit()

    res = await client.post(
        "/api/v1/users/me/domain", json={"domain": "conteso.test"}, headers=other.headers
    )
    assert res.status_code == 409


async def test_fediverse_ids_stable_across_username_change(
    client: AsyncClient, make_user: Callable
) -> None:
    user: AuthedUser = await make_user("fedistable1")
    before = (await client.get("/api/v1/users/me", headers=user.headers)).json()

    rename = await client.patch(
        "/api/v1/users/me", json={"username": "fedistable1renamed"}, headers=user.headers
    )
    assert rename.status_code == 200

    after = await client.get("/api/v1/users/me", headers=user.headers)
    after_body = after.json()
    assert after_body["atproto_did"] == before["atproto_did"]
    assert after_body["activitypub_actor_id"] == before["activitypub_actor_id"]


async def _async_true(*args, **kwargs) -> bool:
    return True


async def _async_false(*args, **kwargs) -> bool:
    return False


async def test_verification_tier_priority_and_admin_list_cascade(
    client: AsyncClient, make_user: Callable, make_admin: Callable, db_session: AsyncSession
) -> None:
    """app/domain/verification.py: GOLD (entità verificate a mano) > SILVER
    (sostenitori) > BLUE (dominio email fidato) > NONE, ricalcolato per
    *tutti* gli utenti attivi a ogni modifica di uno dei tre elenchi in
    `PATCH /admin/config` (stessa cascata già usata per gli interessi
    rimossi) — non solo al momento della registrazione/cambio email."""
    user: AuthedUser = await make_user("vtuser1")
    root = await _super(make_admin, db_session, "vt-root1")

    async def tier() -> str:
        res = await client.get(f"/api/v1/users/{user.username}")
        return res.json()["verification_tier"]

    assert await tier() == "none"

    # BLUE: dominio dell'email dell'utente (make_user usa sempre
    # "{username}@example.com") aggiunto ai domini fidati — il cambio
    # dell'elenco ricalcola subito, senza bisogno che l'utente rifaccia login.
    blue = await client.patch(
        "/api/v1/admin/config", json={"verification_blue_domains": ["example.com"]}, headers=root.headers
    )
    assert blue.status_code == 200
    assert await tier() == "blue"

    # SILVER prevale su BLUE
    silver = await client.patch(
        "/api/v1/admin/config", json={"verification_silver_identifiers": [user.username]}, headers=root.headers
    )
    assert silver.status_code == 200
    assert await tier() == "silver"

    # GOLD prevale su SILVER e BLUE
    gold = await client.patch(
        "/api/v1/admin/config", json={"verification_gold_identifiers": [user.username.upper()]}, headers=root.headers
    )
    assert gold.status_code == 200
    assert await tier() == "gold"  # match case-insensitive

    # rimosso da GOLD: ricade su SILVER (ancora nell'elenco), non su NONE
    await client.patch("/api/v1/admin/config", json={"verification_gold_identifiers": []}, headers=root.headers)
    assert await tier() == "silver"

    # rimosso anche da SILVER: ricade su BLUE (dominio ancora fidato)
    await client.patch("/api/v1/admin/config", json={"verification_silver_identifiers": []}, headers=root.headers)
    assert await tier() == "blue"

    # rimosso anche il dominio fidato: nessun criterio più soddisfatto
    await client.patch("/api/v1/admin/config", json={"verification_blue_domains": []}, headers=root.headers)
    assert await tier() == "none"


async def test_verification_tier_falls_back_to_bronze_not_none(
    client: AsyncClient, make_user: Callable, make_admin: Callable, db_session: AsyncSession, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Un utente con dominio custom verificato (bronzo) promosso a GOLD e poi
    rimosso dall'elenco deve ricadere sul bronzo che ha ancora diritto ad
    avere, non su "none" a prescindere (app/domain/verification.py)."""
    user: AuthedUser = await make_user("vtbronze1")
    root = await _super(make_admin, db_session, "vt-root2")

    await client.post("/api/v1/users/me/domain", json={"domain": "vtbronze1.test"}, headers=user.headers)
    monkeypatch.setattr("app.domain.custom_domains.verify_domain_dns", _async_true)
    verified = await client.post("/api/v1/users/me/domain/verify", headers=user.headers)
    assert verified.status_code == 200

    me = await client.get("/api/v1/users/me", headers=user.headers)
    assert me.json()["verification_tier"] == "bronze"

    await client.patch(
        "/api/v1/admin/config", json={"verification_gold_identifiers": [user.username]}, headers=root.headers
    )
    me_gold = await client.get("/api/v1/users/me", headers=user.headers)
    assert me_gold.json()["verification_tier"] == "gold"

    await client.patch("/api/v1/admin/config", json={"verification_gold_identifiers": []}, headers=root.headers)
    me_after = await client.get("/api/v1/users/me", headers=user.headers)
    assert me_after.json()["verification_tier"] == "bronze"
