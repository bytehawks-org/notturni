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


async def test_admin_search_users_by_username_or_email(
    client: AsyncClient, make_admin: Callable, make_user: Callable
) -> None:
    admin: AuthedUser = await make_admin()
    await make_user("cercami")

    res = await client.get("/api/v1/admin/users?q=cerca", headers=admin.headers)
    assert res.status_code == 200
    usernames = {u["username"] for u in res.json()}
    assert usernames == {"cercami"}

    res = await client.get(f"/api/v1/admin/users?q={admin.email}", headers=admin.headers)
    assert {u["username"] for u in res.json()} == {admin.username}


async def test_list_blogs_requires_admin(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user()
    res = await client.get("/api/v1/admin/blogs", headers=user.headers)
    assert res.status_code == 403


async def test_admin_lists_and_searches_blogs(
    client: AsyncClient, make_admin: Callable, make_user: Callable
) -> None:
    admin: AuthedUser = await make_admin()
    owner: AuthedUser = await make_user("proprietario-blog")
    await client.post(
        "/api/v1/blogs", json={"slug": "blog-cercabile", "title": "Titolo cercabile"}, headers=owner.headers
    )

    res = await client.get("/api/v1/admin/blogs", headers=admin.headers)
    assert res.status_code == 200
    slugs = {b["slug"] for b in res.json()}
    assert "blog-cercabile" in slugs
    row = next(b for b in res.json() if b["slug"] == "blog-cercabile")
    assert row["owner_username"] == owner.username
    assert row["is_suspended"] is False

    res = await client.get("/api/v1/admin/blogs?q=cercabile", headers=admin.headers)
    assert {b["slug"] for b in res.json()} == {"blog-cercabile"}

    res = await client.get(f"/api/v1/admin/blogs?q={owner.username}", headers=admin.headers)
    assert {b["slug"] for b in res.json()} == {"blog-cercabile"}


async def test_admin_suspends_blog_and_blocks_public_access(
    client: AsyncClient, make_admin: Callable, make_user: Callable
) -> None:
    admin: AuthedUser = await make_admin()
    owner: AuthedUser = await make_user("proprietario-sospeso")
    create_res = await client.post(
        "/api/v1/blogs", json={"slug": "blog-sospeso", "title": "x"}, headers=owner.headers
    )
    blog_id = create_res.json()["id"]

    assert (await client.get("/api/v1/blogs/blog-sospeso")).status_code == 200

    res = await client.patch(
        f"/api/v1/admin/blogs/{blog_id}", json={"is_suspended": True}, headers=admin.headers
    )
    assert res.status_code == 200
    assert res.json()["is_suspended"] is True

    res = await client.get("/api/v1/blogs/blog-sospeso")
    assert res.status_code == 404

    # irraggiungibile anche per il proprietario stesso, non solo per gli anonimi
    res = await client.get("/api/v1/blogs/blog-sospeso", headers=owner.headers)
    assert res.status_code == 404

    res = await client.patch(
        f"/api/v1/admin/blogs/{blog_id}", json={"is_suspended": False}, headers=admin.headers
    )
    assert res.status_code == 200
    assert res.json()["is_suspended"] is False
    assert (await client.get("/api/v1/blogs/blog-sospeso")).status_code == 200
