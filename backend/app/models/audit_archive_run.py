import uuid
from datetime import datetime

from sqlalchemy import DateTime, Integer, String, UniqueConstraint, func
from sqlalchemy.orm import Mapped, mapped_column

from app.models.base import Base, UUIDPKMixin


class AuditArchiveRun(Base, UUIDPKMixin):
    """Registro degli scarichi di `audit_log` su storage: una riga per
    settimana ISO archiviata (app/workers/audit_maintenance.py).

    Non è partizionato né soggetto a retention: è il libro mastro che dice
    fin dove il database è già stato riversato su storage. `period_end`
    dell'ultima riga fa da watermark — il job di cancellazione non elimina
    mai eventi più recenti di quel valore. Una settimana senza eventi viene
    comunque registrata (`row_count = 0`, `object_key = NULL`) per far
    avanzare il watermark senza riscansionarla."""

    __tablename__ = "audit_archive_runs"

    # estremi [inizio, fine) della settimana ISO, in UTC
    period_start: Mapped[datetime] = mapped_column(DateTime(timezone=True), nullable=False)
    period_end: Mapped[datetime] = mapped_column(DateTime(timezone=True), nullable=False)
    # etichetta leggibile e basename dell'oggetto: "2026w36"
    week_label: Mapped[str] = mapped_column(String(16), nullable=False)

    # chiave dell'oggetto su storage; NULL se la settimana non aveva eventi
    object_key: Mapped[str | None] = mapped_column(String(512), nullable=True)
    storage_backend: Mapped[str] = mapped_column(String(20), nullable=False)
    row_count: Mapped[int] = mapped_column(Integer, nullable=False)
    byte_size: Mapped[int] = mapped_column(Integer, nullable=False, server_default="0")
    # sha256 esadecimale del contenuto gzippato; NULL se row_count == 0
    sha256: Mapped[str | None] = mapped_column(String(64), nullable=True)

    archived_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now()
    )

    __table_args__ = (
        UniqueConstraint("period_start", name="uq_audit_archive_runs_period_start"),
    )
