"""Aggancio delle note dei post alla libreria note del blog (B8)."""

import uuid

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.domain.blog_notes import extract_url, guess_kind, normalize_note
from app.domain.notes import NoteInput
from app.models.blog_note import BlogNote


async def link_blog_notes(
    session: AsyncSession, *, blog_id: uuid.UUID, notes: list[NoteInput], created_by_id: uuid.UUID | None
) -> dict[str, uuid.UUID]:
    """Per ogni nota restituisce l'id della nota del blog con lo stesso testo
    normalizzato, creandola se manca. Nessun commit: parte della transazione
    di salvataggio del post.

    I campi bibliografici opzionali (`title`/`author`/`isbn`/`doi`/`page`)
    vengono riportati sulla nota di libreria **solo alla creazione**: se una
    nota di libreria con lo stesso testo esiste già non viene mai
    sovrascritta da qui, per non perdere un'eventuale modifica fatta dalla
    libreria stessa (dashboard, `NotesTab.tsx`)."""
    wanted = {n.content: (normalize_note(n.content), n) for n in notes}
    keys = {key for key, _ in wanted.values() if key}
    existing: dict[str, BlogNote] = {}
    if keys:
        rows = (await session.execute(select(BlogNote).where(BlogNote.blog_id == blog_id, BlogNote.normalized.in_(keys)))).scalars().all()
        existing = {n.normalized: n for n in rows}
    out: dict[str, uuid.UUID] = {}
    for content, (key, note_input) in wanted.items():
        if not key:
            continue
        note = existing.get(key)
        if note is None:
            url = extract_url(content)
            note = BlogNote(
                blog_id=blog_id,
                content=content.strip(),
                normalized=key,
                kind=guess_kind(content, url),
                url=url,
                created_by_id=created_by_id,
                title=note_input.title,
                author=note_input.author,
                isbn=note_input.isbn,
                doi=note_input.doi,
                page=note_input.page,
            )
            session.add(note)
            await session.flush()
            existing[key] = note
        out[content] = note.id
    return out
