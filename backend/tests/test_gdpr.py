from collections.abc import Callable

from httpx import AsyncClient

from tests.conftest import AuthedUser


async def test_export_data_includes_profile_and_content(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user("export-owner")
    await client.post("/api/v1/blogs", json={"slug": "blog-export-1", "title": "x"}, headers=owner.headers)
    await client.post(
        "/api/v1/blogs/blog-export-1/posts",
        json={"slug": "post-export", "title": "Titolo", "content": "corpo del post"},
        headers=owner.headers,
    )
    await client.post(
        "/api/v1/users/me/social-links", json={"label": "Sito", "url": "https://example.com"}, headers=owner.headers
    )

    res = await client.get("/api/v1/users/me/export-data", headers=owner.headers)
    assert res.status_code == 200
    body = res.json()

    assert body["account"]["username"] == "export-owner"
    assert [b["slug"] for b in body["blogs_owned"]] == ["blog-export-1"]
    assert [p["title"] for p in body["posts_authored"]] == ["Titolo"]
    assert body["posts_authored"][0]["content"] == "corpo del post"
    assert [s["label"] for s in body["social_links"]] == ["Sito"]


async def test_export_data_requires_session(client: AsyncClient) -> None:
    res = await client.get("/api/v1/users/me/export-data")
    assert res.status_code in (401, 403)


async def test_delete_account_requires_correct_username_confirmation(
    client: AsyncClient, make_user: Callable
) -> None:
    user: AuthedUser = await make_user("delcheck1")
    res = await client.request(
        "DELETE", "/api/v1/users/me", json={"confirm_username": "nome-sbagliato"}, headers=user.headers
    )
    assert res.status_code == 400

    me = await client.get("/api/v1/auth/me", headers=user.headers)
    assert me.status_code == 200


async def test_delete_account_anonymizes_and_deactivates(
    client: AsyncClient, make_user: Callable
) -> None:
    user: AuthedUser = await make_user("delcheck2")
    await client.post(
        "/api/v1/users/me/social-links", json={"label": "Sito", "url": "https://example.com"}, headers=user.headers
    )

    res = await client.request(
        "DELETE", "/api/v1/users/me", json={"confirm_username": "delcheck2"}, headers=user.headers
    )
    assert res.status_code == 204

    # la sessione (JWT già emesso) smette subito di funzionare
    me = await client.get("/api/v1/auth/me", headers=user.headers)
    assert me.status_code == 401

    # l'account non è più raggiungibile con lo username originale
    profile_res = await client.get("/api/v1/users/delcheck2")
    assert profile_res.status_code == 404

    # non può più fare login
    login_res = await client.post(
        "/api/v1/auth/login", json={"email": user.email, "password": user.password}
    )
    assert login_res.status_code == 401


async def test_deleted_account_content_stays_with_anonymized_author(
    client: AsyncClient, make_user: Callable
) -> None:
    """Blog/post/commenti restano (non un DELETE della riga utente): li vede
    ancora chiunque, ora attribuiti a un'identità anonima."""
    author: AuthedUser = await make_user("delcheck3-author")
    other_owner: AuthedUser = await make_user("delcheck3-owner")

    # blog di proprietà dell'utente che cancellerà l'account
    await client.post("/api/v1/blogs", json={"slug": "blog-delcheck3", "title": "x"}, headers=author.headers)
    post_res = await client.post(
        "/api/v1/blogs/blog-delcheck3/posts",
        json={"slug": "post-delcheck3", "title": "Titolo", "content": "y"},
        headers=author.headers,
    )
    post_id = post_res.json()["id"]
    await client.post(f"/api/v1/posts/{post_id}/publish", headers=author.headers)

    # un commento lasciato dallo stesso utente sul blog di qualcun altro
    await client.post(
        "/api/v1/blogs", json={"slug": "blog-delcheck3-altrui", "title": "x"}, headers=other_owner.headers
    )
    other_post_res = await client.post(
        "/api/v1/blogs/blog-delcheck3-altrui/posts",
        json={"slug": "post-altrui", "title": "Altro", "content": "y"},
        headers=other_owner.headers,
    )
    other_post_id = other_post_res.json()["id"]
    await client.post(f"/api/v1/posts/{other_post_id}/publish", headers=other_owner.headers)
    await client.post(
        f"/api/v1/posts/{other_post_id}/comments", json={"content": "un commento"}, headers=author.headers
    )

    del_res = await client.request(
        "DELETE", "/api/v1/users/me", json={"confirm_username": "delcheck3-author"}, headers=author.headers
    )
    assert del_res.status_code == 204

    # il blog di proprietà resta raggiungibile, ancora con il suo post pubblicato
    blog_res = await client.get("/api/v1/blogs/blog-delcheck3")
    assert blog_res.status_code == 200
    own_post_res = await client.get(f"/api/v1/posts/{post_id}")
    assert own_post_res.status_code == 200
    assert own_post_res.json()["author_display_name"] == "Utente eliminato"

    # il commento lasciato sul blog altrui resta, con autore anonimizzato
    comments_res = await client.get(f"/api/v1/posts/{other_post_id}/comments")
    assert comments_res.status_code == 200
    assert comments_res.json()[0]["author_display_name"] == "Utente eliminato"
