import enum
import uuid

from sqlalchemy import Enum, ForeignKey, String, Text
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

    post: Mapped["Post"] = relationship(back_populates="comments")
    replies: Mapped[list["Comment"]] = relationship(
        back_populates="parent", cascade="all, delete-orphan"
    )
    parent: Mapped["Comment | None"] = relationship(back_populates="replies", remote_side="Comment.id")
