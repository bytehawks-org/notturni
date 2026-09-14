import uuid

from sqlalchemy import ForeignKey, String, Text, UniqueConstraint
from sqlalchemy.orm import Mapped, mapped_column

from app.models.base import Base, TimestampMixin, UUIDPKMixin


class Publication(Base, UUIDPKMixin, TimestampMixin):
    """Pubblicazione di un blog (todo/PUBLICATIONS.md, todo/UX_REDESIGN.md
    B9, mockup 2d/3g): raccoglie post come capitoli sotto
    `/{blog}/pub/{name}`, in ordine cronologico dal più vecchio salvo un
    ordine esplicito (`Post.chapter_order`). Un post appartiene al più a una
    pubblicazione (`Post.publication_id`)."""

    __tablename__ = "publications"

    blog_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("blogs.id", ondelete="CASCADE"), nullable=False, index=True)
    # segmento URL: /{blog}/pub/{name}
    name: Mapped[str] = mapped_column(String(60), nullable=False)
    title: Mapped[str] = mapped_column(String(255), nullable=False)
    description: Mapped[str | None] = mapped_column(Text, nullable=True)

    __table_args__ = (UniqueConstraint("blog_id", "name", name="uq_publication_blog_name"),)
