import enum
import uuid

from sqlalchemy import Boolean, Enum, String, Text
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.models.base import Base, TimestampMixin, UUIDPKMixin


class PlatformRole(str, enum.Enum):
    """Ruolo dell'utente sulla piattaforma (CLAUDE.md #1)."""

    SUPER_ADMIN = "super_admin"
    AMMINISTRATORE = "amministratore"
    MODERATORE = "moderatore"
    UTENTE = "utente"


class MfaMethod(str, enum.Enum):
    TOTP = "totp"
    EMAIL = "email"


class User(Base, UUIDPKMixin, TimestampMixin):
    __tablename__ = "users"

    username: Mapped[str] = mapped_column(String(32), unique=True, index=True)
    email: Mapped[str] = mapped_column(String(255), unique=True, index=True)
    # nullable: un utente creato solo via SSO può non avere una password locale
    hashed_password: Mapped[str | None] = mapped_column(String(255), nullable=True)

    platform_role: Mapped[PlatformRole] = mapped_column(
        Enum(
            PlatformRole,
            name="platform_role",
            native_enum=True,
            values_callable=lambda enum_cls: [e.value for e in enum_cls],
        ),
        default=PlatformRole.UTENTE,
        nullable=False,
    )
    is_active: Mapped[bool] = mapped_column(Boolean, default=True, nullable=False)

    # MFA (CLAUDE.md #3)
    mfa_enabled: Mapped[bool] = mapped_column(Boolean, default=False, nullable=False)
    mfa_method: Mapped[MfaMethod | None] = mapped_column(
        Enum(
            MfaMethod,
            name="mfa_method",
            native_enum=True,
            values_callable=lambda enum_cls: [e.value for e in enum_cls],
        ),
        nullable=True,
    )
    # per il metodo TOTP: impostato (non confermato) durante il setup, poi
    # attivo solo quando mfa_enabled=True. Per il metodo email non è usato.
    mfa_totp_secret: Mapped[str | None] = mapped_column(String(64), nullable=True)

    # Profilo pubblico (CLAUDE.md #1)
    bio: Mapped[str | None] = mapped_column(Text, nullable=True)
    # object key su MinIO (bucket avatars), non l'URL: quello si genera al volo
    # (app/core/storage.py) per restare compatibili tra MinIO locale e S3/R2 in
    # produzione senza persistere un host specifico.
    avatar_object_key: Mapped[str | None] = mapped_column(String(255), nullable=True)

    blogs: Mapped[list["Blog"]] = relationship(back_populates="owner")
    memberships: Mapped[list["BlogMembership"]] = relationship(back_populates="user")
    api_tokens: Mapped[list["ApiToken"]] = relationship(back_populates="user")
    sessions: Mapped[list["UserSession"]] = relationship(back_populates="user")
    mfa_email_codes: Mapped[list["MfaEmailCode"]] = relationship(back_populates="user")
    sso_identities: Mapped[list["SsoIdentity"]] = relationship(back_populates="user")
    social_links: Mapped[list["SocialLink"]] = relationship(
        back_populates="user", cascade="all, delete-orphan", order_by="SocialLink.position"
    )
    followers: Mapped[list["UserFollow"]] = relationship(
        foreign_keys="UserFollow.followed_user_id", back_populates="followed_user"
    )
    following: Mapped[list["UserFollow"]] = relationship(
        foreign_keys="UserFollow.follower_id", back_populates="follower"
    )
    followed_blogs: Mapped[list["BlogFollow"]] = relationship(back_populates="follower")
