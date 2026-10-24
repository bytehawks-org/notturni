from collections.abc import Callable

from httpx import AsyncClient

from tests.conftest import AuthedUser


async def _published_post(client: AsyncClient, owner: AuthedUser, blog_slug: str) -> str:
    await client.post("/api/v1/blogs", json={"slug": blog_slug, "title": "x"}, headers=owner.headers)
    post_res = await client.post(
        f"/api/v1/blogs/{blog_slug}/posts",
        json={"slug": "post-commenti", "title": "x", "content": "y"},
        headers=owner.headers,
    )
    post_id = post_res.json()["id"]
    await client.post(f"/api/v1/posts/{post_id}/publish", headers=owner.headers)
    return post_id


async def test_registered_comment_auto_approved(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner-c1")
    commenter: AuthedUser = await make_user("commenter1")
    post_id = await _published_post(client, owner, "blog-commenti-1")

    res = await client.post(
        f"/api/v1/posts/{post_id}/comments", json={"content": "Bellissimo!"}, headers=commenter.headers
    )
    assert res.status_code == 201
    assert res.json()["status"] == "approved"
    assert res.json()["author_display_name"] == "commenter1"

    list_res = await client.get(f"/api/v1/posts/{post_id}/comments")
    assert len(list_res.json()) == 1


async def test_anonymous_comment_blocked_by_default(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner-c2")
    post_id = await _published_post(client, owner, "blog-commenti-2")

    res = await client.post(f"/api/v1/posts/{post_id}/comments", json={"content": "Anonimo"})
    assert res.status_code == 401


async def test_comments_closed_blocks_everyone(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner-c2b")
    other: AuthedUser = await make_user("other-c2b")
    post_id = await _published_post(client, owner, "blog-commenti-2b")

    await client.patch(
        "/api/v1/blogs/blog-commenti-2b", json={"comments_mode": "closed"}, headers=owner.headers
    )

    res = await client.post(
        f"/api/v1/posts/{post_id}/comments", json={"content": "x"}, headers=other.headers
    )
    assert res.status_code == 403


async def test_anonymous_comment_requires_captcha_when_open_to_everyone(
    client: AsyncClient, make_user: Callable, turnstile_enabled: None
) -> None:
    owner: AuthedUser = await make_user("owner-c3")
    post_id = await _published_post(client, owner, "blog-commenti-3")

    await client.patch(
        "/api/v1/blogs/blog-commenti-3", json={"comments_mode": "everyone"}, headers=owner.headers
    )

    missing_fields_res = await client.post(f"/api/v1/posts/{post_id}/comments", json={"content": "x"})
    assert missing_fields_res.status_code == 400

    missing_captcha_res = await client.post(
        f"/api/v1/posts/{post_id}/comments",
        json={"content": "x", "author_display_name": "Visitatore", "author_email": "v@example.com"},
    )
    assert missing_captcha_res.status_code == 400

    res = await client.post(
        f"/api/v1/posts/{post_id}/comments",
        json={
            "content": "Anonimo",
            "author_display_name": "Visitatore",
            "author_email": "v@example.com",
            "captcha_token": "qualsiasi-token-verificato-dal-fixture",
        },
    )
    assert res.status_code == 201
    assert res.json()["status"] == "pending"

    # non ancora visibile pubblicamente
    approved_res = await client.get(f"/api/v1/posts/{post_id}/comments")
    assert approved_res.json() == []


async def test_comments_mode_everyone_requires_turnstile_configured(
    client: AsyncClient, make_user: Callable
) -> None:
    """Senza turnstile_enabled (fixture non usata qui): l'istanza non ha
    Turnstile configurato, quindi non si può nemmeno impostare "everyone"."""
    owner: AuthedUser = await make_user("owner-c3b")
    await client.post("/api/v1/blogs", json={"slug": "blog-commenti-3b", "title": "x"}, headers=owner.headers)

    res = await client.patch(
        "/api/v1/blogs/blog-commenti-3b", json={"comments_mode": "everyone"}, headers=owner.headers
    )
    assert res.status_code == 400


async def test_moderation_approve_and_reject(
    client: AsyncClient, make_user: Callable, turnstile_enabled: None
) -> None:
    owner: AuthedUser = await make_user("owner-c4")
    stranger: AuthedUser = await make_user("stranger-c4")
    post_id = await _published_post(client, owner, "blog-commenti-4")
    await client.patch(
        "/api/v1/blogs/blog-commenti-4", json={"comments_mode": "everyone"}, headers=owner.headers
    )
    comment_res = await client.post(
        f"/api/v1/posts/{post_id}/comments",
        json={
            "content": "x",
            "author_display_name": "V",
            "author_email": "v@example.com",
            "captcha_token": "ok",
        },
    )
    comment_id = comment_res.json()["id"]

    # un utente estraneo non può vedere la coda né moderare
    forbidden_pending = await client.get(f"/api/v1/posts/{post_id}/comments/pending", headers=stranger.headers)
    assert forbidden_pending.status_code == 403
    forbidden_approve = await client.post(f"/api/v1/comments/{comment_id}/approve", headers=stranger.headers)
    assert forbidden_approve.status_code == 403

    pending_res = await client.get(f"/api/v1/posts/{post_id}/comments/pending", headers=owner.headers)
    assert len(pending_res.json()) == 1

    approve_res = await client.post(f"/api/v1/comments/{comment_id}/approve", headers=owner.headers)
    assert approve_res.status_code == 200
    assert approve_res.json()["status"] == "approved"

    approved_list = await client.get(f"/api/v1/posts/{post_id}/comments")
    assert len(approved_list.json()) == 1


async def test_reject_comment(client: AsyncClient, make_user: Callable, turnstile_enabled: None) -> None:
    owner: AuthedUser = await make_user("owner-c5")
    post_id = await _published_post(client, owner, "blog-commenti-5")
    await client.patch(
        "/api/v1/blogs/blog-commenti-5", json={"comments_mode": "everyone"}, headers=owner.headers
    )
    comment_res = await client.post(
        f"/api/v1/posts/{post_id}/comments",
        json={
            "content": "spam",
            "author_display_name": "V",
            "author_email": "v@example.com",
            "captcha_token": "ok",
        },
    )
    comment_id = comment_res.json()["id"]

    reject_res = await client.post(f"/api/v1/comments/{comment_id}/reject", headers=owner.headers)
    assert reject_res.status_code == 200
    assert reject_res.json()["status"] == "rejected"

    approved_list = await client.get(f"/api/v1/posts/{post_id}/comments")
    assert approved_list.json() == []


async def test_reply_thread(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner-c7")
    commenter: AuthedUser = await make_user("commenter-c7")
    post_id = await _published_post(client, owner, "blog-commenti-7")

    top_res = await client.post(
        f"/api/v1/posts/{post_id}/comments", json={"content": "primo commento"}, headers=owner.headers
    )
    top_id = top_res.json()["id"]
    assert top_res.json()["parent_id"] is None

    reply_res = await client.post(
        f"/api/v1/posts/{post_id}/comments",
        json={"content": "risposta", "parent_id": top_id},
        headers=commenter.headers,
    )
    assert reply_res.status_code == 201
    assert reply_res.json()["parent_id"] == top_id

    listed = await client.get(f"/api/v1/posts/{post_id}/comments")
    assert {c["id"]: c["parent_id"] for c in listed.json()} == {top_id: None, reply_res.json()["id"]: top_id}


async def test_reply_to_comment_on_different_post_rejected(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user("owner-c8")
    post_a = await _published_post(client, owner, "blog-commenti-8a")
    await client.post("/api/v1/blogs", json={"slug": "blog-commenti-8b", "title": "x"}, headers=owner.headers)
    post_b_res = await client.post(
        "/api/v1/blogs/blog-commenti-8b/posts",
        json={"slug": "post-b", "title": "x", "content": "y"},
        headers=owner.headers,
    )
    post_b = post_b_res.json()["id"]
    await client.post(f"/api/v1/posts/{post_b}/publish", headers=owner.headers)

    comment_a = await client.post(
        f"/api/v1/posts/{post_a}/comments", json={"content": "su A"}, headers=owner.headers
    )
    comment_a_id = comment_a.json()["id"]

    res = await client.post(
        f"/api/v1/posts/{post_b}/comments",
        json={"content": "risposta sbagliata", "parent_id": comment_a_id},
        headers=owner.headers,
    )
    assert res.status_code == 400


async def test_blog_comments_aggregate_moderation(
    client: AsyncClient, make_user: Callable, turnstile_enabled: None
) -> None:
    owner: AuthedUser = await make_user("owner-c6")
    stranger: AuthedUser = await make_user("stranger-c6")
    await client.post("/api/v1/blogs", json={"slug": "blog-commenti-6", "title": "x"}, headers=owner.headers)
    await client.patch(
        "/api/v1/blogs/blog-commenti-6", json={"comments_mode": "everyone"}, headers=owner.headers
    )

    post_ids = []
    for n in (1, 2):
        res = await client.post(
            "/api/v1/blogs/blog-commenti-6/posts",
            json={"slug": f"post-agg-{n}", "title": f"Titolo {n}", "content": "y"},
            headers=owner.headers,
        )
        pid = res.json()["id"]
        await client.post(f"/api/v1/posts/{pid}/publish", headers=owner.headers)
        post_ids.append(pid)
        await client.post(
            f"/api/v1/posts/{pid}/comments",
            json={
                "content": f"pending {n}",
                "author_display_name": "V",
                "author_email": "v@example.com",
                "captcha_token": "ok",
            },
        )

    # un commento già approvato (autore registrato) sul primo post
    await client.post(
        f"/api/v1/posts/{post_ids[0]}/comments", json={"content": "ok"}, headers=stranger.headers
    )

    # default: solo i pending di tutti i post, con titolo/slug del post
    agg = await client.get("/api/v1/blogs/blog-commenti-6/comments", headers=owner.headers)
    assert agg.status_code == 200
    body = agg.json()
    assert len(body) == 2
    assert {c["status"] for c in body} == {"pending"}
    assert {c["post_title"] for c in body} == {"Titolo 1", "Titolo 2"}
    assert all("post_slug" in c for c in body)

    # filtro esplicito su approved
    approved = await client.get(
        "/api/v1/blogs/blog-commenti-6/comments?status=approved", headers=owner.headers
    )
    assert [c["content"] for c in approved.json()] == ["ok"]

    # chi non modera: 403
    forbidden = await client.get(
        "/api/v1/blogs/blog-commenti-6/comments", headers=stranger.headers
    )
    assert forbidden.status_code == 403

    # blog inesistente: 404
    missing = await client.get("/api/v1/blogs/non-esiste/comments", headers=owner.headers)
    assert missing.status_code == 404
