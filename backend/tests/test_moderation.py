from collections.abc import Callable

from httpx import AsyncClient

from app.domain.moderation import classify_image
from tests.conftest import AuthedUser


async def test_classify_image_returns_false_when_service_not_configured() -> None:
    """NOCT_MODERATION_SERVICE_URL non impostato (default) -> mai bloccante,
    nessuna chiamata di rete tentata."""
    result = await classify_image(b"fake-bytes", "x.png", "image/png")
    assert result is False


async def test_create_post_stores_cover_sensitivity_flag(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user("owner-cover-sensitive")
    await client.post(
        "/api/v1/blogs", json={"slug": "blog-cover-sensitive", "title": "x"}, headers=owner.headers
    )

    res = await client.post(
        "/api/v1/blogs/blog-cover-sensitive/posts",
        json={
            "slug": "post-cover-sensibile",
            "title": "x",
            "content": "y",
            "cover_image_url": "https://x/cover.png",
            "cover_image_is_sensitive": True,
        },
        headers=owner.headers,
    )
    assert res.status_code == 201
    assert res.json()["cover_image_is_sensitive"] is True
    post_id = res.json()["id"]

    # rimuovere la cover azzera anche il flag, anche se passato di nuovo True
    clear_res = await client.patch(
        f"/api/v1/posts/{post_id}",
        json={"cover_image_url": "", "cover_image_is_sensitive": True},
        headers=owner.headers,
    )
    assert clear_res.json()["cover_image_url"] is None
    assert clear_res.json()["cover_image_is_sensitive"] is False
