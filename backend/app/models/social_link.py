import uuid

from sqlalchemy import ForeignKey, Integer, String
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.models.base import Base, TimestampMixin, UUIDPKMixin


class SocialLink(Base, UUIDPKMixin, TimestampMixin):
    """Link social nel profilo utente (CLAUDE.md #1). Etichetta libera invece
    di un enum di piattaforme fisse, per non vincolare l'utente a un elenco
    chiuso di servizi."""

    __tablename__ = "social_links"

    user_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), nullable=False)
    label: Mapped[str] = mapped_column(String(50))
    url: Mapped[str] = mapped_column(String(500))
    position: Mapped[int] = mapped_column(Integer, default=0, nullable=False)

    user: Mapped["User"] = relationship(back_populates="social_links")
