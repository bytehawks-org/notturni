import uuid
from datetime import datetime

from sqlalchemy import DateTime, ForeignKey, String
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.models.base import Base, TimestampMixin, UUIDPKMixin


class EmailChangeRequest(Base, UUIDPKMixin, TimestampMixin):
    """Richiesta di cambio email, in due passi: un codice inviato alla
    vecchia casella (prova il possesso dell'account) e uno alla nuova
    (prova il possesso del nuovo indirizzo) — tabella dedicata invece di
    riusare `mfa_email_codes` per non confondere codici di login con codici
    di cambio email dello stesso utente."""

    __tablename__ = "email_change_requests"

    user_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), nullable=False)
    new_email: Mapped[str] = mapped_column(String(255), nullable=False)

    old_code_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    old_expires_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), nullable=False)
    old_consumed_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)

    new_code_hash: Mapped[str | None] = mapped_column(String(64), nullable=True)
    new_expires_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)
    new_consumed_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)

    completed_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)

    user: Mapped["User"] = relationship(back_populates="email_change_requests")
