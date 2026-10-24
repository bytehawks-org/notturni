import enum
import uuid
from datetime import datetime

from sqlalchemy import DateTime, Enum, ForeignKey, String, Text
from sqlalchemy.orm import Mapped, mapped_column

from app.models.base import Base, TimestampMixin, UUIDPKMixin


class GdprRequestType(str, enum.Enum):
    EXPORT = "export"
    DELETION = "deletion"


class GdprRequestStatus(str, enum.Enum):
    OPEN = "open"
    APPROVED = "approved"
    COMPLETED = "completed"
    REJECTED = "rejected"


class GdprRequest(Base, UUIDPKMixin, TimestampMixin):
    """Coda delle richieste GDPR (todo/UX_REDESIGN.md B6, mockup 5f): sia le
    azioni self-service (registrate come `completed`, per la traccia con
    scadenza legale) sia le richieste arrivate fuori banda e inserite da un
    admin, che per la cancellazione richiedono l'approvazione di un
    **secondo** admin prima dell'esecuzione."""

    __tablename__ = "gdpr_requests"

    user_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), nullable=False, index=True)
    # snapshot dello username al momento della richiesta (dopo la cancellazione
    # l'account è anonimizzato)
    username: Mapped[str] = mapped_column(String(32), nullable=False)
    type: Mapped[GdprRequestType] = mapped_column(
        Enum(GdprRequestType, name="gdpr_request_type", native_enum=True, values_callable=lambda e: [x.value for x in e]),
        nullable=False,
    )
    status: Mapped[GdprRequestStatus] = mapped_column(
        Enum(GdprRequestStatus, name="gdpr_request_status", native_enum=True, values_callable=lambda e: [x.value for x in e]),
        nullable=False,
        default=GdprRequestStatus.OPEN,
        index=True,
    )
    # scadenza legale: 30 giorni dalla ricezione (Art. 12 GDPR)
    deadline_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), nullable=False)
    note: Mapped[str | None] = mapped_column(Text, nullable=True)
    created_by_id: Mapped[uuid.UUID | None] = mapped_column(ForeignKey("users.id"), nullable=True)
    approved_by_id: Mapped[uuid.UUID | None] = mapped_column(ForeignKey("users.id"), nullable=True)
    approved_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)
    completed_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)
