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
