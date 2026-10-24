import enum
import uuid
from datetime import datetime

from sqlalchemy import DateTime, Enum, ForeignKey, String
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.models.base import Base, TimestampMixin, UUIDPKMixin


class CustomDomainStatus(str, enum.Enum):
    PENDING = "pending"
    VERIFIED = "verified"
    FAILED = "failed"


class CustomDomain(Base, UUIDPKMixin, TimestampMixin):
    """Dominio personalizzato dell'utente (stile Bluesky), verificato tramite
    un record TXT sul DNS di proprietà — lo username di piattaforma resta
    comunque il fallback identificativo, mai sostituito a livello di
    routing/permalink (CLAUDE.md #5)."""

    __tablename__ = "custom_domains"

    user_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), unique=True, nullable=False)
    domain: Mapped[str] = mapped_column(String(255), unique=True, nullable=False)
    verification_token: Mapped[str] = mapped_column(String(64), nullable=False)
    status: Mapped[CustomDomainStatus] = mapped_column(
        Enum(
            CustomDomainStatus,
            name="custom_domain_status",
            native_enum=True,
            values_callable=lambda enum_cls: [e.value for e in enum_cls],
        ),
        default=CustomDomainStatus.PENDING,
        nullable=False,
    )
    verified_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)

    user: Mapped["User"] = relationship(back_populates="custom_domain")
