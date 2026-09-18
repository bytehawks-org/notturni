"""Libreria note del blog (todo/UX_REDESIGN.md B8, mockup 3b): elenco con
"citata in" e duplicati, modifica di tipo/URL/testo, merge, import/export
BibTeX. Proprietario e collaboratori."""

import uuid
from datetime import datetime
from typing import NamedTuple

from fastapi import Depends, HTTPException, Response, status
from pydantic import BaseModel, Field
from sqlalchemy import func, select, update
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user
from app.api.v1.blogs._common import _get_blog_or_404, _require_blog_write_access
from app.api.v1.blogs._router import router
from app.core.database import get_session
from app.domain.authorization import get_membership_role
from app.domain.blog_notes import MAX_BIBTEX_LENGTH, find_duplicate_groups, normalize_note, parse_bibtex, to_bibtex
from app.domain.notes import (
    MAX_NOTE_AUTHOR_LENGTH,
    MAX_NOTE_DOI_LENGTH,
    MAX_NOTE_ISBN_LENGTH,
    MAX_NOTE_ISSUED_LENGTH,
    MAX_NOTE_PAGE_LENGTH,
    MAX_NOTE_SOURCE_LENGTH,
    MAX_NOTE_TITLE_LENGTH,
    MAX_NOTE_URL_LENGTH,
    _clean_optional,
    _clean_url,
)
from app.models.blog import Blog
from app.models.blog_note import NOTE_KINDS, BlogNote
from app.models.post import Post
from app.models.post_note import post_notes
from app.models.user import User


class NoteUsageOut(BaseModel):
    post_id: uuid.UUID
    post_slug: str
    post_title: str
    idx: int


class BlogNoteOut(BaseModel):
    id: uuid.UUID
    content: str
    kind: str
    url: str | None
    # Compatibilità BibTeX (app/domain/blog_notes.py) — stessi campi
    # facoltativi di post_notes, esposti qui per la libreria (NotesTab.tsx).
    title: str | None
    author: str | None
    source: str | None
    issued: str | None
    isbn: str | None
    doi: str | None
    page: str | None
    created_at: datetime
    updated_at: datetime
    used_in: list[NoteUsageOut]
    possible_duplicates: list[uuid.UUID]


class BlogNoteCreate(BaseModel):
    content: str
    kind: str = "note"
    url: str | None = None
    title: str | None = None
    author: str | None = None
    source: str | None = None
    issued: str | None = None
    isbn: str | None = None
    doi: str | None = None
    page: str | None = None


class BlogNoteUpdate(BaseModel):
    content: str | None = None
    kind: str | None = None
    url: str | None = None
    # assenti: non toccano il campo; stringa vuota: lo svuota (stesso schema
    # semplificato di BlogNoteCreate, nessun tri-state qui — vedi PATCH sotto).
    title: str | None = None
    author: str | None = None
    source: str | None = None
    issued: str | None = None
    isbn: str | None = None
    doi: str | None = None
    page: str | None = None


class MergeRequest(BaseModel):
    into_id: uuid.UUID


class ImportRequest(BaseModel):
    # MAX_BIBTEX_LENGTH (app/domain/blog_notes.py): rifiutato qui con 422
    # invece che raggiungere il parser, stesso limite comunque applicato lì
    # a difesa in profondità.
    bibtex: str = Field(max_length=MAX_BIBTEX_LENGTH)


async def _require_member(session: AsyncSession, user: User, blog: Blog) -> None:
    if blog.owner_id == user.id:
        return
    if await get_membership_role(session, user_id=user.id, blog_id=blog.id) is None:
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Serve essere proprietario o collaboratore del blog.")


async def _usages(session: AsyncSession, note_ids: list[uuid.UUID]) -> dict[uuid.UUID, list[NoteUsageOut]]:
    if not note_ids:
        return {}
    rows = (
        await session.execute(
            select(post_notes.c.note_id, post_notes.c.idx, Post.id, Post.slug, Post.title)
            .join(Post, Post.id == post_notes.c.post_id)
            .where(post_notes.c.note_id.in_(note_ids))
            .order_by(Post.created_at.desc(), post_notes.c.idx)
        )
    ).all()
    out: dict[uuid.UUID, list[NoteUsageOut]] = {}
    for note_id, idx, post_id, slug, title in rows:
        out.setdefault(note_id, []).append(NoteUsageOut(post_id=post_id, post_slug=slug, post_title=title, idx=idx))
    return out


async def _notes_out(session: AsyncSession, notes: list[BlogNote], all_notes: list[BlogNote] | None = None) -> list[BlogNoteOut]:
    usages = await _usages(session, [n.id for n in notes])
    dups = find_duplicate_groups(all_notes if all_notes is not None else notes)
    return [
        BlogNoteOut(
            id=n.id,
            content=n.content,
            kind=n.kind,
            url=n.url,
            title=n.title,
            author=n.author,
            source=n.source,
            issued=n.issued,
            isbn=n.isbn,
            doi=n.doi,
            page=n.page,
            created_at=n.created_at,
            updated_at=n.updated_at,
            used_in=usages.get(n.id, []),
            possible_duplicates=dups.get(n.id, []),
        )
        for n in notes
    ]


def _validate_kind(kind: str | None) -> None:
    if kind is not None and kind not in NOTE_KINDS:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, f"Tipo non valido: usare uno tra {', '.join(NOTE_KINDS)}.")


class _CleanBibFields(NamedTuple):
    title: str | None
    author: str | None
    source: str | None
    issued: str | None
    isbn: str | None
    doi: str | None
    page: str | None
    url: str | None


def _clean_bib_fields(
    *,
    title: str | None,
    author: str | None,
    source: str | None,
    issued: str | None,
    isbn: str | None,
    doi: str | None,
    page: str | None,
    url: str | None,
) -> _CleanBibFields:
    """Stessi limiti di lunghezza e stessa validazione dello schema dell'URL
    applicati alle note dei post (app/domain/notes.py) — senza questo i campi
    "compatibilità BibTeX" della libreria potevano raggiungere le colonne
    VARCHAR con valori troppo lunghi e l'URL non era ristretto a http(s) come
    lo è altrove."""
    try:
        return _CleanBibFields(
            title=_clean_optional(title, max_length=MAX_NOTE_TITLE_LENGTH, label="titolo"),
            author=_clean_optional(author, max_length=MAX_NOTE_AUTHOR_LENGTH, label="autore"),
            source=_clean_optional(source, max_length=MAX_NOTE_SOURCE_LENGTH, label="editore/rivista/sito"),
            issued=_clean_optional(issued, max_length=MAX_NOTE_ISSUED_LENGTH, label="anno/data"),
            isbn=_clean_optional(isbn, max_length=MAX_NOTE_ISBN_LENGTH, label="ISBN"),
            doi=_clean_optional(doi, max_length=MAX_NOTE_DOI_LENGTH, label="DOI"),
            page=_clean_optional(page, max_length=MAX_NOTE_PAGE_LENGTH, label="pagina"),
            url=_clean_url(url, max_length=MAX_NOTE_URL_LENGTH) if url is not None else None,
        )
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc


def _import_clean(value: str | None, *, max_length: int) -> str | None:
    """Come `_clean_optional`, ma tronca invece di rifiutare: un import
    BibTeX è un lotto di voci, un singolo campo fuori limite non deve far
    fallire l'intero batch (a differenza della form della libreria, dove
    l'utente può correggere subito il singolo valore)."""
    if not value:
        return None
    text = " ".join(value.split())[:max_length]
    return text or None


def _import_clean_url(value: str | None, *, max_length: int) -> str | None:
    """Come `_import_clean`, ma un URL fuori dalla policy http(s) viene
    scartato (mai troncato): un valore troncato potrebbe restare comunque
    uno schema pericoloso, e un URL non è comunque utile spezzato a metà."""
    text = _import_clean(value, max_length=max_length)
    if text is None or not (text.startswith("http://") or text.startswith("https://")):
        return None
    return text


@router.get("/{slug}/notes", response_model=list[BlogNoteOut])
async def list_blog_notes(
    slug: str,
    q: str | None = None,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[BlogNoteOut]:
    """Tutte le note del blog (anche non usate), dalla più recente, con i
    post che le citano e i possibili duplicati (stesso inizio normalizzato).
    `q` filtra sul testo."""
    blog = await _get_blog_or_404(session, slug)
    await _require_member(session, current_user, blog)
    stmt = select(BlogNote).where(BlogNote.blog_id == blog.id).order_by(BlogNote.updated_at.desc())
    all_notes = list((await session.execute(stmt)).scalars().all())
    notes = [n for n in all_notes if not q or q.strip().lower() in n.content.lower()]
    return await _notes_out(session, notes, all_notes)


@router.post("/{slug}/notes", response_model=BlogNoteOut, status_code=status.HTTP_201_CREATED)
async def create_blog_note(
    slug: str,
    payload: BlogNoteCreate,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> BlogNoteOut:
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    content = payload.content.strip()
    if not content:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Il testo della nota è obbligatorio.")
    _validate_kind(payload.kind)
    bib = _clean_bib_fields(
        title=payload.title,
        author=payload.author,
        source=payload.source,
        issued=payload.issued,
        isbn=payload.isbn,
        doi=payload.doi,
        page=payload.page,
        url=payload.url,
    )
    note = BlogNote(
        blog_id=blog.id,
        content=content[:2000],
        normalized=normalize_note(content),
        kind=payload.kind,
        url=bib.url,
        title=bib.title,
        author=bib.author,
        source=bib.source,
        issued=bib.issued,
        isbn=bib.isbn,
        doi=bib.doi,
        page=bib.page,
        created_by_id=current_user.id,
    )
    session.add(note)
    await session.commit()
    await session.refresh(note)
    return (await _notes_out(session, [note]))[0]


async def _get_note(session: AsyncSession, blog: Blog, note_id: uuid.UUID) -> BlogNote:
    note = await session.get(BlogNote, note_id)
    if note is None or note.blog_id != blog.id:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Nota non trovata.")
    return note


@router.patch("/{slug}/notes/{note_id}", response_model=BlogNoteOut)
async def update_blog_note(
    slug: str,
    note_id: uuid.UUID,
    payload: BlogNoteUpdate,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> BlogNoteOut:
    """Modifica tipo, URL/DOI e testo. Cambiando il testo viene aggiornato
    anche nei post che la citano (`post_notes.content`), così bibliografia e
    note a piè di pagina restano coerenti."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    note = await _get_note(session, blog, note_id)
    _validate_kind(payload.kind)
    if payload.kind is not None:
        note.kind = payload.kind
    try:
        if payload.url is not None:
            note.url = _clean_url(payload.url, max_length=MAX_NOTE_URL_LENGTH)
        if payload.title is not None:
            note.title = _clean_optional(payload.title, max_length=MAX_NOTE_TITLE_LENGTH, label="titolo")
        if payload.author is not None:
            note.author = _clean_optional(payload.author, max_length=MAX_NOTE_AUTHOR_LENGTH, label="autore")
        if payload.source is not None:
            note.source = _clean_optional(payload.source, max_length=MAX_NOTE_SOURCE_LENGTH, label="editore/rivista/sito")
        if payload.issued is not None:
            note.issued = _clean_optional(payload.issued, max_length=MAX_NOTE_ISSUED_LENGTH, label="anno/data")
        if payload.isbn is not None:
            note.isbn = _clean_optional(payload.isbn, max_length=MAX_NOTE_ISBN_LENGTH, label="ISBN")
        if payload.doi is not None:
            note.doi = _clean_optional(payload.doi, max_length=MAX_NOTE_DOI_LENGTH, label="DOI")
        if payload.page is not None:
            note.page = _clean_optional(payload.page, max_length=MAX_NOTE_PAGE_LENGTH, label="pagina")
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc
    if payload.content is not None:
        content = payload.content.strip()
        if not content:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, "Il testo della nota è obbligatorio.")
        note.content = content[:2000]
        note.normalized = normalize_note(content)
        await session.execute(update(post_notes).where(post_notes.c.note_id == note.id).values(content=note.content))
    await session.commit()
    await session.refresh(note)
    return (await _notes_out(session, [note]))[0]


@router.delete("/{slug}/notes/{note_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_blog_note(
    slug: str,
    note_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    """Solo note non citate da nessun post (`409` altrimenti)."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    note = await _get_note(session, blog, note_id)
    used = (await session.execute(select(func.count()).select_from(post_notes).where(post_notes.c.note_id == note.id))).scalar_one()
    if used:
        raise HTTPException(status.HTTP_409_CONFLICT, "La nota è citata in un post: rimuovila prima dal post o uniscila a un'altra.")
    await session.delete(note)
    await session.commit()


@router.post("/{slug}/notes/{note_id}/merge", response_model=BlogNoteOut)
async def merge_blog_note(
    slug: str,
    note_id: uuid.UUID,
    payload: MergeRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> BlogNoteOut:
    """Unisce la nota `note_id` in `into_id` (mockup 3b "Possible duplicate ·
    merge"): le citazioni nei post passano alla nota di destinazione (testo
    incluso), la nota sorgente viene eliminata."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    if payload.into_id == note_id:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Scegli una nota diversa in cui unire.")
    source = await _get_note(session, blog, note_id)
    target = await _get_note(session, blog, payload.into_id)
    await session.execute(update(post_notes).where(post_notes.c.note_id == source.id).values(note_id=target.id, content=target.content))
    await session.delete(source)
    await session.commit()
    await session.refresh(target)
    return (await _notes_out(session, [target]))[0]


@router.get("/{slug}/notes/export.bib")
async def export_blog_notes(
    slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> Response:
    blog = await _get_blog_or_404(session, slug)
    await _require_member(session, current_user, blog)
    notes = (await session.execute(select(BlogNote).where(BlogNote.blog_id == blog.id).order_by(BlogNote.created_at))).scalars().all()
    return Response(content=to_bibtex(notes), media_type="application/x-bibtex", headers={"Content-Disposition": f'attachment; filename="{blog.slug}.bib"'})


@router.post("/{slug}/notes/import", response_model=list[BlogNoteOut], status_code=status.HTTP_201_CREATED)
async def import_blog_notes(
    slug: str,
    payload: ImportRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[BlogNoteOut]:
    """Import BibTeX (parser minimale, vedi app/domain/blog_notes.py): le
    voci con testo già presente (normalizzato) vengono saltate."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    parsed = parse_bibtex(payload.bibtex)
    if not parsed:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Nessuna voce BibTeX riconosciuta.")
    existing = set((await session.execute(select(BlogNote.normalized).where(BlogNote.blog_id == blog.id))).scalars().all())
    created: list[BlogNote] = []
    for item in parsed:
        key = normalize_note(item["content"])
        if not key or key in existing:
            continue
        existing.add(key)
        note = BlogNote(
            blog_id=blog.id,
            content=item["content"],
            normalized=key,
            kind=item["kind"],
            url=_import_clean_url(item["url"], max_length=MAX_NOTE_URL_LENGTH),
            title=_import_clean(item.get("title"), max_length=MAX_NOTE_TITLE_LENGTH),
            author=_import_clean(item.get("author"), max_length=MAX_NOTE_AUTHOR_LENGTH),
            source=_import_clean(item.get("source"), max_length=MAX_NOTE_SOURCE_LENGTH),
            issued=_import_clean(item.get("issued"), max_length=MAX_NOTE_ISSUED_LENGTH),
            isbn=_import_clean(item.get("isbn"), max_length=MAX_NOTE_ISBN_LENGTH),
            doi=_import_clean(item.get("doi"), max_length=MAX_NOTE_DOI_LENGTH),
            page=_import_clean(item.get("page"), max_length=MAX_NOTE_PAGE_LENGTH),
            created_by_id=current_user.id,
        )
        session.add(note)
        created.append(note)
    await session.commit()
    for n in created:
        await session.refresh(n)
    return await _notes_out(session, created)

