from collections.abc import Callable

from httpx import AsyncClient

from tests.conftest import AuthedUser, FakeS3Client


async def test_create_blog(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user()
    res = await client.post(
        "/api/v1/blogs", json={"slug": "il-mio-blog", "title": "Il mio blog"}, headers=user.headers
    )
    assert res.status_code == 201
    body = res.json()
    assert body["slug"] == "il-mio-blog"
    assert body["default_locale"] == "it"
    assert body["owner_id"]


async def test_create_blog_requires_auth(client: AsyncClient) -> None:
    res = await client.post("/api/v1/blogs", json={"slug": "il-mio-blog", "title": "x"})
    assert res.status_code in (401, 403)


async def test_blog_slug_too_short(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user()
    res = await client.post("/api/v1/blogs", json={"slug": "ab", "title": "x"}, headers=user.headers)
    assert res.status_code == 400


async def test_blog_slug_reserved(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user()
    res = await client.post("/api/v1/blogs", json={"slug": "blog", "title": "x"}, headers=user.headers)
    assert res.status_code == 400


async def test_blog_slug_conflict(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user()
    await client.post("/api/v1/blogs", json={"slug": "duplicato", "title": "x"}, headers=user.headers)
    res = await client.post("/api/v1/blogs", json={"slug": "duplicato", "title": "y"}, headers=user.headers)
    assert res.status_code == 409


async def test_max_five_blogs_per_user(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user()
    for i in range(5):
        res = await client.post(
            "/api/v1/blogs", json={"slug": f"blog-numero-{i}", "title": f"Blog {i}"}, headers=user.headers
        )
        assert res.status_code == 201, res.text

    res = await client.post("/api/v1/blogs", json={"slug": "blog-numero-6", "title": "x"}, headers=user.headers)
    assert res.status_code == 400


async def test_get_blog_public(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user()
    await client.post("/api/v1/blogs", json={"slug": "pubblico-blog", "title": "x"}, headers=user.headers)

    res = await client.get("/api/v1/blogs/pubblico-blog")
    assert res.status_code == 200
    assert res.json()["slug"] == "pubblico-blog"

    missing_res = await client.get("/api/v1/blogs/non-esiste")
    assert missing_res.status_code == 404


async def test_update_blog_owner_only(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner1")
    stranger: AuthedUser = await make_user("stranger1")
    await client.post("/api/v1/blogs", json={"slug": "proprieta-blog", "title": "x"}, headers=owner.headers)

    forbidden_res = await client.patch(
        "/api/v1/blogs/proprieta-blog", json={"title": "rubato"}, headers=stranger.headers
    )
    assert forbidden_res.status_code == 403

    ok_res = await client.patch(
        "/api/v1/blogs/proprieta-blog",
        json={"title": "nuovo titolo", "allow_anonymous_comments": True},
        headers=owner.headers,
    )
    assert ok_res.status_code == 200
    assert ok_res.json()["title"] == "nuovo titolo"
    assert ok_res.json()["allow_anonymous_comments"] is True


async def test_default_author_display_name_used_when_post_omits_it(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user("owner-pen-name")
    await client.post(
        "/api/v1/blogs", json={"slug": "blog-pen-name", "title": "x"}, headers=owner.headers
    )

    set_res = await client.patch(
        "/api/v1/blogs/blog-pen-name",
        json={"default_author_display_name": "Nome di Penna"},
        headers=owner.headers,
    )
    assert set_res.json()["default_author_display_name"] == "Nome di Penna"

    post_res = await client.post(
        "/api/v1/blogs/blog-pen-name/posts",
        json={"slug": "post-anonimo", "title": "x", "content": "y"},
        headers=owner.headers,
    )
    assert post_res.json()["author_display_name"] == "Nome di Penna"

    # un autore che specifica esplicitamente il proprio nome per il singolo
    # post continua a poterlo fare, sovrascrivendo il default del blog
    override_res = await client.post(
        "/api/v1/blogs/blog-pen-name/posts",
        json={
            "slug": "post-con-nome",
            "title": "x",
            "content": "y",
            "author_display_name": "Altro Nome",
        },
        headers=owner.headers,
    )
    assert override_res.json()["author_display_name"] == "Altro Nome"

    # rimozione con stringa vuota: torna al fallback sullo username
    clear_res = await client.patch(
        "/api/v1/blogs/blog-pen-name",
        json={"default_author_display_name": ""},
        headers=owner.headers,
    )
    assert clear_res.json()["default_author_display_name"] is None


async def test_blog_follow_unfollow(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner2")
    follower: AuthedUser = await make_user("follower2")
    await client.post("/api/v1/blogs", json={"slug": "seguito-blog", "title": "x"}, headers=owner.headers)

    follow_res = await client.post("/api/v1/blogs/seguito-blog/follow", headers=follower.headers)
    assert follow_res.status_code == 204
    # idempotente
    again_res = await client.post("/api/v1/blogs/seguito-blog/follow", headers=follower.headers)
    assert again_res.status_code == 204

    followers_res = await client.get("/api/v1/blogs/seguito-blog/followers")
    assert followers_res.status_code == 200
    assert [f["username"] for f in followers_res.json()] == ["follower2"]

    unfollow_res = await client.delete("/api/v1/blogs/seguito-blog/follow", headers=follower.headers)
    assert unfollow_res.status_code == 204
    followers_res2 = await client.get("/api/v1/blogs/seguito-blog/followers")
    assert followers_res2.json() == []


async def test_media_upload_requires_write_access(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner-media")
    stranger: AuthedUser = await make_user("stranger-media")
    await client.post("/api/v1/blogs", json={"slug": "blog-media-test", "title": "x"}, headers=owner.headers)

    res = await client.post(
        "/api/v1/blogs/blog-media-test/media",
        files={"file": ("x.png", b"\x89PNG\r\n\x1a\n0000", "image/png")},
        headers=stranger.headers,
    )
    assert res.status_code == 403


async def test_media_upload_public_url(
    client: AsyncClient, make_user: Callable, fake_s3: FakeS3Client
) -> None:
    owner: AuthedUser = await make_user("owner-media2")
    await client.post("/api/v1/blogs", json={"slug": "blog-media-test2", "title": "x"}, headers=owner.headers)

    res = await client.post(
        "/api/v1/blogs/blog-media-test2/media",
        files={"file": ("x.png", b"\x89PNG\r\n\x1a\n0000", "image/png")},
        headers=owner.headers,
    )
    assert res.status_code == 201
    url = res.json()["url"]
    assert "/userdata/" in url
    assert "/media/" in url
    assert len(fake_s3.objects) == 1

    rejected = await client.post(
        "/api/v1/blogs/blog-media-test2/media",
        files={"file": ("x.txt", b"non e' un'immagine", "text/plain")},
        headers=owner.headers,
    )
    assert rejected.status_code == 400
