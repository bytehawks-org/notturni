import uuid

from sqlalchemy import Boolean, ForeignKey, Index, String, Text, UniqueConstraint, text
from sqlalchemy.dialects.postgresql import UUID
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.domain.i18n import DEFAULT_LOCALE
from app.models.base import Base, TimestampMixin, UUIDPKMixin


class Page(Base, UUIDPKMixin, TimestampMixin):
    """Pagine statiche: del sito principale notturni.eu (Chi siamo, Contatti,
    Privacy, ecc. — `blog_id` NULL) oppure di un blog utente, feature opt-in
    (`Blog.static_pages_enabled`, vedi app/models/blog.py). Stesso schema di
    traduzione di Post: le traduzioni condividono translation_group_id."""

    __tablename__ = "pages"

    # NULL = pagina di piattaforma (Amministratore/Super Admin). Valorizzato
    # = pagina del blog (proprietario/co-autore), CLAUDE.md #1: "Add capability
    # to blog owner (Author) to create static pages".
    blog_id: Mapped[uuid.UUID | None] = mapped_column(
        ForeignKey("blogs.id", ondelete="CASCADE"), nullable=True, index=True
    )
    blog: Mapped["Blog | None"] = relationship(back_populates="pages")

    slug: Mapped[str] = mapped_column(String(255), index=True)
    locale: Mapped[str] = mapped_column(String(2), default=DEFAULT_LOCALE, nullable=False)
    translation_group_id: Mapped[uuid.UUID] = mapped_column(
        UUID(as_uuid=True), default=uuid.uuid4, nullable=False, index=True
    )

    title: Mapped[str] = mapped_column(String(255))
    content: Mapped[str] = mapped_column(Text)
    is_published: Mapped[bool] = mapped_column(Boolean, default=False, nullable=False)

    # ultimo utente (amministratore o proprietario/co-autore del blog) che ha
    # modificato la pagina
    updated_by_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), nullable=False)
    updated_by: Mapped["User"] = relationship()

    __table_args__ = (
        # Pagine di blog: slug unico per blog e lingua (stesso pattern di
        # Post.uq_post_blog_slug_locale). Non copre le pagine di piattaforma
        # (blog_id NULL): in Postgres due NULL non collidono su un vincolo
        # unique — vedi l'indice parziale sotto.
        UniqueConstraint("blog_id", "slug", "locale", name="uq_page_blog_slug_locale"),
        UniqueConstraint("translation_group_id", "locale", name="uq_page_translation_group_locale"),
        Index(
            "uq_page_slug_locale_platform",
            "slug",
            "locale",
            unique=True,
            postgresql_where=text("blog_id IS NULL"),
        ),
    )
