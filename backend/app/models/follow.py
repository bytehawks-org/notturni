import uuid

from sqlalchemy import CheckConstraint, ForeignKey, UniqueConstraint
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.models.base import Base, TimestampMixin, UUIDPKMixin


class UserFollow(Base, UUIDPKMixin, TimestampMixin):
    __tablename__ = "user_follows"

    follower_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), nullable=False)
    followed_user_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), nullable=False)

    follower: Mapped["User"] = relationship(foreign_keys=[follower_id], back_populates="following")
    followed_user: Mapped["User"] = relationship(foreign_keys=[followed_user_id], back_populates="followers")

    __table_args__ = (
        UniqueConstraint("follower_id", "followed_user_id", name="uq_user_follow_pair"),
        CheckConstraint("follower_id != followed_user_id", name="ck_user_follow_not_self"),
    )


class BlogFollow(Base, UUIDPKMixin, TimestampMixin):
    __tablename__ = "blog_follows"

    follower_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), nullable=False)
    blog_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("blogs.id"), nullable=False)

    follower: Mapped["User"] = relationship(back_populates="followed_blogs")
    blog: Mapped["Blog"] = relationship(back_populates="follows")

    __table_args__ = (UniqueConstraint("follower_id", "blog_id", name="uq_blog_follow_pair"),)
