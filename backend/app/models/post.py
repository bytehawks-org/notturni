import enum
import uuid
from datetime import datetime

from sqlalchemy import DateTime, Enum, ForeignKey, String, Text, UniqueConstraint
from sqlalchemy.dialects.postgresql import UUID
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.domain.i18n import DEFAULT_LOCALE
from app.models.base import Base, TimestampMixin, UUIDPKMixin


class PostStatus(str, enum.Enum):
    DRAFT = "draft"
    PENDING_REVIEW = "pending_review"
    PUBLISHED = "published"


class Post(Base, UUIDPKMixin, TimestampMixin):
    __tablename__ = "posts"

    blog_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("blogs.id"), nullable=False)
    author_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), nullable=False)
    # nome pubblico dell'autore per questo post, può differire da User.username (CLAUDE.md #1)
    author_display_name: Mapped[str] = mapped_column(String(255))

    # i18n: le traduzioni di uno stesso contenuto condividono translation_group_id
    # (di default = id del post stesso, quando non è traduzione di nient'altro)
    # ma hanno ciascuna il proprio locale/slug/titolo/contenuto.
    locale: Mapped[str] = mapped_column(String(2), default=DEFAULT_LOCALE, nullable=False)
    translation_group_id: Mapped[uuid.UUID] = mapped_column(
        UUID(as_uuid=True), default=uuid.uuid4, nullable=False, index=True
    )

    title: Mapped[str] = mapped_column(String(255))
    slug: Mapped[str] = mapped_column(String(255), index=True)
    # Markdown. Nessun rendering lato backend: sanificazione/rendering a HTML
    # è responsabilità del frontend al momento della lettura.
    content: Mapped[str] = mapped_column(Text)

    status: Mapped[PostStatus] = mapped_column(
        Enum(
            PostStatus,
            name="post_status",
            native_enum=True,
            values_callable=lambda enum_cls: [e.value for e in enum_cls],
        ),
        default=PostStatus.DRAFT,
        nullable=False,
    )
    # se nel futuro: pianificazione della pubblicazione. Un post con
    # status=published e published_at futuro non è ancora pubblicamente
    # visibile — vedi app/domain/authorization.py:is_publicly_visible.
    published_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)

    blog: Mapped["Blog"] = relationship(back_populates="posts")
    comments: Mapped[list["Comment"]] = relationship(back_populates="post", cascade="all, delete-orphan")

    __table_args__ = (
        UniqueConstraint("blog_id", "slug", "locale", name="uq_post_blog_slug_locale"),
        UniqueConstraint("translation_group_id", "locale", name="uq_post_translation_group_locale"),
    )
