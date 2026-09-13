"""Aggancio delle note dei post alla libreria note del blog (B8)."""

import uuid

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.domain.blog_notes import extract_url, guess_kind, normalize_note
from app.models.blog_note import BlogNote


async def link_blog_notes(
    session: AsyncSession, *, blog_id: uuid.UUID, contents: list[str], created_by_id: uuid.UUID | None
) -> dict[str, uuid.UUID]:
    """Per ogni testo restituisce l'id della nota del blog con lo stesso testo
    normalizzato, creandola se manca. Nessun commit: parte della transazione
    di salvataggio del post."""
    wanted = {c: normalize_note(c) for c in contents}
    keys = {k for k in wanted.values() if k}
    existing: dict[str, BlogNote] = {}
    if keys:
        rows = (await session.execute(select(BlogNote).where(BlogNote.blog_id == blog_id, BlogNote.normalized.in_(keys)))).scalars().all()
        existing = {n.normalized: n for n in rows}
    out: dict[str, uuid.UUID] = {}
    for content, key in wanted.items():
        if not key:
            continue
        note = existing.get(key)
        if note is None:
            url = extract_url(content)
            note = BlogNote(blog_id=blog_id, content=content.strip(), normalized=key, kind=guess_kind(content, url), url=url, created_by_id=created_by_id)
            session.add(note)
            await session.flush()
            existing[key] = note
        out[content] = note.id
    return out
