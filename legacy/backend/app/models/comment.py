import enum
import uuid

from datetime import datetime

from sqlalchemy import Boolean, DateTime, Enum, ForeignKey, String, Text
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.models.base import Base, TimestampMixin, UUIDPKMixin


class CommentStatus(str, enum.Enum):
    PENDING = "pending"
    APPROVED = "approved"
    REJECTED = "rejected"


class CommentsMode(str, enum.Enum):
    """Chi può commentare (blog-level default, con override per singolo post
    — vedi Post.comments_mode). CLAUDE.md #1: `members` è il default di
    piattaforma; `everyone` richiede la verifica captcha lato client per gli
    autori non registrati (app/core/captcha.py)."""

    EVERYONE = "everyone"
    MEMBERS = "members"
    CLOSED = "closed"


def comments_mode_column(*, nullable: bool, default: CommentsMode | None):
    return mapped_column(
        Enum(
            CommentsMode,
            name="comments_mode",
            native_enum=True,
            values_callable=lambda enum_cls: [e.value for e in enum_cls],
        ),
        nullable=nullable,
        default=default,
    )


class Comment(Base, UUIDPKMixin, TimestampMixin):
    """CLAUDE.md #1: commenti solo utenti registrati di default; il proprietario del
    blog può aprirli anche ai non registrati, con moderazione obbligatoria in quel caso.
    La logica di assegnazione automatica dello status è demandata al service layer."""

    __tablename__ = "comments"

    post_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("posts.id"), nullable=False, index=True)
    # nullable: valorizzato solo se il blog consente commenti a utenti non registrati
    author_id: Mapped[uuid.UUID | None] = mapped_column(ForeignKey("users.id"), nullable=True)
    author_display_name: Mapped[str] = mapped_column(String(255))
    author_email: Mapped[str | None] = mapped_column(String(255), nullable=True)

    content: Mapped[str] = mapped_column(Text)
    status: Mapped[CommentStatus] = mapped_column(
        Enum(
            CommentStatus,
            name="comment_status",
            native_enum=True,
            values_callable=lambda enum_cls: [e.value for e in enum_cls],
        ),
        default=CommentStatus.PENDING,
        nullable=False,
    )
    # Risposta a un altro commento dello stesso post (thread): nullable, un
    # commento di primo livello non ne ha. Nessun limite di profondità — il
    # frontend rende comunque tutto su un solo livello di indentazione
    # visiva per restare leggibile.
    parent_id: Mapped[uuid.UUID | None] = mapped_column(
        ForeignKey("comments.id", ondelete="CASCADE"), nullable=True, index=True
    )

    # todo/UX_REDESIGN.md B4 (mockup 5b "Report to platform"): segnalazione
    # del proprietario/mediatore del blog ai moderatori di piattaforma, con
    # nota. Resta anche dopo la moderazione, come traccia.
    reported_to_platform: Mapped[bool] = mapped_column(Boolean, default=False, nullable=False)
    report_note: Mapped[str | None] = mapped_column(Text, nullable=True)
    reported_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)

    post: Mapped["Post"] = relationship(back_populates="comments")
    replies: Mapped[list["Comment"]] = relationship(
        back_populates="parent", cascade="all, delete-orphan"
    )
    parent: Mapped["Comment | None"] = relationship(back_populates="replies", remote_side="Comment.id")


class BlogBlockedAuthor(Base, UUIDPKMixin, TimestampMixin):
    """Lista dei bloccati per blog (mockup 5b "Block author"): un utente
    registrato (`user_id`) o, per i commenti anonimi, l'hash sha256 dell'email
    (`email_hash` — mai l'email in chiaro né l'IP). Un autore bloccato non può
    più commentare sui post del blog (`403`)."""

    __tablename__ = "blog_blocked_authors"

    blog_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("blogs.id", ondelete="CASCADE"), nullable=False, index=True)
    user_id: Mapped[uuid.UUID | None] = mapped_column(ForeignKey("users.id"), nullable=True)
    email_hash: Mapped[str | None] = mapped_column(String(64), nullable=True)
    # etichetta mostrata nella lista (username o "anonimo"), salvata per non
    # dover risolvere l'utente a ogni lettura e per gli anonimi
    label: Mapped[str] = mapped_column(String(255), nullable=False)
    note: Mapped[str | None] = mapped_column(String(255), nullable=True)
    created_by_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), nullable=False)
