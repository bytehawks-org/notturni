"""GET /blogs/{slug}/media-bibliography (CLAUDE.md #4): copre in particolare
`is_sensitive`, separato da `categories` perché un'immagine segnalata dalla
sola automoderazione (o dal modal senza una categoria specifica) ha
`categories` vuoto pur essendo sensibile — bug corretto a valle nella
bibliografia media pubblica del blog (mostrava l'immagine sfocata solo nella
pagina del post, non nella griglia media)."""

from collections.abc import Callable

from httpx import AsyncClient

from tests.conftest import AuthedUser


async def _blog(client: AsyncClient, owner: AuthedUser, slug: str) -> None:
    res = await client.post("/api/v1/blogs", json={"slug": slug, "title": slug}, headers=owner.headers)
    assert res.status_code == 201, res.text


async def _publish(client: AsyncClient, owner: AuthedUser, blog_slug: str, slug: str, content: str) -> None:
    res = await client.post(
        f"/api/v1/blogs/{blog_slug}/posts",
        json={"slug": slug, "title": slug, "content": content},
        headers=owner.headers,
    )
    assert res.status_code == 201, res.text
    pub = await client.post(f"/api/v1/posts/{res.json()['id']}/publish", headers=owner.headers)
    assert pub.status_code == 200, pub.text


async def test_media_bibliography_marks_automod_sensitive_without_category(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user("media-biblio-owner")
    await _blog(client, owner, "media-biblio")

    await _publish(
        client,
        owner,
        "media-biblio",
        "post-automod",
        '![alt automod](https://example.com/automod.png "sensitive")',
    )
    await _publish(
        client,
        owner,
        "media-biblio",
        "post-chosen",
        '![alt scelta](https://example.com/scelta.png "sensitive:nudity")',
    )
    await _publish(
        client,
        owner,
        "media-biblio",
        "post-clean",
        "![alt pulita](https://example.com/pulita.png)",
    )

    res = await client.get("/api/v1/blogs/media-biblio/media-bibliography")
    assert res.status_code == 200, res.text
    by_url = {e["url"]: e for e in res.json()}

    automod = by_url["https://example.com/automod.png"]
    assert automod["categories"] == []
    assert automod["is_sensitive"] is True

    chosen = by_url["https://example.com/scelta.png"]
    assert chosen["categories"] == ["nudity"]
    assert chosen["is_sensitive"] is True

    clean = by_url["https://example.com/pulita.png"]
    assert clean["categories"] == []
    assert clean["is_sensitive"] is False
