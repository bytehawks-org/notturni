"""GET /api/v1/feed/locales: pillole di lingua della homepage (§1 del blocco
"5 feature backend" — stessa clausola di visibilità di list_feed)."""

from collections.abc import Callable

from httpx import AsyncClient

from tests.conftest import AuthedUser


async def _publish(
    client: AsyncClient, owner: AuthedUser, blog_slug: str, slug: str, locale: str = "it"
) -> None:
    res = await client.post("/api/v1/blogs", json={"slug": blog_slug, "title": "x"}, headers=owner.headers)
    if res.status_code not in (201, 409):
        assert False, res.text
    created = await client.post(
        f"/api/v1/blogs/{blog_slug}/posts",
        json={"slug": slug, "title": "x", "content": "y" * 50, "locale": locale},
        headers=owner.headers,
    )
    assert created.status_code == 201, created.text
    pub = await client.post(f"/api/v1/posts/{created.json()['id']}/publish", headers=owner.headers)
    assert pub.status_code == 200, pub.text


async def test_feed_locales_counts_and_ordering(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("locales-owner")
    await _publish(client, owner, "blog-locales-1", "post-it-1", locale="it")
    await _publish(client, owner, "blog-locales-1", "post-it-2", locale="it")
    await _publish(client, owner, "blog-locales-2", "post-en-1", locale="en")

    res = await client.get("/api/v1/feed/locales")
    assert res.status_code == 200, res.text
    data = res.json()
    # "it" ha più post di "en": deve comparire per primo
    locales = [row["locale"] for row in data]
    assert locales.index("it") < locales.index("en")
    by_locale = {row["locale"]: row["count"] for row in data}
    assert by_locale["it"] >= 2
    assert by_locale["en"] >= 1


async def test_feed_locales_max_five_and_only_nonzero(client: AsyncClient, make_user: Callable) -> None:
    res = await client.get("/api/v1/feed/locales")
    assert res.status_code == 200, res.text
    data = res.json()
    assert len(data) <= 5
    assert all(row["count"] > 0 for row in data)
