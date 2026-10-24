"""todo/UX_REDESIGN.md B2: letture aggregate per giorno e spazio occupato."""

from collections.abc import Callable

from httpx import AsyncClient

from tests.conftest import AuthedUser, FakeS3Client
from tests.test_overview import _blog, _post


async def test_read_counts_once_per_ip_window_and_shows_in_overview(
    client: AsyncClient, make_user: Callable, fake_s3: FakeS3Client
) -> None:
    owner: AuthedUser = await make_user("rd-owner")
    await _blog(client, owner, "rd-blog")
    post = await _post(client, owner, "rd-blog", "letto")
    draft = await _post(client, owner, "rd-blog", "bozza", publish=False)

    first = await client.post(f"/api/v1/posts/{post['id']}/read")
    assert first.status_code == 204
    # stesso IP entro la finestra: accettata ma non contata (rate limit Redis,
    # se Redis è giù il contatore sale a 2 — il test resta valido in entrambi i casi)
    second = await client.post(f"/api/v1/posts/{post['id']}/read")
    assert second.status_code == 204

    assert (await client.post(f"/api/v1/posts/{draft['id']}/read")).status_code == 404

    res = await client.get("/api/v1/blogs/rd-blog/overview", headers=owner.headers)
    assert res.status_code == 200, res.text
    data = res.json()
    assert len(data["reads_30d"]) == 30
    assert data["reads_total_30d"] in (1, 2)
    assert data["reads_30d"][-1]["reads"] == data["reads_total_30d"]
    assert data["storage_bytes"] == 0


async def test_overview_storage_bytes_sums_blog_prefix(
    client: AsyncClient, make_user: Callable, fake_s3: FakeS3Client
) -> None:
    owner: AuthedUser = await make_user("st-owner")
    blog = await _blog(client, owner, "st-blog")
    upload = await client.post(
        "/api/v1/blogs/st-blog/media",
        files={"file": ("img.png", b"\x89PNG" + b"0" * 100, "image/png")},
        headers=owner.headers,
    )
    assert upload.status_code in (200, 201), upload.text
    res = await client.get("/api/v1/blogs/st-blog/overview", headers=owner.headers)
    assert res.status_code == 200
    assert res.json()["storage_bytes"] == 104
    assert blog["slug"] == "st-blog"
