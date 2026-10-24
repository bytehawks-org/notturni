import enum
import uuid
from datetime import datetime

from sqlalchemy import CheckConstraint, DateTime, Enum, ForeignKey, String
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.models.base import Base, TimestampMixin, UUIDPKMixin


class ApiTokenOwnerType(str, enum.Enum):
    """Distingue i token del motore core (nessun utente associato, uso interno/
    machine-to-machine) da quelli che, in futuro, gli utenti potranno emettere
    per interfacciarsi con l'API senza passare da editor o admin di blog."""

    CORE = "core"
    USER = "user"


class ApiToken(Base, UUIDPKMixin, TimestampMixin):
    __tablename__ = "api_tokens"

    name: Mapped[str] = mapped_column(String(255))
    owner_type: Mapped[ApiTokenOwnerType] = mapped_column(
        Enum(
            ApiTokenOwnerType,
            name="api_token_owner_type",
            native_enum=True,
            values_callable=lambda enum_cls: [e.value for e in enum_cls],
        ),
        nullable=False,
    )
    user_id: Mapped[uuid.UUID | None] = mapped_column(ForeignKey("users.id"), nullable=True)

    # primi caratteri del token in chiaro, per riconoscerlo nelle liste senza
    # doverlo esporre di nuovo (il valore completo è mostrato una sola volta)
    token_prefix: Mapped[str] = mapped_column(String(16))
    token_hash: Mapped[str] = mapped_column(String(64), unique=True, index=True)

    expires_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)
    last_used_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)
    revoked_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)

    user: Mapped["User"] = relationship(back_populates="api_tokens")

    __table_args__ = (
        CheckConstraint(
            "(owner_type = 'user' AND user_id IS NOT NULL) "
            "OR (owner_type = 'core' AND user_id IS NULL)",
            name="ck_api_token_owner_consistency",
        ),
    )
