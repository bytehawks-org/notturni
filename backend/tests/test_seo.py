from collections.abc import Callable

from httpx import AsyncClient

from tests.conftest import AuthedUser


async def _create_blog(client: AsyncClient, owner: AuthedUser, slug: str) -> str:
    res = await client.post("/api/v1/blogs", json={"slug": slug, "title": "x"}, headers=owner.headers)
    assert res.status_code == 201
    return slug


async def _create_and_publish_post(client: AsyncClient, owner: AuthedUser, blog_slug: str, post_slug: str) -> str:
    create_res = await client.post(
        f"/api/v1/blogs/{blog_slug}/posts",
        json={"slug": post_slug, "title": "x", "content": "y"},
        headers=owner.headers,
    )
    post_id = create_res.json()["id"]
    await client.post(f"/api/v1/posts/{post_id}/publish", headers=owner.headers)
    return post_id


async def test_crawl_directives_excludes_opted_out_blog_entirely(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user("owner-seo-blog")
    slug = await _create_blog(client, owner, "blog-seo-escluso")
    await _create_and_publish_post(client, owner, slug, "post-seo-escluso")

    await client.patch(
        f"/api/v1/blogs/{slug}",
        json={"search_indexing_enabled": False, "ai_crawling_enabled": False},
        headers=owner.headers,
    )

    res = await client.get("/api/v1/seo/crawl-directives")
    assert res.status_code == 200
    body = res.json()
    assert f"/{slug}" in body["search_disallow"]
    assert f"/{slug}" in body["ai_disallow"]
    # il blog è escluso per intero: nessun path di post aggiunto separatamente
    assert f"/{slug}/post-seo-escluso" not in body["search_disallow"]
    assert len(body["ai_user_agents"]) > 0


async def test_crawl_directives_post_level_ai_disallow_includes_search_disallow(
    client: AsyncClient, make_user: Callable
) -> None:
    """ai_disallow è sempre un superset di search_disallow: un gruppo
    user-agent specifico (es. GPTBot) in robots.txt sostituisce interamente
    `User-agent: *` per quel bot, non lo integra — vedi app/domain/seo.py."""
    owner: AuthedUser = await make_user("owner-seo-post")
    slug = await _create_blog(client, owner, "blog-seo-post")
    post_id = await _create_and_publish_post(client, owner, slug, "post-seo-solo-motori")

    await client.patch(
        f"/api/v1/posts/{post_id}",
        json={"search_indexing_enabled": False},
        headers=owner.headers,
    )

    res = await client.get("/api/v1/seo/crawl-directives")
    body = res.json()
    permalink = f"/{slug}/post-seo-solo-motori"
    assert permalink in body["search_disallow"]
    assert permalink in body["ai_disallow"]


async def test_crawl_directives_public_blog_default_not_disallowed(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user("owner-seo-default")
    slug = await _create_blog(client, owner, "blog-seo-default")
    await _create_and_publish_post(client, owner, slug, "post-seo-default")

    res = await client.get("/api/v1/seo/crawl-directives")
    body = res.json()
    assert f"/{slug}" not in body["search_disallow"]
    assert f"/{slug}/post-seo-default" not in body["search_disallow"]
    assert f"/{slug}" not in body["ai_disallow"]


async def test_sitemap_entries_excludes_noindex_content(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner-seo-sitemap")
    slug = await _create_blog(client, owner, "blog-seo-sitemap")
    await _create_and_publish_post(client, owner, slug, "post-seo-sitemap-visible")
    excluded_post_id = await _create_and_publish_post(client, owner, slug, "post-seo-sitemap-escluso")
    await client.patch(
        f"/api/v1/posts/{excluded_post_id}",
        json={"search_indexing_enabled": False},
        headers=owner.headers,
    )

    res = await client.get("/api/v1/seo/sitemap-entries")
    assert res.status_code == 200
    body = res.json()
    assert any(b["slug"] == slug for b in body["blogs"])
    permalinks = {p["permalink"] for p in body["posts"]}
    assert f"/{slug}/post-seo-sitemap-visible" in permalinks
    assert f"/{slug}/post-seo-sitemap-escluso" not in permalinks
