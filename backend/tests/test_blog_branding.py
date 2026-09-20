"""Cover image e favicon del blog (identità/branding, facoltative)."""

from collections.abc import Callable

from httpx import AsyncClient

from tests.conftest import AuthedUser, FakeS3Client
from tests.test_overview import _blog


async def test_cover_image_upload_update_categories_and_delete(
    client: AsyncClient, make_user: Callable, fake_s3: FakeS3Client
) -> None:
    owner: AuthedUser = await make_user("cover-owner")
    other: AuthedUser = await make_user("cover-other")
    await _blog(client, owner, "cover-blog")

    assert (
        await client.post(
            "/api/v1/blogs/cover-blog/cover-image",
            files={"file": ("a.png", b"\x89PNG" + b"1" * 50, "image/png")},
            headers=other.headers,
        )
    ).status_code == 403

    up = await client.post(
        "/api/v1/blogs/cover-blog/cover-image",
        files={"file": ("a.png", b"\x89PNG" + b"1" * 50, "image/png")},
        headers=owner.headers,
    )
    assert up.status_code == 201, up.text
    body = up.json()
    assert body["cover_image_url"]
    assert body["cover_image_is_sensitive"] is False
    assert body["cover_image_categories"] == []

    upd = await client.patch(
        "/api/v1/blogs/cover-blog/cover-image", json={"categories": ["nudity"]}, headers=owner.headers
    )
    assert upd.status_code == 200
    assert upd.json()["cover_image_categories"] == ["nudity"]
    assert upd.json()["cover_image_is_sensitive"] is True
    # alt_text omesso: resta invariato (vuoto di default)
    assert upd.json()["cover_image_alt_text"] == ""

    alt_upd = await client.patch(
        "/api/v1/blogs/cover-blog/cover-image", json={"categories": ["nudity"], "alt_text": "Copertina del blog"}, headers=owner.headers
    )
    assert alt_upd.json()["cover_image_alt_text"] == "Copertina del blog"

    bad = await client.patch(
        "/api/v1/blogs/cover-blog/cover-image", json={"categories": ["boh"]}, headers=owner.headers
    )
    assert bad.status_code == 400

    # la cover caricata finisce nella libreria media del blog (B7)
    lib = await client.get("/api/v1/blogs/cover-blog/media", headers=owner.headers)
    assert lib.status_code == 200
    assert len(lib.json()["items"]) == 1
    assert lib.json()["items"][0]["url"] == body["cover_image_url"]
    assert lib.json()["items"][0]["used_as_blog_cover"] is True

    deleted = await client.delete("/api/v1/blogs/cover-blog/cover-image", headers=owner.headers)
    assert deleted.status_code == 200
    assert deleted.json()["cover_image_url"] is None
    assert deleted.json()["cover_image_categories"] == []
    assert deleted.json()["cover_image_alt_text"] == ""

    # nessun oggetto content cancellato dalla sostituzione/rimozione (stessa
    # scelta di Post.cover_image_url, vedi branding.py)
    assert any("/media/" in k for (_b, k) in fake_s3.objects)


async def test_cover_image_categories_require_existing_cover(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user("cover2-owner")
    await _blog(client, owner, "cover2-blog")
    res = await client.patch(
        "/api/v1/blogs/cover2-blog/cover-image", json={"categories": ["nudity"]}, headers=owner.headers
    )
    assert res.status_code == 400


async def test_favicon_upload_replace_and_delete(
    client: AsyncClient, make_user: Callable, fake_s3: FakeS3Client
) -> None:
    owner: AuthedUser = await make_user("favicon-owner")
    other: AuthedUser = await make_user("favicon-other")
    await _blog(client, owner, "favicon-blog")

    assert (
        await client.post(
            "/api/v1/blogs/favicon-blog/favicon",
            files={"file": ("a.png", b"\x89PNG" + b"1" * 20, "image/png")},
            headers=other.headers,
        )
    ).status_code == 403

    up = await client.post(
        "/api/v1/blogs/favicon-blog/favicon",
        files={"file": ("a.png", b"\x89PNG" + b"1" * 20, "image/png")},
        headers=owner.headers,
    )
    assert up.status_code == 201, up.text
    first_url = up.json()["favicon_url"]
    assert first_url
    assert any("favicons/" in k for (_b, k) in fake_s3.objects)

    # sostituzione: la precedente viene cancellata dallo storage
    up2 = await client.post(
        "/api/v1/blogs/favicon-blog/favicon",
        files={"file": ("b.png", b"\x89PNG" + b"2" * 20, "image/png")},
        headers=owner.headers,
    )
    assert up2.status_code == 201
    second_url = up2.json()["favicon_url"]
    assert second_url != first_url
    favicon_keys = [k for (_b, k) in fake_s3.objects if "favicons/" in k]
    assert len(favicon_keys) == 1

    # formato non supportato
    bad = await client.post(
        "/api/v1/blogs/favicon-blog/favicon",
        files={"file": ("a.svg", b"<svg/>", "image/svg+xml")},
        headers=owner.headers,
    )
    assert bad.status_code == 400

    deleted = await client.delete("/api/v1/blogs/favicon-blog/favicon", headers=owner.headers)
    assert deleted.status_code == 200
    assert deleted.json()["favicon_url"] is None
    assert not any("favicons/" in k for (_b, k) in fake_s3.objects)


async def test_get_blog_exposes_cover_and_favicon(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("cover3-owner")
    await _blog(client, owner, "cover3-blog")
    res = await client.get("/api/v1/blogs/cover3-blog")
    assert res.status_code == 200
    body = res.json()
    assert body["cover_image_url"] is None
    assert body["favicon_url"] is None
