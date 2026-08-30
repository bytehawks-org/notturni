from collections.abc import Callable

from httpx import AsyncClient

from tests.conftest import AuthedUser


async def test_list_users_requires_admin(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user()
    res = await client.get("/api/v1/admin/users", headers=user.headers)
    assert res.status_code == 403


async def test_admin_lists_users(client: AsyncClient, make_admin: Callable, make_user: Callable) -> None:
    admin: AuthedUser = await make_admin()
    await make_user("altro1")

    res = await client.get("/api/v1/admin/users", headers=admin.headers)
    assert res.status_code == 200
    usernames = {u["username"] for u in res.json()}
    assert usernames == {admin.username, "altro1"}


async def test_admin_cannot_self_deactivate(client: AsyncClient, make_admin: Callable) -> None:
    admin: AuthedUser = await make_admin()
    users_res = await client.get("/api/v1/admin/users", headers=admin.headers)
    self_id = next(u["id"] for u in users_res.json() if u["username"] == admin.username)

    res = await client.patch(
        f"/api/v1/admin/users/{self_id}", json={"is_active": False}, headers=admin.headers
    )
    assert res.status_code == 400


async def test_admin_deactivates_other_user(
    client: AsyncClient, make_admin: Callable, make_user: Callable
) -> None:
    admin: AuthedUser = await make_admin()
    target: AuthedUser = await make_user("bersaglio1")
    users_res = await client.get("/api/v1/admin/users", headers=admin.headers)
    target_id = next(u["id"] for u in users_res.json() if u["username"] == target.username)

    res = await client.patch(
        f"/api/v1/admin/users/{target_id}", json={"is_active": False}, headers=admin.headers
    )
    assert res.status_code == 200
    assert res.json()["is_active"] is False

    # l'utente disattivato non può più fare login
    login_res = await client.post(
        "/api/v1/auth/login", json={"email": target.email, "password": target.password}
    )
    assert login_res.status_code == 401


async def test_only_super_admin_grants_privileged_roles(
    client: AsyncClient, make_admin: Callable, make_user: Callable
) -> None:
    """make_admin promuove ad 'amministratore' (non super_admin): un
    amministratore non deve poter creare altri amministratori/super admin."""
    plain_admin: AuthedUser = await make_admin()
    target: AuthedUser = await make_user("bersaglio2")
    users_res = await client.get("/api/v1/admin/users", headers=plain_admin.headers)
    target_id = next(u["id"] for u in users_res.json() if u["username"] == target.username)

    res = await client.patch(
        f"/api/v1/admin/users/{target_id}",
        json={"platform_role": "super_admin"},
        headers=plain_admin.headers,
    )
    assert res.status_code == 403
