from collections.abc import Callable

import pytest
from httpx import AsyncClient
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.domain.notes import NoteInput, normalize_notes
from app.models.blog_note import BlogNote
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


def test_normalize_notes_optional_fields_trimmed_and_blanked() -> None:
    out = normalize_notes(
        [NoteInput(1, "testo", title="  Il Titolo  ", author=" ", isbn="978-1", doi=None, page="42")]
    )
    assert out == [
        NoteInput(idx=1, content="testo", title="Il Titolo", author=None, isbn="978-1", doi=None, page="42")
    ]

    with pytest.raises(ValueError):
        normalize_notes([NoteInput(1, "testo", title="x" * 301)])
    with pytest.raises(ValueError):
        normalize_notes([NoteInput(1, "testo", isbn="1" * 33)])


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
    empty_fields = {"title": None, "author": None, "isbn": None, "doi": None, "page": None, "kind": None, "source": None, "issued": None, "url": None}
    assert created.json()["notes"] == [
        {"idx": 1, "content": "La *prima* nota.", **empty_fields},
        {"idx": 2, "content": "La seconda nota.", **empty_fields},
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
    assert replaced.json()["notes"] == [{"idx": 1, "content": "Nota rivista.", **empty_fields}]

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


async def test_post_note_structured_fields_roundtrip_and_library_propagation(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user("note-struct-owner")
    await _blog(client, owner, "blog-note-struct")

    created = await client.post(
        "/api/v1/blogs/blog-note-struct/posts",
        json={
            "slug": "p",
            "title": "t",
            "content": "c",
            "notes": [
                {
                    "idx": 1,
                    "content": "Una citazione strutturata.",
                    "title": "  Il Titolo del Testo  ",
                    "author": "Autrice Autrice",
                    "isbn": "978-3-16-148410-0",
                    "doi": "10.1000/xyz123",
                    "page": "42",
                }
            ],
        },
        headers=owner.headers,
    )
    assert created.status_code == 201, created.text
    note_out = created.json()["notes"][0]
    assert note_out["title"] == "Il Titolo del Testo"
    assert note_out["author"] == "Autrice Autrice"
    assert note_out["isbn"] == "978-3-16-148410-0"
    assert note_out["doi"] == "10.1000/xyz123"
    assert note_out["page"] == "42"

    await client.post(f"/api/v1/posts/{created.json()['id']}/publish", headers=owner.headers)

    biblio = await client.get("/api/v1/blogs/blog-note-struct/bibliography")
    assert biblio.status_code == 200
    entry = biblio.json()[0]
    assert entry["title"] == "Il Titolo del Testo"
    assert entry["author"] == "Autrice Autrice"
    assert entry["isbn"] == "978-3-16-148410-0"
    assert entry["doi"] == "10.1000/xyz123"
    assert entry["page"] == "42"

    # B8: propagato anche alla libreria note del blog
    library = await client.get(
        "/api/v1/blogs/blog-note-struct/notes?q=citazione", headers=owner.headers
    )
    assert library.status_code == 200
    lib_note = next(n for n in library.json() if "citazione" in n["content"].casefold())
    assert lib_note["used_in"][0]["idx"] == 1


async def test_post_note_bibtex_fields_roundtrip_and_export(
    client: AsyncClient, make_user: Callable
) -> None:
    """kind/source/issued/url (compatibilità BibTeX): propagati dal post alla
    bibliografia pubblica, alla libreria del blog e da lì all'export .bib."""
    owner: AuthedUser = await make_user("note-bibtex-owner")
    await _blog(client, owner, "blog-note-bibtex")

    created = await client.post(
        "/api/v1/blogs/blog-note-bibtex/posts",
        json={
            "slug": "p",
            "title": "t",
            "content": "c",
            "notes": [
                {
                    "idx": 1,
                    "content": "Un libro citato per intero.",
                    "title": "Il Titolo del Libro",
                    "author": "Autrice Autrice",
                    "kind": "book",
                    "source": "Editore Esempio",
                    "issued": "2019",
                    "url": "https://example.com/libro",
                }
            ],
        },
        headers=owner.headers,
    )
    assert created.status_code == 201, created.text
    note_out = created.json()["notes"][0]
    assert note_out["kind"] == "book"
    assert note_out["source"] == "Editore Esempio"
    assert note_out["issued"] == "2019"
    assert note_out["url"] == "https://example.com/libro"

    await client.post(f"/api/v1/posts/{created.json()['id']}/publish", headers=owner.headers)

    biblio = await client.get("/api/v1/blogs/blog-note-bibtex/bibliography")
    entry = biblio.json()[0]
    assert entry["source"] == "Editore Esempio"
    assert entry["issued"] == "2019"
    assert entry["kind"] == "book"
    assert entry["url"] == "https://example.com/libro"

    library = await client.get(
        "/api/v1/blogs/blog-note-bibtex/notes?q=libro", headers=owner.headers
    )
    lib_note = next(n for n in library.json() if "libro" in n["content"].casefold())
    assert lib_note["source"] == "Editore Esempio"
    assert lib_note["issued"] == "2019"

    export = await client.get(
        "/api/v1/blogs/blog-note-bibtex/notes/export.bib", headers=owner.headers
    )
    assert export.status_code == 200
    bib = export.text
    assert "@book{" in bib
    assert "title = {Il Titolo del Libro}" in bib
    assert "author = {Autrice Autrice}" in bib
    assert "publisher = {Editore Esempio}" in bib
    assert "year = {2019}" in bib
    assert "url = {https://example.com/libro}" in bib

    invalid_kind = await client.post(
        "/api/v1/blogs/blog-note-bibtex/posts",
        json={
            "slug": "p2",
            "title": "t2",
            "content": "c",
            "notes": [{"idx": 1, "content": "x", "kind": "not-a-kind"}],
        },
        headers=owner.headers,
    )
    assert invalid_kind.status_code == 400


async def test_bibtex_import_parses_structured_fields(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("note-bibtex-import-owner")
    await _blog(client, owner, "blog-note-bibtex-import")

    bibtex = (
        "@article{ref1,\n"
        "  title = {Un Articolo},\n"
        "  author = {Autore Vario},\n"
        "  journal = {Rivista Esempio},\n"
        "  year = {2021},\n"
        "  doi = {10.1000/abc}\n"
        "}\n"
    )
    imported = await client.post(
        "/api/v1/blogs/blog-note-bibtex-import/notes/import",
        json={"bibtex": bibtex},
        headers=owner.headers,
    )
    assert imported.status_code == 201, imported.text
    note = imported.json()[0]
    assert note["kind"] == "article"
    assert note["title"] == "Un Articolo"
    assert note["author"] == "Autore Vario"
    assert note["source"] == "Rivista Esempio"
    assert note["issued"] == "2021"
    assert note["doi"] == "10.1000/abc"


async def test_post_note_structured_fields_never_overwrite_existing_library_note(
    client: AsyncClient, make_user: Callable, db_session: AsyncSession
) -> None:
    """Una modifica manuale in libreria non deve essere sovrascritta da un
    nuovo post che cita lo stesso testo — i campi opzionali si propagano solo
    alla creazione di una nuova BlogNote, mai su una già esistente (letta
    direttamente da DB qui solo perché più diretto di un secondo giro di
    `PATCH .../notes/{id}` per verificare lo stesso valore già noto)."""
    owner: AuthedUser = await make_user("note-struct-owner2")
    await _blog(client, owner, "blog-note-struct2")

    first = await client.post(
        "/api/v1/blogs/blog-note-struct2/posts",
        json={
            "slug": "p1",
            "title": "t1",
            "content": "c",
            "notes": [{"idx": 1, "content": "Testo condiviso", "author": "Autore Originale"}],
        },
        headers=owner.headers,
    )
    assert first.status_code == 201, first.text

    second = await client.post(
        "/api/v1/blogs/blog-note-struct2/posts",
        json={
            "slug": "p2",
            "title": "t2",
            "content": "c",
            "notes": [{"idx": 1, "content": "Testo condiviso", "author": "Autore Diverso"}],
        },
        headers=owner.headers,
    )
    assert second.status_code == 201, second.text

    library = await client.get(
        "/api/v1/blogs/blog-note-struct2/notes?q=condiviso", headers=owner.headers
    )
    lib_note = next(n for n in library.json() if "condiviso" in n["content"].casefold())
    assert lib_note["used_in"] and len(lib_note["used_in"]) == 2

    blog_note = (
        await db_session.execute(select(BlogNote).where(BlogNote.id == lib_note["id"]))
    ).scalar_one()
    assert blog_note.author == "Autore Originale"

    # la nota di post p2 mantiene comunque il proprio autore
    p2 = await client.get(f"/api/v1/posts/{second.json()['id']}", headers=owner.headers)
    assert p2.json()["notes"][0]["author"] == "Autore Diverso"


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
    empty_fields = {"title": None, "author": None, "isbn": None, "doi": None, "page": None, "kind": None, "source": None, "issued": None, "url": None}
    assert tr.json()["notes"] == [{"idx": 1, "content": "EN", **empty_fields}]
    # l'originale resta con la sua
    again = await client.get(f"/api/v1/posts/{original.json()['id']}", headers=owner.headers)
    assert again.json()["notes"] == [{"idx": 1, "content": "IT", **empty_fields}]


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
