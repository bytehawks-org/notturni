import enum
import uuid
from datetime import datetime

from sqlalchemy import DateTime, Enum, ForeignKey, String, UniqueConstraint
from sqlalchemy.orm import Mapped, mapped_column

from app.models.base import Base, TimestampMixin, UUIDPKMixin


class ReportTargetType(str, enum.Enum):
    BLOG = "blog"
    POST = "post"


class ReportReason(str, enum.Enum):
    SPAM = "spam"
    ABUSE = "abuse"
    ILLEGAL = "illegal"
    OTHER = "other"


class ReportStatus(str, enum.Enum):
    OPEN = "open"
    DISMISSED = "dismissed"
    ACTIONED = "actioned"


class ContentReport(Base, UUIDPKMixin, TimestampMixin):
    """Segnalazione di un blog o di un post da parte di un lettore registrato
    (todo/UX_REDESIGN.md B5, mockup 5e). Una per lettore e bersaglio; viene
    chiusa (`dismissed`/`actioned`) dall'azione admin con nota obbligatoria."""

    __tablename__ = "content_reports"

    reporter_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), nullable=False, index=True)
    target_type: Mapped[ReportTargetType] = mapped_column(
        Enum(ReportTargetType, name="report_target_type", native_enum=True, values_callable=lambda e: [x.value for x in e]),
        nullable=False,
    )
    target_id: Mapped[uuid.UUID] = mapped_column(nullable=False, index=True)
    # blog di appartenenza (il blog stesso, o il blog del post): per contare e
    # chiudere le segnalazioni per blog dal pannello admin
    blog_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("blogs.id", ondelete="CASCADE"), nullable=False, index=True)
    reason: Mapped[ReportReason] = mapped_column(
        Enum(ReportReason, name="report_reason", native_enum=True, values_callable=lambda e: [x.value for x in e]),
        nullable=False,
    )
    note: Mapped[str | None] = mapped_column(String(500), nullable=True)
    status: Mapped[ReportStatus] = mapped_column(
        Enum(ReportStatus, name="report_status", native_enum=True, values_callable=lambda e: [x.value for x in e]),
        default=ReportStatus.OPEN,
        nullable=False,
        index=True,
    )
    resolved_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)
    resolved_by_id: Mapped[uuid.UUID | None] = mapped_column(ForeignKey("users.id"), nullable=True)

    __table_args__ = (UniqueConstraint("reporter_id", "target_type", "target_id", name="uq_content_report_per_reporter"),)
