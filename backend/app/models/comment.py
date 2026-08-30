import enum
import uuid

from sqlalchemy import Enum, ForeignKey, String, Text
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.models.base import Base, TimestampMixin, UUIDPKMixin


class CommentStatus(str, enum.Enum):
    PENDING = "pending"
    APPROVED = "approved"
    REJECTED = "rejected"


class Comment(Base, UUIDPKMixin, TimestampMixin):
    """CLAUDE.md #1: commenti solo utenti registrati di default; il proprietario del
    blog può aprirli anche ai non registrati, con moderazione obbligatoria in quel caso.
    La logica di assegnazione automatica dello status è demandata al service layer."""

    __tablename__ = "comments"

    post_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("posts.id"), nullable=False)
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

    post: Mapped["Post"] = relationship(back_populates="comments")
