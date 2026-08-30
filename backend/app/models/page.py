import uuid

from sqlalchemy import Boolean, ForeignKey, String, Text, UniqueConstraint
from sqlalchemy.dialects.postgresql import UUID
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.domain.i18n import DEFAULT_LOCALE
from app.models.base import Base, TimestampMixin, UUIDPKMixin


class Page(Base, UUIDPKMixin, TimestampMixin):
    """Pagine statiche del sito principale notturni.eu (CLAUDE.md #1/#2):
    Chi siamo, Contatti, Privacy, ecc. Non legate a un blog utente. Stesso
    schema di traduzione di Post: le traduzioni condividono translation_group_id."""

    __tablename__ = "pages"

    slug: Mapped[str] = mapped_column(String(255), index=True)
    locale: Mapped[str] = mapped_column(String(2), default=DEFAULT_LOCALE, nullable=False)
    translation_group_id: Mapped[uuid.UUID] = mapped_column(
        UUID(as_uuid=True), default=uuid.uuid4, nullable=False, index=True
    )

    title: Mapped[str] = mapped_column(String(255))
    content: Mapped[str] = mapped_column(Text)
    is_published: Mapped[bool] = mapped_column(Boolean, default=False, nullable=False)

    # ultimo amministratore/moderatore che ha modificato la pagina
    updated_by_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), nullable=False)
    updated_by: Mapped["User"] = relationship()

    __table_args__ = (
        UniqueConstraint("slug", "locale", name="uq_page_slug_locale"),
        UniqueConstraint("translation_group_id", "locale", name="uq_page_translation_group_locale"),
    )
