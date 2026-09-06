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


async def test_list_posts_requires_admin(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user()
    res = await client.get("/api/v1/admin/posts", headers=user.headers)
    assert res.status_code == 403


async def test_admin_lists_and_searches_posts(
    client: AsyncClient, make_admin: Callable, make_user: Callable
) -> None:
    admin: AuthedUser = await make_admin()
    author: AuthedUser = await make_user("autore-post-admin")
    await client.post(
        "/api/v1/blogs", json={"slug": "blog-post-admin", "title": "x"}, headers=author.headers
    )
    await client.post(
        "/api/v1/blogs/blog-post-admin/posts",
        json={"slug": "post-cercabile", "title": "Titolo cercabile", "content": "y"},
        headers=author.headers,
    )

    res = await client.get("/api/v1/admin/posts", headers=admin.headers)
    assert res.status_code == 200
    slugs = {p["slug"] for p in res.json()}
    assert "post-cercabile" in slugs
    row = next(p for p in res.json() if p["slug"] == "post-cercabile")
    assert row["blog_slug"] == "blog-post-admin"
    assert row["author_username"] == author.username
    assert row["is_hidden"] is False

    res = await client.get("/api/v1/admin/posts?q=cercabile", headers=admin.headers)
    assert {p["slug"] for p in res.json()} == {"post-cercabile"}

    res = await client.get(f"/api/v1/admin/posts?q={author.username}", headers=admin.headers)
    assert {p["slug"] for p in res.json()} == {"post-cercabile"}


async def test_admin_hides_post_and_blocks_public_access(
    client: AsyncClient, make_admin: Callable, make_user: Callable
) -> None:
    admin: AuthedUser = await make_admin()
    author: AuthedUser = await make_user("autore-post-nascosto")
    await client.post(
        "/api/v1/blogs", json={"slug": "blog-post-nascosto", "title": "x"}, headers=author.headers
    )
    create_res = await client.post(
        "/api/v1/blogs/blog-post-nascosto/posts",
        json={"slug": "post-da-nascondere", "title": "x", "content": "y"},
        headers=author.headers,
    )
    post_id = create_res.json()["id"]
    await client.post(f"/api/v1/posts/{post_id}/publish", headers=author.headers)

    assert (await client.get(f"/api/v1/posts/{post_id}")).status_code == 200

    res = await client.patch(
        f"/api/v1/admin/posts/{post_id}", json={"is_hidden": True}, headers=admin.headers
    )
    assert res.status_code == 200
    assert res.json()["is_hidden"] is True

    res = await client.get(f"/api/v1/posts/{post_id}")
    assert res.status_code == 404

    res = await client.patch(
        f"/api/v1/admin/posts/{post_id}", json={"is_hidden": False}, headers=admin.headers
    )
    assert res.status_code == 200
    assert res.json()["is_hidden"] is False
    assert (await client.get(f"/api/v1/posts/{post_id}")).status_code == 200


async def test_list_all_comments_requires_moderator(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user()
    res = await client.get("/api/v1/admin/comments", headers=user.headers)
    assert res.status_code == 403


async def test_moderator_role_sees_comments_across_blogs(
    client: AsyncClient, make_moderator: Callable, make_user: Callable, turnstile_enabled: None
) -> None:
    """ROADMAP.md §1: il ruolo Moderatore non ha nessuna membership sui blog
    coinvolti, ma deve comunque vedere (ed eventualmente moderare) i
    commenti in attesa su tutta la piattaforma."""
    moderator: AuthedUser = await make_moderator()
    owner_a: AuthedUser = await make_user("owner-admcm-a")
    owner_b: AuthedUser = await make_user("owner-admcm-b")

    for slug, owner in (("blog-admcm-a", owner_a), ("blog-admcm-b", owner_b)):
        await client.post("/api/v1/blogs", json={"slug": slug, "title": "x"}, headers=owner.headers)
        await client.patch(
            f"/api/v1/blogs/{slug}", json={"comments_mode": "everyone"}, headers=owner.headers
        )
        post_res = await client.post(
            f"/api/v1/blogs/{slug}/posts",
            json={"slug": f"post-{slug}", "title": f"Titolo {slug}", "content": "y"},
            headers=owner.headers,
        )
        post_id = post_res.json()["id"]
        await client.post(f"/api/v1/posts/{post_id}/publish", headers=owner.headers)
        await client.post(
            f"/api/v1/posts/{post_id}/comments",
            json={
                "content": f"pending su {slug}",
                "author_display_name": "V",
                "author_email": "v@example.com",
                "captcha_token": "ok",
            },
        )

    # il moderatore non è proprietario né mediatore di nessuno dei due blog,
    # ma il pannello trasversale li mostra comunque entrambi
    res = await client.get("/api/v1/admin/comments", headers=moderator.headers)
    assert res.status_code == 200
    body = res.json()
    assert len(body) == 2
    assert {c["blog_slug"] for c in body} == {"blog-admcm-a", "blog-admcm-b"}
    assert {c["status"] for c in body} == {"pending"}

    # e può approvarli tramite gli stessi endpoint della moderazione per-blog
    comment_id = body[0]["id"]
    approve_res = await client.post(f"/api/v1/comments/{comment_id}/approve", headers=moderator.headers)
    assert approve_res.status_code == 200
    assert approve_res.json()["status"] == "approved"
