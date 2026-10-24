import uuid
from datetime import datetime

from sqlalchemy import DateTime, ForeignKey, String
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.models.base import Base, TimestampMixin, UUIDPKMixin


class PasswordResetCode(Base, UUIDPKMixin, TimestampMixin):
    """Codice OTP monouso per il flusso "password dimenticata" (CLAUDE.md #3),
    stesso pattern di MfaEmailCode — tabella dedicata (non riusa
    `mfa_email_codes`) per non confondere un codice di login con uno che, se
    intercettato, permette di *impostare* una nuova password."""

    __tablename__ = "password_reset_codes"

    user_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), nullable=False)
    code_hash: Mapped[str] = mapped_column(String(64))

    expires_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), nullable=False)
    consumed_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)

    user: Mapped["User"] = relationship(back_populates="password_reset_codes")
