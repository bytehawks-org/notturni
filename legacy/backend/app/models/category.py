import uuid

from sqlalchemy import ForeignKey, String, UniqueConstraint
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.models.base import Base, TimestampMixin, UUIDPKMixin


class Category(Base, UUIDPKMixin, TimestampMixin):
    """Categoria di un blog: tassonomia definita dal proprietario/autori,
    a differenza dei tag (liberi, fino a 5 per post — vedi app/domain/tags.py)
    un post appartiene al più a UNA categoria (vedi Post.category_id),
    pensata come classificazione principale dei contenuti, non descrittiva."""

    __tablename__ = "categories"

    blog_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("blogs.id"), nullable=False)
    name: Mapped[str] = mapped_column(String(50))
    slug: Mapped[str] = mapped_column(String(60))

    blog: Mapped["Blog"] = relationship(back_populates="categories")

    __table_args__ = (UniqueConstraint("blog_id", "slug", name="uq_category_blog_slug"),)
