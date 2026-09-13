"""todo/UX_REDESIGN.md B1: panoramiche (blog e piattaforma), directory con
conteggi, elenchi pubblici per utente, feed dei seguiti, campi extra admin."""

from collections.abc import Callable

from httpx import AsyncClient

from tests.conftest import AuthedUser


async def _blog(client: AsyncClient, owner: AuthedUser, slug: str, **extra) -> dict:
    res = await client.post("/api/v1/blogs", json={"slug": slug, "title": slug, **extra}, headers=owner.headers)
    assert res.status_code == 201, res.text
    return res.json()


async def _post(client: AsyncClient, owner: AuthedUser, blog_slug: str, slug: str, publish: bool = True) -> dict:
    res = await client.post(
        f"/api/v1/blogs/{blog_slug}/posts",
        json={"slug": slug, "title": slug, "content": "testo del post"},
        headers=owner.headers,
    )
    assert res.status_code == 201, res.text
    post = res.json()
    if publish:
        res = await client.post(f"/api/v1/posts/{post['id']}/publish", headers=owner.headers)
        assert res.status_code == 200, res.text
        post = res.json()
    return post


async def test_blog_overview_counts_and_access(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("ov-owner")
    reader: AuthedUser = await make_user("ov-reader")
    await _blog(client, owner, "ov-blog")
    await _post(client, owner, "ov-blog", "pubblicato")
    await _post(client, owner, "ov-blog", "bozza", publish=False)
    follow = await client.post("/api/v1/blogs/ov-blog/follow", headers=reader.headers)
    assert follow.status_code in (200, 201, 204), follow.text

    res = await client.get("/api/v1/blogs/ov-blog/overview", headers=owner.headers)
    assert res.status_code == 200, res.text
    data = res.json()
    assert data["posts_total"] == 2
    assert data["posts_published"] == 1
    assert data["posts_draft"] == 1
    assert data["followers"] == 1
    assert data["pending_comments"] == 0
    assert data["last_published_at"] is not None

    forbidden = await client.get("/api/v1/blogs/ov-blog/overview", headers=reader.headers)
    assert forbidden.status_code == 403
    anonymous = await client.get("/api/v1/blogs/ov-blog/overview")
    assert anonymous.status_code == 401


async def test_public_directory_search_and_counts(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("dir-owner")
    reader: AuthedUser = await make_user("dir-reader")
    await _blog(client, owner, "dir-alpha", title="Quaderno alpha")
    await _blog(client, owner, "dir-beta", title="Beta", default_locale="en")
    await _post(client, owner, "dir-alpha", "uno")
    await client.post("/api/v1/blogs/dir-alpha/follow", headers=reader.headers)

    res = await client.get("/api/v1/blogs", params={"q": "quaderno"})
    assert res.status_code == 200
    slugs = [b["slug"] for b in res.json()]
    assert slugs == ["dir-alpha"]
    alpha = res.json()[0]
    assert alpha["post_count"] == 1
    assert alpha["follower_count"] == 1
    assert alpha["owner_id"] is None

    res = await client.get("/api/v1/blogs", params={"locale": "en"})
    assert [b["slug"] for b in res.json()] == ["dir-beta"]

    res = await client.get("/api/v1/blogs", params={"sort": "followers"})
    assert [b["slug"] for b in res.json()][0] == "dir-alpha"


async def test_user_public_lists_hide_aliased_content(client: AsyncClient, make_user: Callable) -> None:
    author: AuthedUser = await make_user("pub-author")
    reader: AuthedUser = await make_user("pub-reader")
    await _blog(client, author, "pub-open")
    # blog firmato con un alias: non deve comparire, né i suoi post (CLAUDE.md #8)
    await _blog(client, author, "pub-alias", default_author_display_name="Lume")
    open_post = await _post(client, author, "pub-open", "aperto")
    await _post(client, author, "pub-alias", "nascosto")

    comment = await client.post(
        f"/api/v1/posts/{open_post['id']}/comments", json={"content": "bel post"}, headers=reader.headers
    )
    assert comment.status_code == 201, comment.text

    blogs = await client.get("/api/v1/users/pub-author/blogs")
    assert blogs.status_code == 200
    assert [b["slug"] for b in blogs.json()] == ["pub-open"]

    posts = await client.get("/api/v1/users/pub-author/posts")
    assert posts.status_code == 200
    assert [p["slug"] for p in posts.json()] == ["aperto"]

    comments = await client.get("/api/v1/users/pub-reader/comments")
    assert comments.status_code == 200
    assert len(comments.json()) == 1
    assert comments.json()[0]["permalink"] == "/pub-open/aperto"
    assert comments.json()[0]["post_title"] == "aperto"

    missing = await client.get("/api/v1/users/nessuno/blogs")
    assert missing.status_code == 404


async def test_feed_following_filters_by_followed_blogs_and_users(client: AsyncClient, make_user: Callable) -> None:
    a: AuthedUser = await make_user("fol-a")
    b: AuthedUser = await make_user("fol-b")
    c: AuthedUser = await make_user("fol-c")
    reader: AuthedUser = await make_user("fol-reader")
    await _blog(client, a, "fol-blog-a")
    await _blog(client, b, "fol-blog-b")
    await _blog(client, c, "fol-blog-c")
    pa = await _post(client, a, "fol-blog-a", "da-a")
    pb = await _post(client, b, "fol-blog-b", "da-b")
    pc = await _post(client, c, "fol-blog-c", "da-c")
    await client.post("/api/v1/blogs/fol-blog-a/follow", headers=reader.headers)
    await client.post("/api/v1/users/fol-b/follow", headers=reader.headers)

    res = await client.get("/api/v1/feed/posts", params={"following": "true"}, headers=reader.headers)
    assert res.status_code == 200, res.text
    ids = {p["id"] for p in res.json()}
    assert pa["id"] in ids and pb["id"] in ids and pc["id"] not in ids

    anonymous = await client.get("/api/v1/feed/posts", params={"following": "true"})
    assert anonymous.status_code == 401


async def test_admin_overview_and_user_fields(
    client: AsyncClient, make_user: Callable, make_admin: Callable, make_moderator: Callable
) -> None:
    admin: AuthedUser = await make_admin("ov-admin")
    moderator: AuthedUser = await make_moderator("ov-mod")
    plain: AuthedUser = await make_user("ov-plain")
    await _blog(client, plain, "ov-plain-blog")

    res = await client.get("/api/v1/admin/overview", headers=admin.headers)
    assert res.status_code == 200, res.text
    data = res.json()
    assert data["users_total"] >= 3
    assert data["blogs_total"] >= 1
    assert data["queue_pending_comments"] == 0
    names = {s["name"] for s in data["services"]}
    assert {"postgres", "redis", "rabbitmq", "storage", "moderation"} <= names
    assert next(s for s in data["services"] if s["name"] == "postgres")["status"] == "ok"

    assert (await client.get("/api/v1/admin/overview", headers=moderator.headers)).status_code == 200
    assert (await client.get("/api/v1/admin/overview", headers=plain.headers)).status_code == 403

    users = await client.get("/api/v1/admin/users", params={"q": "ov-plain"}, headers=admin.headers)
    assert users.status_code == 200
    row = next(u for u in users.json() if u["username"] == "ov-plain")
    assert row["blogs_count"] == 1
    assert "last_seen_at" in row


async def test_palette_dark_validated_like_palette(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("pal-owner")
    await _blog(client, owner, "pal-blog")
    ok = await client.put(
        "/api/v1/blogs/pal-blog/config",
        json={"palette": {"background": "#faf8f4"}, "palette_dark": {"background": "#18191b", "primary": "#83b8a5"}},
        headers=owner.headers,
    )
    assert ok.status_code == 200, ok.text
    assert ok.json()["palette_dark"]["primary"] == "#83b8a5"
    bad = await client.put(
        "/api/v1/blogs/pal-blog/config",
        json={"palette_dark": {"primary": "#ff0000"}},
        headers=owner.headers,
    )
    assert bad.status_code == 400
