import enum
import uuid
from datetime import datetime

from sqlalchemy import ARRAY, Boolean, DateTime, Enum, ForeignKey, String, Text, UniqueConstraint
from sqlalchemy.dialects.postgresql import UUID
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.domain.i18n import DEFAULT_LOCALE
from app.models.base import Base, TimestampMixin, UUIDPKMixin
from app.models.tag import Tag, post_tags


class PostStatus(str, enum.Enum):
    DRAFT = "draft"
    PENDING_REVIEW = "pending_review"
    PUBLISHED = "published"


class Post(Base, UUIDPKMixin, TimestampMixin):
    __tablename__ = "posts"

    blog_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("blogs.id"), nullable=False)
    author_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), nullable=False)
    # Categoria (tassonomia del blog, vedi app/models/category.py): al più
    # una per post, a differenza dei tag. Colonna semplice, non una
    # relationship — stesso motivo di `tags` sotto: evitare assegnazioni
    # ORM che possano innescare un lazy-load/flush sincrono in un contesto
    # async non awaited. ondelete SET NULL: cancellare la categoria non
    # cancella i post, li lascia solo senza categoria.
    category_id: Mapped[uuid.UUID | None] = mapped_column(
        ForeignKey("categories.id", ondelete="SET NULL"), nullable=True
    )
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
    # Immagine di copertina (URL pubblico su MinIO/S3, stesso bucket/prefisso
    # dei media incorporati nel contenuto — vedi POST /blogs/{slug}/media).
    cover_image_url: Mapped[str | None] = mapped_column(String(2048), nullable=True)
    # Risultato della moderazione automatica (app/domain/moderation.py) al
    # momento dell'upload della cover — il client lo riceve già nella
    # risposta di POST /blogs/{slug}/media e lo ripropone qui, non viene
    # ricalcolato lato server (stesso principio di fiducia già in atto per
    # cover_image_url stesso: non verificato contro un media reale).
    cover_image_is_sensitive: Mapped[bool] = mapped_column(Boolean, default=False, nullable=False)
    # Categorie di avviso scelte manualmente dall'autore tramite il modal
    # stile Bluesky (CLAUDE.md #3, vedi app/domain/content_media.py per il
    # vocabolario) — indipendenti da `cover_image_is_sensitive`, che resta il
    # flag che pilota davvero lo sfocamento: impostare una categoria qui forza
    # anche quello a True (vedi update_post), ma il flag può restare True per
    # sola segnalazione automatica anche a lista vuota.
    cover_image_categories: Mapped[list[str]] = mapped_column(ARRAY(String(20)), default=list, nullable=False)

    # Tag inseriti esplicitamente nel campo dedicato (vedi app/domain/tags.py):
    # SOLO quelli, non gli hashtag nel testo — serve a poterli ripresentare
    # in modifica senza perderli quando cambia solo il contenuto. L'insieme
    # effettivo (questi + gli hashtag estratti dal testo, max 5 in tutto) è
    # materializzato nella relazione `tags` sotto, per le query di tendenza.
    #
    # ATTENZIONE: non assegnare `post.tags = [...]` in un endpoint — il
    # back_populates bidirezionale fa scattare un flush implicito in
    # quell'istruzione sincrona, che fuori da un contesto async awaited fa
    # fallire la request (MissingGreenlet). Per scrivere l'associazione usa
    # la tabella `post_tags` (Core) direttamente — vedi
    # app/api/v1/posts.py:_sync_post_tags. La relazione resta comunque utile
    # in lettura, con un eager load esplicito (es. selectinload(Post.tags)).
    manual_tags: Mapped[list[str]] = mapped_column(ARRAY(String(30)), default=list, nullable=False)
    tags: Mapped[list[Tag]] = relationship(secondary=post_tags, back_populates="posts")

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
