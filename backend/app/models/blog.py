import enum
import uuid
from datetime import datetime

from sqlalchemy import (
    Boolean,
    CheckConstraint,
    DateTime,
    Enum,
    ForeignKey,
    String,
    UniqueConstraint,
)
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.domain.i18n import DEFAULT_LOCALE
from app.models.base import Base, TimestampMixin, UUIDPKMixin


class BlogRole(str, enum.Enum):
    """Ruolo utente specifico di un blog, distinto dal PlatformRole (CLAUDE.md #1)."""

    AUTORE = "autore"
    CO_AUTORE = "co_autore"
    REVISORE = "revisore"
    MEDIATORE = "mediatore"


class BlogVisibility(str, enum.Enum):
    """Visibilità del blog (todo/BLOG.md #2):

    - ``PUBLIC``: raggiungibile da chiunque, compare nel feed della homepage.
    - ``MEMBERS``: pagine pubbliche leggibili solo da chi è autenticato sulla
      piattaforma; escluso dal feed aggregato.
    - ``PRIVATE``: diario privato — leggibile solo dal proprietario (e da chi
      ha una membership), e con scrittura consentita al **solo proprietario**
      a prescindere dalle membership (vedi app/domain/authorization.py).
    """

    PUBLIC = "public"
    MEMBERS = "members"
    PRIVATE = "private"


class BlogInvitationStatus(str, enum.Enum):
    PENDING = "pending"
    ACCEPTED = "accepted"
    DECLINED = "declined"
    REVOKED = "revoked"


class Blog(Base, UUIDPKMixin, TimestampMixin):
    __tablename__ = "blogs"

    # sottodominio: https://{slug}.notturni.eu (CLAUDE.md #2)
    slug: Mapped[str] = mapped_column(String(63), unique=True, index=True)
    title: Mapped[str] = mapped_column(String(255))
    # Sottotitolo (max 64) e descrizione breve (max 256) del blog
    # (todo/BLOG.md #1) — entrambi opzionali.
    subtitle: Mapped[str | None] = mapped_column(String(64), nullable=True)
    description: Mapped[str | None] = mapped_column(String(256), nullable=True)
    custom_domain: Mapped[str | None] = mapped_column(String(255), unique=True, nullable=True)
    visibility: Mapped[BlogVisibility] = mapped_column(
        Enum(
            BlogVisibility,
            name="blog_visibility",
            native_enum=True,
            values_callable=lambda enum_cls: [e.value for e in enum_cls],
        ),
        default=BlogVisibility.PUBLIC,
        nullable=False,
    )
    # CLAUDE.md #1: di default i commenti sono possibili solo a utenti registrati;
    # il proprietario del blog può aprirli anche ai non registrati (moderazione
    # obbligatoria in quel caso, vedi app/api/v1/comments.py)
    allow_anonymous_comments: Mapped[bool] = mapped_column(Boolean, default=False, nullable=False)
    # todo/EDITOR.md: le @menzioni nel contenuto dei post vengono trasformate
    # in link al profilo dell'utente citato. Attive di default, disattivabili
    # dal proprietario del blog.
    mentions_enabled: Mapped[bool] = mapped_column(Boolean, default=True, nullable=False)
    # CLAUDE.md #1: pagine statiche (Chi siamo, Contattami, ...) sono opt-in
    # per blog, disattive di default — sempre attive invece per le pagine di
    # piattaforma (vedi app/models/page.py, app/api/v1/pages.py).
    static_pages_enabled: Mapped[bool] = mapped_column(Boolean, default=False, nullable=False)
    # Sospensione da parte di un admin di piattaforma (frontend/admin/):
    # blog irraggiungibile pubblicamente e non scrivibile finché non viene
    # riattivato, indipendentemente da `visibility` — vedi app/domain/authorization.py.
    # Mai impostabile dal proprietario, solo da PATCH /api/v1/admin/blogs/{id}.
    is_suspended: Mapped[bool] = mapped_column(Boolean, default=False, nullable=False)
    # i18n (CLAUDE.md #1/#2): lingua di default del blog; i singoli post
    # possono avere traduzioni in altre lingue, vedi app/models/post.py
    default_locale: Mapped[str] = mapped_column(String(2), default=DEFAULT_LOCALE, nullable=False)
    # Nome pubblico predefinito per i testi scritti su questo blog (CLAUDE.md
    # #1: il nome dell'autore può differire dal nome utente reale) — usato
    # come default di Post.author_display_name quando non specificato
    # esplicitamente in creazione, non un vincolo: resta sempre modificabile
    # per singolo post/autore.
    default_author_display_name: Mapped[str | None] = mapped_column(String(255), nullable=True)

    owner_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), nullable=False)

    owner: Mapped["User"] = relationship(back_populates="blogs")
    memberships: Mapped[list["BlogMembership"]] = relationship(
        back_populates="blog", cascade="all, delete-orphan"
    )
    invitations: Mapped[list["BlogInvitation"]] = relationship(
        back_populates="blog", cascade="all, delete-orphan"
    )
    posts: Mapped[list["Post"]] = relationship(back_populates="blog", cascade="all, delete-orphan")
    pages: Mapped[list["Page"]] = relationship(back_populates="blog", cascade="all, delete-orphan")
    categories: Mapped[list["Category"]] = relationship(back_populates="blog", cascade="all, delete-orphan")
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
    # Alias con cui questo utente firma i testi scritti su questo specifico
    # blog (todo/BLOG.md #4). Ha precedenza sul default del blog e sull'alias
    # globale del profilo, ma non su un author_display_name indicato per il
    # singolo post — vedi app/api/v1/posts.py.
    author_display_name: Mapped[str | None] = mapped_column(String(255), nullable=True)

    user: Mapped["User"] = relationship(back_populates="memberships")
    blog: Mapped["Blog"] = relationship(back_populates="memberships")

    __table_args__ = (UniqueConstraint("user_id", "blog_id", name="uq_blog_membership_user_blog"),)


class BlogInvitation(Base, UUIDPKMixin, TimestampMixin):
    """Invito a collaborare su un blog come co-autore o mediatore
    (todo/BLOG.md #3). L'invitato accetta o rifiuta dalla propria dashboard;
    solo all'accettazione viene creata la ``BlogMembership`` corrispondente.

    Una sola riga per (blog, utente invitato): un nuovo invito dopo un
    rifiuto/revoca riusa la stessa riga riportandola a ``pending``.
    """

    __tablename__ = "blog_invitations"

    blog_id: Mapped[uuid.UUID] = mapped_column(
        ForeignKey("blogs.id", ondelete="CASCADE"), nullable=False
    )
    invited_user_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), nullable=False)
    invited_by_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), nullable=False)
    role: Mapped[BlogRole] = mapped_column(
        Enum(
            BlogRole,
            name="blog_role",
            native_enum=True,
            values_callable=lambda enum_cls: [e.value for e in enum_cls],
            create_type=False,
        ),
        nullable=False,
    )
    status: Mapped[BlogInvitationStatus] = mapped_column(
        Enum(
            BlogInvitationStatus,
            name="blog_invitation_status",
            native_enum=True,
            values_callable=lambda enum_cls: [e.value for e in enum_cls],
        ),
        default=BlogInvitationStatus.PENDING,
        nullable=False,
    )
    responded_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)

    blog: Mapped["Blog"] = relationship(back_populates="invitations")
    invited_user: Mapped["User"] = relationship(foreign_keys=[invited_user_id])
    invited_by: Mapped["User"] = relationship(foreign_keys=[invited_by_id])

    __table_args__ = (
        UniqueConstraint("blog_id", "invited_user_id", name="uq_blog_invitation_blog_user"),
    )
