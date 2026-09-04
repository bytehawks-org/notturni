from collections.abc import Callable

from httpx import AsyncClient

from tests.conftest import AuthedUser


async def _published_post(
    client: AsyncClient, owner: AuthedUser, blog_slug: str, content: str = "x" * 200
) -> str:
    await client.post("/api/v1/blogs", json={"slug": blog_slug, "title": "x"}, headers=owner.headers)
    post_res = await client.post(
        f"/api/v1/blogs/{blog_slug}/posts",
        json={"slug": "post-frammenti", "title": "x", "content": content},
        headers=owner.headers,
    )
    post_id = post_res.json()["id"]
    await client.post(f"/api/v1/posts/{post_id}/publish", headers=owner.headers)
    return post_id


async def test_save_fragment(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner-f1")
    reader: AuthedUser = await make_user("reader-f1")
    post_id = await _published_post(client, owner, "blog-frammenti-1")

    res = await client.post(
        f"/api/v1/posts/{post_id}/fragments", json={"text": "un pezzo di testo"}, headers=reader.headers
    )
    assert res.status_code == 201, res.text
    assert res.json()["post_id"] == post_id
    assert res.json()["text"] == "un pezzo di testo"

    mine = await client.get(f"/api/v1/posts/{post_id}/fragments", headers=reader.headers)
    assert len(mine.json()) == 1

    # non visibile ad altri utenti: ognuno vede solo i propri frammenti
    owner_view = await client.get(f"/api/v1/posts/{post_id}/fragments", headers=owner.headers)
    assert owner_view.json() == []


async def test_saving_same_fragment_twice_is_idempotent(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner-f2")
    reader: AuthedUser = await make_user("reader-f2")
    post_id = await _published_post(client, owner, "blog-frammenti-2")

    first = await client.post(
        f"/api/v1/posts/{post_id}/fragments", json={"text": "stesso frammento"}, headers=reader.headers
    )
    second = await client.post(
        f"/api/v1/posts/{post_id}/fragments", json={"text": "stesso frammento"}, headers=reader.headers
    )
    assert first.json()["id"] == second.json()["id"]

    mine = await client.get(f"/api/v1/posts/{post_id}/fragments", headers=reader.headers)
    assert len(mine.json()) == 1


async def test_fragment_over_15_percent_rejected(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner-f3")
    reader: AuthedUser = await make_user("reader-f3")
    # post di 100 caratteri: il 15% sono 15 caratteri
    post_id = await _published_post(client, owner, "blog-frammenti-3", content="a" * 100)

    too_long = await client.post(
        f"/api/v1/posts/{post_id}/fragments", json={"text": "a" * 16}, headers=reader.headers
    )
    assert too_long.status_code == 400

    ok = await client.post(
        f"/api/v1/posts/{post_id}/fragments", json={"text": "a" * 15}, headers=reader.headers
    )
    assert ok.status_code == 201


async def test_cannot_save_fragment_on_draft(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner-f4")
    reader: AuthedUser = await make_user("reader-f4")
    await client.post("/api/v1/blogs", json={"slug": "blog-frammenti-4", "title": "x"}, headers=owner.headers)
    post_res = await client.post(
        "/api/v1/blogs/blog-frammenti-4/posts",
        json={"slug": "bozza", "title": "x", "content": "y" * 100},
        headers=owner.headers,
    )
    post_id = post_res.json()["id"]

    res = await client.post(
        f"/api/v1/posts/{post_id}/fragments", json={"text": "y" * 5}, headers=reader.headers
    )
    assert res.status_code == 404


async def test_collection_and_delete(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner-f5")
    reader: AuthedUser = await make_user("reader-f5")
    other: AuthedUser = await make_user("other-f5")
    post_id = await _published_post(client, owner, "blog-frammenti-5")

    create_res = await client.post(
        f"/api/v1/posts/{post_id}/fragments", json={"text": "da raccogliere"}, headers=reader.headers
    )
    fragment_id = create_res.json()["id"]

    collection = await client.get("/api/v1/users/me/fragments", headers=reader.headers)
    assert collection.status_code == 200
    entries = collection.json()
    assert len(entries) == 1
    assert entries[0]["text"] == "da raccogliere"
    assert entries[0]["post_title"] == "x"
    assert entries[0]["author_display_name"] == "owner-f5"
    assert entries[0]["permalink"].startswith("/blog-frammenti-5/")

    # nessun frammento salvato da altri: la raccolta resta personale
    other_collection = await client.get("/api/v1/users/me/fragments", headers=other.headers)
    assert other_collection.json() == []

    # solo il proprietario del frammento può rimuoverlo
    forbidden = await client.delete(f"/api/v1/fragments/{fragment_id}", headers=other.headers)
    assert forbidden.status_code == 404

    delete_res = await client.delete(f"/api/v1/fragments/{fragment_id}", headers=reader.headers)
    assert delete_res.status_code == 204

    after = await client.get("/api/v1/users/me/fragments", headers=reader.headers)
    assert after.json() == []
