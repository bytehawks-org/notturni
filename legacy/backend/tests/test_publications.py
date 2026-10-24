"""todo/PUBLICATIONS.md, todo/UX_REDESIGN.md B9: pubblicazioni e capitoli."""

from collections.abc import Callable

from httpx import AsyncClient

from tests.conftest import AuthedUser
from tests.test_overview import _blog


async def _post(client: AsyncClient, owner: AuthedUser, blog: str, slug: str, pub_id: str | None, publish: bool = True) -> dict:
    res = await client.post(f"/api/v1/blogs/{blog}/posts", json={"slug": slug, "title": slug, "content": "parole " * 50, "publication_id": pub_id}, headers=owner.headers)
    assert res.status_code == 201, res.text
    post = res.json()
    if publish:
        post = (await client.post(f"/api/v1/posts/{post['id']}/publish", headers=owner.headers)).json()
    return post


async def test_publication_index_order_and_visibility(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("pub-owner")
    await _blog(client, owner, "pub-blog")
    assert (await client.post("/api/v1/blogs/pub-blog/publications", json={"name": "Bad Name", "title": "x"}, headers=owner.headers)).status_code == 400
    created = await client.post("/api/v1/blogs/pub-blog/publications", json={"name": "margini", "title": "A Grammar of Margins", "description": "saggi"}, headers=owner.headers)
    assert created.status_code == 201, created.text
    pub = created.json()
    assert (await client.post("/api/v1/blogs/pub-blog/publications", json={"name": "margini", "title": "dup"}, headers=owner.headers)).status_code == 409

    # nessun capitolo pubblicato: invisibile ai lettori, visibile a chi scrive
    assert (await client.get("/api/v1/blogs/pub-blog/publications")).json() == []
    assert (await client.get("/api/v1/blogs/pub-blog/publications/margini")).status_code == 404
    assert len((await client.get("/api/v1/blogs/pub-blog/publications", headers=owner.headers)).json()) == 1

    c1 = await _post(client, owner, "pub-blog", "primo", pub["id"])
    c2 = await _post(client, owner, "pub-blog", "secondo", pub["id"])
    draft = await _post(client, owner, "pub-blog", "bozza", pub["id"], publish=False)
    assert c1["publication"]["name"] == "margini"

    public = await client.get("/api/v1/blogs/pub-blog/publications/margini")
    assert public.status_code == 200, public.text
    data = public.json()
    assert [c["slug"] for c in data["chapters"]] == ["primo", "secondo"]
    assert data["chapters"][0]["n"] == 1 and data["chapters"][0]["reading_minutes"] == 1
    assert data["chapters_total"] == 3 and data["chapters_published"] == 2
    listed = (await client.get("/api/v1/blogs/pub-blog/publications")).json()
    assert listed[0]["chapters_published"] == 2

    mine = await client.get("/api/v1/blogs/pub-blog/publications/margini", headers=owner.headers)
    assert [c["slug"] for c in mine.json()["chapters"]] == ["primo", "secondo", "bozza"]
    assert mine.json()["chapters"][2]["is_public"] is False

    # ordine esplicito: secondo, primo; la bozza in coda
    ordered = await client.put(f"/api/v1/blogs/pub-blog/publications/{pub['id']}/order", json={"post_ids": [c2["id"], c1["id"]]}, headers=owner.headers)
    assert ordered.status_code == 200, ordered.text
    assert [c["slug"] for c in ordered.json()["chapters"]] == ["secondo", "primo", "bozza"]
    assert (await client.put(f"/api/v1/blogs/pub-blog/publications/{pub['id']}/order", json={"post_ids": [draft["id"], "00000000-0000-0000-0000-000000000000"]}, headers=owner.headers)).status_code == 400

    # togliere il post dalla pubblicazione azzera l'ordine
    moved = await client.patch(f"/api/v1/posts/{c2['id']}", json={"publication_id": None}, headers=owner.headers)
    assert moved.status_code == 200 and moved.json()["publication"] is None and moved.json()["chapter_order"] is None
    assert [c["slug"] for c in (await client.get("/api/v1/blogs/pub-blog/publications/margini")).json()["chapters"]] == ["primo"]

    upd = await client.patch("/api/v1/blogs/pub-blog/publications/margini", json={"title": "Nuovo titolo", "name": "margini-2"}, headers=owner.headers)
    assert upd.status_code == 200 and upd.json()["name"] == "margini-2"
    assert (await client.delete("/api/v1/blogs/pub-blog/publications/margini-2", headers=owner.headers)).status_code == 204
    detail = await client.get(f"/api/v1/posts/{c1['id']}", headers=owner.headers)
    assert detail.json()["publication"] is None


async def test_publication_id_must_belong_to_blog(client: AsyncClient, make_user: Callable) -> None:
    a: AuthedUser = await make_user("pub-a")
    b: AuthedUser = await make_user("pub-b")
    await _blog(client, a, "pub-blog-a")
    await _blog(client, b, "pub-blog-b")
    other = (await client.post("/api/v1/blogs/pub-blog-b/publications", json={"name": "serie", "title": "S"}, headers=b.headers)).json()
    res = await client.post("/api/v1/blogs/pub-blog-a/posts", json={"slug": "x", "title": "x", "content": "y", "publication_id": other["id"]}, headers=a.headers)
    assert res.status_code == 400
