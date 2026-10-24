"""todo/UX_REDESIGN.md B8: note del blog come entità, duplicati, merge, BibTeX."""

from collections.abc import Callable

from httpx import AsyncClient

from app.domain.blog_notes import guess_kind, normalize_note, parse_bibtex, to_bibtex
from tests.conftest import AuthedUser
from tests.test_overview import _blog


def test_normalize_and_guess() -> None:
    assert normalize_note("Zerby, Chuck. The Devil's Details (2002).") == "zerby chuck the devils details 2002"
    assert guess_kind("Zerby, C. The Devil's Details. 2002.", None) == "book"
    assert guess_kind("vedi https://example.org/x", "https://example.org/x") == "web"
    assert guess_kind("Rossi 2020, doi", "https://doi.org/10.1000/xyz") == "article"
    parsed = parse_bibtex('@book{z2002,\n  author = {Zerby, Chuck},\n  title = {The Devil\'s Details},\n  year = {2002}\n}\n@misc{w,\n  note = {Una pagina},\n  url = {https://example.org}\n}')
    assert parsed[0]["kind"] == "book" and "Zerby, Chuck. The Devil's Details. 2002." == parsed[0]["content"]
    assert parsed[1]["kind"] == "web" and parsed[1]["url"] == "https://example.org"


async def test_post_notes_feed_library_with_usage_and_duplicates(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("bn-owner")
    await _blog(client, owner, "bn-blog")
    p1 = await client.post(
        "/api/v1/blogs/bn-blog/posts",
        json={"slug": "uno", "title": "Uno", "content": "a [1](#nota-1)", "notes": [{"idx": 1, "content": "Zerby, Chuck. The Devil's Details. 2002."}]},
        headers=owner.headers,
    )
    assert p1.status_code == 201, p1.text
    p2 = await client.post(
        "/api/v1/blogs/bn-blog/posts",
        json={"slug": "due", "title": "Due", "content": "b [1](#nota-1) [2](#nota-2)", "notes": [{"idx": 1, "content": "Zerby, Chuck. The Devil's Details. 2002."}, {"idx": 2, "content": "Zerby, Chuck. The Devils Details (Invisible Cities, 2002)"}]},
        headers=owner.headers,
    )
    assert p2.status_code == 201, p2.text

    notes = await client.get("/api/v1/blogs/bn-blog/notes", headers=owner.headers)
    assert notes.status_code == 200
    items = notes.json()
    assert len(items) == 2
    canonical = next(n for n in items if n["content"].endswith("2002."))
    variant = next(n for n in items if n["content"].endswith("2002)"))
    assert {u["post_slug"] for u in canonical["used_in"]} == {"uno", "due"}
    assert canonical["kind"] == "book"
    assert variant["id"] in canonical["possible_duplicates"]

    merged = await client.post(f"/api/v1/blogs/bn-blog/notes/{variant['id']}/merge", json={"into_id": canonical["id"]}, headers=owner.headers)
    assert merged.status_code == 200, merged.text
    assert len(merged.json()["used_in"]) == 3
    detail = await client.get("/api/v1/posts/" + p2.json()["id"], headers=owner.headers)
    assert all(n["content"].endswith("2002.") for n in detail.json()["notes"])
    assert len((await client.get("/api/v1/blogs/bn-blog/notes", headers=owner.headers)).json()) == 1

    upd = await client.patch(f"/api/v1/blogs/bn-blog/notes/{canonical['id']}", json={"kind": "article", "url": "https://doi.org/10.1/x", "content": "Zerby (2002)"}, headers=owner.headers)
    assert upd.status_code == 200 and upd.json()["kind"] == "article"
    detail = await client.get("/api/v1/posts/" + p1.json()["id"], headers=owner.headers)
    assert detail.json()["notes"][0]["content"] == "Zerby (2002)"
    await client.post(f"/api/v1/posts/{p1.json()['id']}/publish", headers=owner.headers)
    bib = await client.get("/api/v1/blogs/bn-blog/bibliography")
    assert bib.json()[0]["kind"] == "article" and bib.json()[0]["url"] == "https://doi.org/10.1/x"

    assert (await client.delete(f"/api/v1/blogs/bn-blog/notes/{canonical['id']}", headers=owner.headers)).status_code == 409


async def test_manual_notes_import_export(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("bx-owner")
    reader: AuthedUser = await make_user("bx-reader")
    await _blog(client, owner, "bx-blog")
    created = await client.post("/api/v1/blogs/bx-blog/notes", json={"content": "Nota a mano", "kind": "note"}, headers=owner.headers)
    assert created.status_code == 201
    assert (await client.post("/api/v1/blogs/bx-blog/notes", json={"content": "x", "kind": "boh"}, headers=owner.headers)).status_code == 400
    assert (await client.get("/api/v1/blogs/bx-blog/notes", headers=reader.headers)).status_code == 403

    imported = await client.post(
        "/api/v1/blogs/bx-blog/notes/import",
        json={"bibtex": "@book{k,\n  author = {Calvino, Italo},\n  title = {Le città invisibili},\n  year = {1972}\n}"},
        headers=owner.headers,
    )
    assert imported.status_code == 201 and imported.json()[0]["kind"] == "book"
    again = await client.post("/api/v1/blogs/bx-blog/notes/import", json={"bibtex": "@book{k,\n  author = {Calvino, Italo},\n  title = {Le città invisibili},\n  year = {1972}\n}"}, headers=owner.headers)
    assert again.status_code == 201 and again.json() == []

    exp = await client.get("/api/v1/blogs/bx-blog/notes/export.bib", headers=owner.headers)
    assert exp.status_code == 200 and "@book{" in exp.text and "Calvino" in exp.text
    assert (await client.delete(f"/api/v1/blogs/bx-blog/notes/{created.json()['id']}", headers=owner.headers)).status_code == 204
    assert to_bibtex([]) == ""
