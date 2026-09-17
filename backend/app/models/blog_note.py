import uuid

from sqlalchemy import ForeignKey, String, Text
from sqlalchemy.orm import Mapped, mapped_column

from app.models.base import Base, TimestampMixin, UUIDPKMixin

NOTE_KINDS = ("book", "article", "web", "note")


class BlogNote(Base, UUIDPKMixin, TimestampMixin):
    """Nota/riferimento bibliografico di un blog come entità di prima classe
    (todo/UX_REDESIGN.md B8, mockup 3b): tipo, URL/DOI, testo. Le note dei
    post (`post_notes`) vi si agganciano con `note_id` alla sincronizzazione
    (stesso testo normalizzato = stessa nota), così "citata in" e i duplicati
    si calcolano dal collegamento, non da confronti di testo a ogni lettura."""

    __tablename__ = "blog_notes"

    blog_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("blogs.id", ondelete="CASCADE"), nullable=False, index=True)
    content: Mapped[str] = mapped_column(Text, nullable=False)
    # chiave di deduplica: testo minuscolo, solo alfanumerici, spazi collassati
    normalized: Mapped[str] = mapped_column(String(600), nullable=False, index=True)
    kind: Mapped[str] = mapped_column(String(10), nullable=False, default="note")
    url: Mapped[str | None] = mapped_column(Text, nullable=True)
    created_by_id: Mapped[uuid.UUID | None] = mapped_column(ForeignKey("users.id"), nullable=True)
    # Stessi campi facoltativi di `post_notes` (todo/EDITOR.md, modal "Nota"):
    # riportati qui solo alla creazione della nota di libreria, dalla nota di
    # post che l'ha generata (app/domain/blog_notes_sync.py::link_blog_notes)
    # — mai sovrascritti da una sincronizzazione successiva, per non perdere
    # un'eventuale modifica fatta da qui (NotesTab.tsx).
    title: Mapped[str | None] = mapped_column(Text, nullable=True)
    author: Mapped[str | None] = mapped_column(Text, nullable=True)
    isbn: Mapped[str | None] = mapped_column(String(32), nullable=True)
    doi: Mapped[str | None] = mapped_column(String(255), nullable=True)
    page: Mapped[str | None] = mapped_column(String(32), nullable=True)
    # Compatibilità BibTeX (vedi app/domain/blog_notes.py::to_bibtex):
    # editore/rivista/sito (publisher/journal/container-title) e anno/data
    # (year/date) — stessa origine (riportati solo alla creazione) e stesse
    # regole di `title`/`author` sopra.
    source: Mapped[str | None] = mapped_column(Text, nullable=True)
    issued: Mapped[str | None] = mapped_column(String(32), nullable=True)
