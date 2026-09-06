from typing import Callable

from httpx import AsyncClient


async def test_tokens_require_auth(client: AsyncClient) -> None:
    res = await client.get("/api/v1/tokens")
    # HTTPBearer senza header -> 401/403 a seconda della versione FastAPI;
    # qui interessa solo che l'accesso sia negato.
    assert res.status_code in (401, 403)


async def test_invalid_token_rejected(client: AsyncClient) -> None:
    res = await client.get("/api/v1/tokens", headers={"Authorization": "Bearer noct_non-esiste"})
    assert res.status_code == 401


async def test_create_list_revoke_token(client: AsyncClient, core_api_token: str) -> None:
    headers = {"Authorization": f"Bearer {core_api_token}"}

    create_res = await client.post("/api/v1/tokens", json={"name": "seconda-integrazione"}, headers=headers)
    assert create_res.status_code == 201
    body = create_res.json()
    assert body["token"].startswith("noct_")
    new_token_id = body["id"]
    new_token_value = body["token"]

    list_res = await client.get("/api/v1/tokens", headers=headers)
    assert list_res.status_code == 200
    names = {t["name"] for t in list_res.json()}
    assert {"test-core-token", "seconda-integrazione"} <= names
    # il valore in chiaro/hash non devono mai comparire nella lista
    assert all("token" not in t and "token_hash" not in t for t in list_res.json())

    revoke_res = await client.delete(f"/api/v1/tokens/{new_token_id}", headers=headers)
    assert revoke_res.status_code == 204

    # il token revocato non deve più funzionare
    reuse_res = await client.get(
        "/api/v1/tokens", headers={"Authorization": f"Bearer {new_token_value}"}
    )
    assert reuse_res.status_code == 401


async def test_new_token_inherits_owner_type(client: AsyncClient, core_api_token: str) -> None:
    headers = {"Authorization": f"Bearer {core_api_token}"}
    res = await client.post("/api/v1/tokens", json={"name": "figlio"}, headers=headers)
    assert res.status_code == 201

    list_res = await client.get("/api/v1/tokens", headers=headers)
    owner_types = {t["owner_type"] for t in list_res.json()}
    assert owner_types == {"core"}


async def test_user_can_create_first_token_via_session(client: AsyncClient, make_user: Callable) -> None:
    """Un utente senza ancora nessun token deve poterne emettere uno dalla
    dashboard usando l'access token JWT di sessione (get_token_actor)."""
    authed = await make_user()

    create_res = await client.post(
        "/api/v1/tokens", json={"name": "dashboard-token"}, headers=authed.headers
    )
    assert create_res.status_code == 201, create_res.text
    body = create_res.json()
    assert body["token"].startswith("noct_")

    list_res = await client.get("/api/v1/tokens", headers=authed.headers)
    assert list_res.status_code == 200
    tokens = list_res.json()
    assert [t["name"] for t in tokens] == ["dashboard-token"]
    assert tokens[0]["owner_type"] == "user"

    revoke_res = await client.delete(f"/api/v1/tokens/{body['id']}", headers=authed.headers)
    assert revoke_res.status_code == 204

    reuse_res = await client.get("/api/v1/tokens", headers={"Authorization": f"Bearer {body['token']}"})
    assert reuse_res.status_code == 401


async def test_user_tokens_are_isolated_between_users(client: AsyncClient, make_user: Callable) -> None:
    alice = await make_user("alice")
    bob = await make_user("bob")

    res = await client.post("/api/v1/tokens", json={"name": "alice-token"}, headers=alice.headers)
    assert res.status_code == 201
    alice_token_id = res.json()["id"]

    list_res = await client.get("/api/v1/tokens", headers=bob.headers)
    assert list_res.json() == []

    revoke_res = await client.delete(f"/api/v1/tokens/{alice_token_id}", headers=bob.headers)
    assert revoke_res.status_code == 403
