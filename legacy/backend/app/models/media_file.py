import uuid

from sqlalchemy import ARRAY, Boolean, ForeignKey, Integer, String, Text
from sqlalchemy.orm import Mapped, mapped_column

from app.models.base import Base, TimestampMixin, UUIDPKMixin


class MediaFile(Base, UUIDPKMixin, TimestampMixin):
    """Libreria media del blog (todo/UX_REDESIGN.md B7, mockup 3c): una riga
    per ogni immagine caricata via `POST /blogs/{slug}/media`, con alt text,
    didascalia e avviso sui contenuti modificabili dalla dashboard. Le
    immagini citate nei post prima di questa tabella vengono importate da
    `post_media` con `POST /blogs/{slug}/media/sync` (dimensione ignota)."""

    __tablename__ = "media_files"

    blog_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("blogs.id", ondelete="CASCADE"), nullable=False, index=True)
    uploader_id: Mapped[uuid.UUID | None] = mapped_column(ForeignKey("users.id"), nullable=True)
    # chiave sull'object storage (null per le righe importate da post_media)
    object_key: Mapped[str | None] = mapped_column(String(512), nullable=True)
    url: Mapped[str] = mapped_column(Text, nullable=False, unique=True)
    content_type: Mapped[str] = mapped_column(String(100), nullable=False, default="")
    size_bytes: Mapped[int] = mapped_column(Integer, nullable=False, default=0)
    alt_text: Mapped[str] = mapped_column(Text, nullable=False, default="")
    caption: Mapped[str | None] = mapped_column(Text, nullable=True)
    # categorie di avviso scelte dall'autore (app/domain/content_media.py)
    categories: Mapped[list[str]] = mapped_column(ARRAY(String(20)), nullable=False, default=list)
    # esito della moderazione automatica all'upload
    is_sensitive: Mapped[bool] = mapped_column(Boolean, nullable=False, default=False)
