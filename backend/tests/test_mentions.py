from collections.abc import Callable

from httpx import AsyncClient

from app.domain.mentions import extract_mentions
from tests.conftest import AuthedUser


def test_extract_mentions_dedupes_and_preserves_order() -> None:
    text = "ciao @anna e @bruno, di nuovo @anna. email non-mention@example.com resta fuori"
    assert extract_mentions(text) == ["anna", "bruno"]


def test_extract_mentions_matches_username_format() -> None:
    # `@-nope` non è una menzione valida: lo username non può iniziare con `-`
    assert extract_mentions("@gio-vanni e @gio_99 ok, @-nope no") == ["gio-vanni", "gio_99"]
    assert extract_mentions("nessuna menzione qui") == []
    assert extract_mentions("email me@dominio.it non conta come menzione") == []


async def test_post_out_carries_mentions_enabled(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("mention-owner")
    await client.post("/api/v1/blogs", json={"slug": "blog-menzioni", "title": "x"}, headers=owner.headers)

    created = await client.post(
        "/api/v1/blogs/blog-menzioni/posts",
        json={"slug": "p", "title": "t", "content": "ciao @qualcuno"},
        headers=owner.headers,
    )
    assert created.json()["mentions_enabled"] is True

    await client.patch(
        "/api/v1/blogs/blog-menzioni", json={"mentions_enabled": False}, headers=owner.headers
    )
    blog = await client.get("/api/v1/blogs/blog-menzioni")
    assert blog.json()["mentions_enabled"] is False

    fetched = await client.get(f"/api/v1/posts/{created.json()['id']}", headers=owner.headers)
    assert fetched.json()["mentions_enabled"] is False


async def test_mentionable_users_endpoint(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("mu-owner")
    collab: AuthedUser = await make_user("mu-collab")
    follower: AuthedUser = await make_user("mu-follower")
    stranger: AuthedUser = await make_user("mu-stranger")
    await client.post("/api/v1/blogs", json={"slug": "blog-mu", "title": "x"}, headers=owner.headers)

    inv = await client.post(
        "/api/v1/blogs/blog-mu/invitations",
        json={"username": "mu-collab", "role": "co_autore"},
        headers=owner.headers,
    )
    await client.post(
        f"/api/v1/blogs/received-invitations/{inv.json()['id']}/accept", headers=collab.headers
    )
    await client.post("/api/v1/blogs/blog-mu/follow", headers=follower.headers)

    res = await client.get("/api/v1/blogs/blog-mu/mentionable-users", headers=owner.headers)
    assert res.status_code == 200
    usernames = {u["username"] for u in res.json()}
    assert {"mu-owner", "mu-collab", "mu-follower"} <= usernames
    assert "mu-stranger" not in usernames

    # filtro per prefisso
    filtered = await client.get(
        "/api/v1/blogs/blog-mu/mentionable-users?q=mu-coll", headers=owner.headers
    )
    assert [u["username"] for u in filtered.json()] == ["mu-collab"]

    # serve accesso in scrittura
    forbidden = await client.get(
        "/api/v1/blogs/blog-mu/mentionable-users", headers=stranger.headers
    )
    assert forbidden.status_code == 403

    # disattivando le menzioni sul blog, nessun suggerimento
    await client.patch(
        "/api/v1/blogs/blog-mu", json={"mentions_enabled": False}, headers=owner.headers
    )
    disabled = await client.get(
        "/api/v1/blogs/blog-mu/mentionable-users", headers=owner.headers
    )
    assert disabled.json() == []
