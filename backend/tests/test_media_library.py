"""todo/UX_REDESIGN.md B7: libreria media del blog."""

from collections.abc import Callable

from httpx import AsyncClient
from sqlalchemy.ext.asyncio import AsyncSession

from tests.conftest import AuthedUser, FakeS3Client
from tests.test_overview import _blog
from tests.test_platform_config import _super


async def test_upload_lists_edits_and_deletes_media(client: AsyncClient, make_user: Callable, fake_s3: FakeS3Client) -> None:
    owner: AuthedUser = await make_user("ml-owner")
    other: AuthedUser = await make_user("ml-other")
    await _blog(client, owner, "ml-blog")
    up = await client.post("/api/v1/blogs/ml-blog/media", files={"file": ("a.png", b"\x89PNG" + b"1" * 50, "image/png")}, headers=owner.headers)
    assert up.status_code == 201, up.text
    url = up.json()["url"]
    assert up.json()["media_id"]

    assert (await client.get("/api/v1/blogs/ml-blog/media", headers=other.headers)).status_code == 403
    lib = await client.get("/api/v1/blogs/ml-blog/media", headers=owner.headers)
    assert lib.status_code == 200
    assert lib.json()["total_bytes"] == 54 and len(lib.json()["items"]) == 1
    item = lib.json()["items"][0]
    assert item["url"] == url and item["used_in"] == [] and item["uploader_username"] == "ml-owner"

    upd = await client.patch(
        f"/api/v1/blogs/ml-blog/media/{item['id']}", json={"alt_text": "Lampada", "caption": "Notte", "categories": ["nudity"]}, headers=owner.headers
    )
    assert upd.status_code == 200 and upd.json()["alt_text"] == "Lampada" and upd.json()["categories"] == ["nudity"]
    bad = await client.patch(f"/api/v1/blogs/ml-blog/media/{item['id']}", json={"categories": ["boh"]}, headers=owner.headers)
    assert bad.status_code == 400

    # usata in un post → non cancellabile
    post = await client.post(
        "/api/v1/blogs/ml-blog/posts", json={"slug": "con-img", "title": "x", "content": f"![Lampada]({url})"}, headers=owner.headers
    )
    assert post.status_code == 201
    lib = await client.get("/api/v1/blogs/ml-blog/media", headers=owner.headers)
    assert lib.json()["items"][0]["used_in"][0]["post_slug"] == "con-img"
    assert (await client.delete(f"/api/v1/blogs/ml-blog/media/{item['id']}", headers=owner.headers)).status_code == 409

    # tolta dal post → cancellabile, oggetto rimosso dallo storage
    await client.patch(f"/api/v1/posts/{post.json()['id']}", json={"content": "senza immagine"}, headers=owner.headers)
    assert (await client.delete(f"/api/v1/blogs/ml-blog/media/{item['id']}", headers=owner.headers)).status_code == 204
    assert not any(k for (b, k) in fake_s3.objects if "/media/" in k)
    assert (await client.get("/api/v1/blogs/ml-blog/media", headers=owner.headers)).json()["items"] == []


async def test_sync_imports_legacy_post_media(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("ms-owner")
    await _blog(client, owner, "ms-blog")
    await client.post(
        "/api/v1/blogs/ms-blog/posts",
        json={"slug": "legacy", "title": "x", "content": '![Vecchia](https://cdn.example/old.png "sensitive:nudity")'},
        headers=owner.headers,
    )
    res = await client.post("/api/v1/blogs/ms-blog/media/sync", headers=owner.headers)
    assert res.status_code == 200, res.text
    items = res.json()["items"]
    assert len(items) == 1
    assert items[0]["url"] == "https://cdn.example/old.png" and items[0]["alt_text"] == "Vecchia" and items[0]["categories"] == ["nudity"]
    assert items[0]["size_bytes"] == 0 and items[0]["used_in"][0]["post_slug"] == "legacy"
    again = await client.post("/api/v1/blogs/ms-blog/media/sync", headers=owner.headers)
    assert len(again.json()["items"]) == 1


async def test_image_used_only_as_post_cover_is_not_deletable(client: AsyncClient, make_user: Callable) -> None:
    """Bug trovato dal vivo: used_in era calcolato solo da post_media (immagini
    incorporate nel contenuto), mai da Post.cover_image_url — un'immagine
    usata solo come cover risultava "libera" e cancellabile mentre era
    ancora la cover live del post."""
    owner: AuthedUser = await make_user("mc-owner")
    await _blog(client, owner, "mc-blog")
    up = await client.post(
        "/api/v1/blogs/mc-blog/media", files={"file": ("a.png", b"\x89PNG" + b"1" * 50, "image/png")}, headers=owner.headers
    )
    url = up.json()["url"]
    media_id = up.json()["media_id"]

    post = await client.post(
        "/api/v1/blogs/mc-blog/posts",
        json={"slug": "con-cover", "title": "x", "content": "senza immagini nel corpo", "cover_image_url": url},
        headers=owner.headers,
    )
    assert post.status_code == 201, post.text

    lib = await client.get("/api/v1/blogs/mc-blog/media", headers=owner.headers)
    item = next(i for i in lib.json()["items"] if i["id"] == media_id)
    assert item["used_in"][0]["post_slug"] == "con-cover"

    assert (await client.delete(f"/api/v1/blogs/mc-blog/media/{media_id}", headers=owner.headers)).status_code == 409

    # tolta la cover dal post → di nuovo cancellabile
    await client.patch(f"/api/v1/posts/{post.json()['id']}", json={"cover_image_url": ""}, headers=owner.headers)
    assert (await client.delete(f"/api/v1/blogs/mc-blog/media/{media_id}", headers=owner.headers)).status_code == 204


async def test_blog_cover_upload_lands_in_library_and_is_protected(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("bc-owner")
    await _blog(client, owner, "bc-blog")
    up = await client.post(
        "/api/v1/blogs/bc-blog/cover-image", files={"file": ("banner.png", b"\x89PNG" + b"1" * 50, "image/png")}, headers=owner.headers
    )
    assert up.status_code == 201, up.text
    cover_url = up.json()["cover_image_url"]

    lib = await client.get("/api/v1/blogs/bc-blog/media", headers=owner.headers)
    assert lib.status_code == 200
    items = lib.json()["items"]
    assert len(items) == 1 and items[0]["url"] == cover_url and items[0]["used_as_blog_cover"] is True

    assert (await client.delete(f"/api/v1/blogs/bc-blog/media/{items[0]['id']}", headers=owner.headers)).status_code == 409

    # rimossa la cover dal blog → di nuovo cancellabile
    await client.delete("/api/v1/blogs/bc-blog/cover-image", headers=owner.headers)
    assert (await client.delete(f"/api/v1/blogs/bc-blog/media/{items[0]['id']}", headers=owner.headers)).status_code == 204


async def test_media_upload_blocked_over_storage_quota(
    client: AsyncClient, make_user: Callable, make_admin: Callable, db_session: AsyncSession, fake_s3: FakeS3Client
) -> None:
    """`platform_config.max_blog_storage_mb` (blocco "impostazioni di
    piattaforma"): null di default (nessun limite, comportamento invariato),
    un upload che lo supererebbe risponde 413 (app/domain/storage_quota.py)."""
    owner: AuthedUser = await make_user("quota-owner")
    await _blog(client, owner, "quota-blog")
    root = await _super(make_admin, db_session, "quota-media-root")

    set_res = await client.patch(
        "/api/v1/admin/config", json={"max_blog_storage_mb": 1}, headers=root.headers
    )
    assert set_res.status_code == 200 and set_res.json()["max_blog_storage_mb"] == 1

    under_limit = b"\x89PNG" + b"1" * 700_000
    first = await client.post(
        "/api/v1/blogs/quota-blog/media",
        files={"file": ("a.png", under_limit, "image/png")},
        headers=owner.headers,
    )
    assert first.status_code == 201, first.text

    second = await client.post(
        "/api/v1/blogs/quota-blog/media",
        files={"file": ("b.png", under_limit, "image/png")},
        headers=owner.headers,
    )
    assert second.status_code == 413

    # senza limite, lo stesso upload torna a essere accettato
    await client.patch("/api/v1/admin/config", json={"max_blog_storage_mb": 0}, headers=root.headers)
    third = await client.post(
        "/api/v1/blogs/quota-blog/media",
        files={"file": ("c.png", under_limit, "image/png")},
        headers=owner.headers,
    )
    assert third.status_code == 201
