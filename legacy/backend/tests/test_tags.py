import pytest
from collections.abc import Callable

from httpx import AsyncClient

from app.domain.tags import extract_hashtags, normalize_tag, resolve_tags
from tests.conftest import AuthedUser


def test_normalize_tag_lowercases_and_strips_hash() -> None:
    assert normalize_tag("#Poesia") == "poesia"
    assert normalize_tag("  Viaggi  ") == "viaggi"
    assert normalize_tag("vita_privata") == "vita-privata"


def test_normalize_tag_rejects_invalid() -> None:
    with pytest.raises(ValueError):
        normalize_tag("")
    with pytest.raises(ValueError):
        normalize_tag("a" * 31)
    with pytest.raises(ValueError):
        normalize_tag("tag con spazi assurdi!!")


def test_extract_hashtags_dedupes_and_preserves_order() -> None:
    content = "Un post su #Viaggi e ancora #viaggi, ma anche #cibo."
    assert extract_hashtags(content) == ["viaggi", "cibo"]


def test_extract_hashtags_ignores_malformed_silently() -> None:
    content = "prezzo di 100# non è un tag, ma #vero sì."
    assert extract_hashtags(content) == ["vero"]


def test_resolve_tags_merges_manual_and_inline() -> None:
    manual, effective = resolve_tags(["Poesia"], "Testo con #cibo e #poesia ripetuto.")
    assert manual == ["poesia"]
    assert effective == ["poesia", "cibo"]


def test_resolve_tags_rejects_more_than_five() -> None:
    with pytest.raises(ValueError):
        resolve_tags(["uno", "due", "tre", "quattro", "cinque"], "testo con #sei")


async def _create_blog(client: AsyncClient, owner: AuthedUser, slug: str) -> str:
    res = await client.post("/api/v1/blogs", json={"slug": slug, "title": "x"}, headers=owner.headers)
    assert res.status_code == 201
    return slug


async def test_create_post_with_manual_and_inline_tags(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner-tags")
    slug = await _create_blog(client, owner, "blog-tags-test")

    res = await client.post(
        f"/api/v1/blogs/{slug}/posts",
        json={
            "slug": "post-tag",
            "title": "x",
            "content": "Un post su #viaggi e #cibo.",
            "tags": ["Poesia"],
        },
        headers=owner.headers,
    )
    assert res.status_code == 201
    assert res.json()["manual_tags"] == ["poesia"]
    assert res.json()["tags"] == ["poesia", "viaggi", "cibo"]


async def test_create_post_rejects_more_than_five_tags(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner-tags-limit")
    slug = await _create_blog(client, owner, "blog-tags-limit-test")

    res = await client.post(
        f"/api/v1/blogs/{slug}/posts",
        json={
            "slug": "post-troppi-tag",
            "title": "x",
            "content": "Testo con #sei tag extra.",
            "tags": ["uno", "due", "tre", "quattro", "cinque"],
        },
        headers=owner.headers,
    )
    assert res.status_code == 400


async def test_update_content_keeps_manual_tags_and_refreshes_inline(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user("owner-tags-update")
    slug = await _create_blog(client, owner, "blog-tags-update-test")

    create_res = await client.post(
        f"/api/v1/blogs/{slug}/posts",
        json={"slug": "post-update-tag", "title": "x", "content": "Testo con #vecchio.", "tags": ["manuale"]},
        headers=owner.headers,
    )
    post_id = create_res.json()["id"]
    assert create_res.json()["tags"] == ["manuale", "vecchio"]

    # cambia solo il contenuto, senza toccare il campo tags: il tag manuale
    # resta, l'hashtag nel testo si aggiorna
    update_res = await client.patch(
        f"/api/v1/posts/{post_id}", json={"content": "Testo con #nuovo."}, headers=owner.headers
    )
    assert update_res.status_code == 200
    assert update_res.json()["manual_tags"] == ["manuale"]
    assert update_res.json()["tags"] == ["manuale", "nuovo"]

    # sostituisce esplicitamente il campo dedicato
    replace_res = await client.patch(
        f"/api/v1/posts/{post_id}", json={"tags": ["altro"]}, headers=owner.headers
    )
    assert replace_res.json()["manual_tags"] == ["altro"]
    assert replace_res.json()["tags"] == ["altro", "nuovo"]
