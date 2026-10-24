import uuid

from sqlalchemy import Column, ForeignKey, String, Table
from sqlalchemy.dialects.postgresql import UUID
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.models.base import Base, TimestampMixin, UUIDPKMixin

# Associazione many-to-many post<->tag (un post ha al massimo 5 tag, vedi
# app/domain/tags.py — vincolo applicativo, non imposto qui a livello di DB).
post_tags = Table(
    "post_tags",
    Base.metadata,
    Column("post_id", UUID(as_uuid=True), ForeignKey("posts.id", ondelete="CASCADE"), primary_key=True),
    Column("tag_id", UUID(as_uuid=True), ForeignKey("tags.id", ondelete="CASCADE"), primary_key=True),
)


class Tag(Base, UUIDPKMixin, TimestampMixin):
    __tablename__ = "tags"

    # Normalizzato (minuscolo, senza #, solo [a-z0-9-]) da app/domain/tags.py
    # — un solo Tag per ogni forma normalizzata, riusato tra i post.
    name: Mapped[str] = mapped_column(String(30), unique=True, index=True)

    posts: Mapped[list["Post"]] = relationship(secondary=post_tags, back_populates="tags")
