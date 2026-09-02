from collections.abc import Callable

import pytest
from httpx import AsyncClient

from app.domain.notes import NoteInput, normalize_notes
from tests.conftest import AuthedUser


def test_normalize_notes_orders_trims_and_rejects_duplicates() -> None:
    out = normalize_notes([NoteInput(2, "  seconda  "), NoteInput(1, "prima")])
    assert [(n.idx, n.content) for n in out] == [(1, "prima"), (2, "seconda")]

    with pytest.raises(ValueError):
        normalize_notes([NoteInput(1, "a"), NoteInput(1, "b")])
    with pytest.raises(ValueError):
        normalize_notes([NoteInput(0, "a")])
    with pytest.raises(ValueError):
        normalize_notes([NoteInput(1, "   ")])


async def _blog(client: AsyncClient, owner: AuthedUser, slug: str) -> None:
    res = await client.post("/api/v1/blogs", json={"slug": slug, "title": "T"}, headers=owner.headers)
    assert res.status_code == 201, res.text


async def test_post_notes_crud_roundtrip(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("note-owner")
    await _blog(client, owner, "blog-note")

    created = await client.post(
        "/api/v1/blogs/blog-note/posts",
        json={
            "slug": "p",
            "title": "t",
            "content": "Testo con riferimento [1](#nota-1).",
            "notes": [
                {"idx": 2, "content": "La seconda nota."},
                {"idx": 1, "content": "La *prima* nota."},
            ],
        },
        headers=owner.headers,
    )
    assert created.status_code == 201, created.text
    assert created.json()["notes"] == [
        {"idx": 1, "content": "La *prima* nota."},
        {"idx": 2, "content": "La seconda nota."},
    ]
    post_id = created.json()["id"]

    # update: assente lascia invariato
    untouched = await client.patch(
        f"/api/v1/posts/{post_id}", json={"title": "t2"}, headers=owner.headers
    )
    assert len(untouched.json()["notes"]) == 2

    # update: lista sostituisce
    replaced = await client.patch(
        f"/api/v1/posts/{post_id}",
        json={"notes": [{"idx": 1, "content": "Nota rivista."}]},
        headers=owner.headers,
    )
    assert replaced.json()["notes"] == [{"idx": 1, "content": "Nota rivista."}]

    # update: [] azzera
    cleared = await client.patch(
        f"/api/v1/posts/{post_id}", json={"notes": []}, headers=owner.headers
    )
    assert cleared.json()["notes"] == []

    # validazione: idx duplicato → 400
    bad = await client.patch(
        f"/api/v1/posts/{post_id}",
        json={"notes": [{"idx": 1, "content": "a"}, {"idx": 1, "content": "b"}]},
        headers=owner.headers,
    )
    assert bad.status_code == 400


async def test_translation_has_its_own_notes(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("note-tr-owner")
    await _blog(client, owner, "blog-note-tr")
    original = await client.post(
        "/api/v1/blogs/blog-note-tr/posts",
        json={"slug": "p", "title": "t", "content": "x", "notes": [{"idx": 1, "content": "IT"}]},
        headers=owner.headers,
    )
    tr = await client.post(
        f"/api/v1/posts/{original.json()['id']}/translations",
        json={"slug": "p-en", "locale": "en", "title": "t", "content": "x",
              "notes": [{"idx": 1, "content": "EN"}]},
        headers=owner.headers,
    )
    assert tr.json()["notes"] == [{"idx": 1, "content": "EN"}]
    # l'originale resta con la sua
    again = await client.get(f"/api/v1/posts/{original.json()['id']}", headers=owner.headers)
    assert again.json()["notes"] == [{"idx": 1, "content": "IT"}]


async def test_blog_bibliography_aggregates_and_dedupes(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user("biblio-owner")
    await _blog(client, owner, "blog-biblio")

    async def publish_with_notes(slug: str, notes: list[dict]) -> None:
        res = await client.post(
            "/api/v1/blogs/blog-biblio/posts",
            json={"slug": slug, "title": slug, "content": "c", "notes": notes},
            headers=owner.headers,
        )
        await client.post(f"/api/v1/posts/{res.json()['id']}/publish", headers=owner.headers)

    await publish_with_notes("post-a", [
        {"idx": 1, "content": "Fonte condivisa"},
        {"idx": 2, "content": "Solo di A"},
    ])
    await publish_with_notes("post-b", [{"idx": 1, "content": "  fonte   condivisa "}])

    # una bozza non deve comparire
    draft = await client.post(
        "/api/v1/blogs/blog-biblio/posts",
        json={"slug": "bozza", "title": "b", "content": "c", "notes": [{"idx": 1, "content": "Da bozza"}]},
        headers=owner.headers,
    )
    assert draft.status_code == 201

    res = await client.get("/api/v1/blogs/blog-biblio/bibliography")
    assert res.status_code == 200
    body = res.json()
    contents = [e["content"] for e in body]
    assert "Da bozza" not in contents
    assert "Solo di A" in contents

    shared = next(e for e in body if "condivisa" in e["content"].casefold())
    slugs = sorted(c["post_slug"] for c in shared["citations"])
    assert slugs == ["post-a", "post-b"]


async def test_bibliography_follows_blog_visibility(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user("biblio-vis-owner")
    stranger: AuthedUser = await make_user("biblio-vis-stranger")
    await client.post(
        "/api/v1/blogs",
        json={"slug": "blog-biblio-priv", "title": "T", "visibility": "private"},
        headers=owner.headers,
    )
    res = await client.post(
        "/api/v1/blogs/blog-biblio-priv/posts",
        json={"slug": "p", "title": "t", "content": "c", "notes": [{"idx": 1, "content": "x"}]},
        headers=owner.headers,
    )
    await client.post(f"/api/v1/posts/{res.json()['id']}/publish", headers=owner.headers)

    assert (await client.get("/api/v1/blogs/blog-biblio-priv/bibliography")).status_code == 404
    assert (
        await client.get("/api/v1/blogs/blog-biblio-priv/bibliography", headers=stranger.headers)
    ).status_code == 404
    assert (
        await client.get("/api/v1/blogs/blog-biblio-priv/bibliography", headers=owner.headers)
    ).status_code == 200
