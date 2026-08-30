import enum
import uuid

from sqlalchemy import Boolean, CheckConstraint, Enum, ForeignKey, String, UniqueConstraint
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.domain.i18n import DEFAULT_LOCALE
from app.models.base import Base, TimestampMixin, UUIDPKMixin


class BlogRole(str, enum.Enum):
    """Ruolo utente specifico di un blog, distinto dal PlatformRole (CLAUDE.md #1)."""

    AUTORE = "autore"
    CO_AUTORE = "co_autore"
    REVISORE = "revisore"
    MEDIATORE = "mediatore"


class Blog(Base, UUIDPKMixin, TimestampMixin):
    __tablename__ = "blogs"

    # sottodominio: https://{slug}.notturni.eu (CLAUDE.md #2)
    slug: Mapped[str] = mapped_column(String(63), unique=True, index=True)
    title: Mapped[str] = mapped_column(String(255))
    custom_domain: Mapped[str | None] = mapped_column(String(255), unique=True, nullable=True)
    # CLAUDE.md #1: di default i commenti sono possibili solo a utenti registrati;
    # il proprietario del blog può aprirli anche ai non registrati (moderazione
    # obbligatoria in quel caso, vedi app/api/v1/comments.py)
    allow_anonymous_comments: Mapped[bool] = mapped_column(Boolean, default=False, nullable=False)
    # i18n (CLAUDE.md #1/#2): lingua di default del blog; i singoli post
    # possono avere traduzioni in altre lingue, vedi app/models/post.py
    default_locale: Mapped[str] = mapped_column(String(2), default=DEFAULT_LOCALE, nullable=False)

    owner_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), nullable=False)

    owner: Mapped["User"] = relationship(back_populates="blogs")
    memberships: Mapped[list["BlogMembership"]] = relationship(
        back_populates="blog", cascade="all, delete-orphan"
    )
    posts: Mapped[list["Post"]] = relationship(back_populates="blog", cascade="all, delete-orphan")
    follows: Mapped[list["BlogFollow"]] = relationship(back_populates="blog", cascade="all, delete-orphan")
    config: Mapped["BlogConfig | None"] = relationship(
        back_populates="blog", uselist=False, cascade="all, delete-orphan"
    )

    __table_args__ = (
        # la lunghezza minima riflette CLAUDE.md #2; il controllo su blacklist e
        # sui nomi riservati (<=3 caratteri) è applicativo, vedi app/domain/blog_rules.py
        CheckConstraint("length(slug) >= 4", name="ck_blog_slug_min_length"),
    )


class BlogMembership(Base, UUIDPKMixin, TimestampMixin):
    __tablename__ = "blog_memberships"

    user_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), nullable=False)
    blog_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("blogs.id"), nullable=False)
    role: Mapped[BlogRole] = mapped_column(
        Enum(
            BlogRole,
            name="blog_role",
            native_enum=True,
            values_callable=lambda enum_cls: [e.value for e in enum_cls],
        ),
        default=BlogRole.AUTORE,
        nullable=False,
    )

    user: Mapped["User"] = relationship(back_populates="memberships")
    blog: Mapped["Blog"] = relationship(back_populates="memberships")

    __table_args__ = (UniqueConstraint("user_id", "blog_id", name="uq_blog_membership_user_blog"),)
