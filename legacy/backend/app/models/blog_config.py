import uuid

from sqlalchemy import ForeignKey
from sqlalchemy.dialects.postgresql import JSONB
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.models.base import Base, TimestampMixin, UUIDPKMixin


class BlogConfig(Base, UUIDPKMixin, TimestampMixin):
    """Configurazione di presentazione del blog (palette, tipografia, layout)
    in JSON libero — CLAUDE.md #1: "prevedere ma non implementare da subito
    la possibilità di personalizzazione" — implementato qui. Un solo record
    per blog; il default applicato quando manca è in app/domain/blog_config.py."""

    __tablename__ = "blog_configs"

    blog_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("blogs.id"), unique=True, nullable=False)
    config: Mapped[dict] = mapped_column(JSONB, default=dict, nullable=False)

    blog: Mapped["Blog"] = relationship(back_populates="config")
