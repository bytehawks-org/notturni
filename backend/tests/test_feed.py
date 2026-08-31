from collections.abc import Callable
from datetime import datetime, timedelta, timezone

from httpx import AsyncClient

from tests.conftest import AuthedUser


async def _create_and_publish(
    client: AsyncClient, owner: AuthedUser, blog_slug: str, post_slug: str, locale: str = "it"
) -> dict:
    payload = {"slug": post_slug, "title": post_slug, "content": "x"}
    if locale != "it":
        payload["locale"] = locale
    create_res = await client.post(f"/api/v1/blogs/{blog_slug}/posts", json=payload, headers=owner.headers)
    assert create_res.status_code == 201
    post_id = create_res.json()["id"]
    publish_res = await client.post(f"/api/v1/posts/{post_id}/publish", headers=owner.headers)
    assert publish_res.status_code == 200
    return publish_res.json()


async def test_feed_orders_by_published_at_desc_across_blogs(
    client: AsyncClient, make_user: Callable
) -> None:
    owner_a: AuthedUser = await make_user("feed-owner-a")
    owner_b: AuthedUser = await make_user("feed-owner-b")
    await client.post("/api/v1/blogs", json={"slug": "feed-blog-a", "title": "A"}, headers=owner_a.headers)
    await client.post("/api/v1/blogs", json={"slug": "feed-blog-b", "title": "B"}, headers=owner_b.headers)

    first = await _create_and_publish(client, owner_a, "feed-blog-a", "primo")
    second = await _create_and_publish(client, owner_b, "feed-blog-b", "secondo")
    third = await _create_and_publish(client, owner_a, "feed-blog-a", "terzo")

    res = await client.get("/api/v1/feed/posts")
    assert res.status_code == 200
    ids = [p["id"] for p in res.json()]
    # dal più recente: terzo, secondo, primo (a meno di altri post di altri
    # test nello stesso DB condiviso — verifichiamo solo l'ordine relativo)
    assert ids.index(third["id"]) < ids.index(second["id"]) < ids.index(first["id"])


async def test_feed_excludes_drafts_and_scheduled(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("feed-owner-draft")
    await client.post("/api/v1/blogs", json={"slug": "feed-blog-draft", "title": "x"}, headers=owner.headers)

    draft_res = await client.post(
        "/api/v1/blogs/feed-blog-draft/posts",
        json={"slug": "resta-bozza", "title": "x", "content": "y"},
        headers=owner.headers,
    )
    draft_id = draft_res.json()["id"]

    scheduled_create = await client.post(
        "/api/v1/blogs/feed-blog-draft/posts",
        json={"slug": "pianificato", "title": "x", "content": "y"},
        headers=owner.headers,
    )
    scheduled_id = scheduled_create.json()["id"]
    future = (datetime.now(timezone.utc) + timedelta(days=1)).isoformat()
    await client.post(
        f"/api/v1/posts/{scheduled_id}/publish", json={"published_at": future}, headers=owner.headers
    )

    res = await client.get("/api/v1/feed/posts")
    ids = [p["id"] for p in res.json()]
    assert draft_id not in ids
    assert scheduled_id not in ids


async def test_feed_filters_by_locale_and_paginates(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("feed-owner-locale")
    await client.post("/api/v1/blogs", json={"slug": "feed-blog-locale", "title": "x"}, headers=owner.headers)

    en_post = await _create_and_publish(client, owner, "feed-blog-locale", "hello", locale="en")
    await _create_and_publish(client, owner, "feed-blog-locale", "ciao", locale="it")

    res = await client.get("/api/v1/feed/posts?locale=en")
    ids = [p["id"] for p in res.json()]
    assert en_post["id"] in ids
    assert all(p["locale"] == "en" for p in res.json())

    limited_res = await client.get("/api/v1/feed/posts?limit=1")
    assert len(limited_res.json()) == 1


async def test_feed_filters_by_tag(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("feed-owner-tag")
    await client.post("/api/v1/blogs", json={"slug": "feed-blog-tag", "title": "x"}, headers=owner.headers)

    tagged_res = await client.post(
        "/api/v1/blogs/feed-blog-tag/posts",
        json={"slug": "con-tag", "title": "x", "content": "y", "tags": ["cucina"]},
        headers=owner.headers,
    )
    tagged_id = tagged_res.json()["id"]
    await client.post(f"/api/v1/posts/{tagged_id}/publish", headers=owner.headers)

    untagged = await _create_and_publish(client, owner, "feed-blog-tag", "senza-tag")

    res = await client.get("/api/v1/feed/posts?tag=cucina")
    ids = [p["id"] for p in res.json()]
    assert tagged_id in ids
    assert untagged["id"] not in ids


async def test_trending_tags_counts_recent_published_posts(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user("feed-owner-trending")
    await client.post(
        "/api/v1/blogs", json={"slug": "feed-blog-trending", "title": "x"}, headers=owner.headers
    )

    for i in range(3):
        res = await client.post(
            "/api/v1/blogs/feed-blog-trending/posts",
            json={"slug": f"trend-{i}", "title": "x", "content": "y", "tags": ["montagna"]},
            headers=owner.headers,
        )
        await client.post(f"/api/v1/posts/{res.json()['id']}/publish", headers=owner.headers)

    other_res = await client.post(
        "/api/v1/blogs/feed-blog-trending/posts",
        json={"slug": "trend-other", "title": "x", "content": "y", "tags": ["mare"]},
        headers=owner.headers,
    )
    await client.post(f"/api/v1/posts/{other_res.json()['id']}/publish", headers=owner.headers)

    res = await client.get("/api/v1/feed/trending?days=7&limit=5")
    assert res.status_code == 200
    by_tag = {row["tag"]: row["post_count"] for row in res.json()}
    assert by_tag["montagna"] == 3
    assert by_tag["mare"] == 1
    # dal più frequente
    assert res.json()[0]["tag"] == "montagna"
