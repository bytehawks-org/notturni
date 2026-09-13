import uuid
from datetime import date

from sqlalchemy import Date, ForeignKey, Integer
from sqlalchemy.orm import Mapped, mapped_column

from app.models.base import Base


class PostReadDaily(Base):
    """Letture di un post aggregate per giorno (todo/UX_REDESIGN.md B2,
    mockup 5a "Reads · last 30 days"). Privacy by design: nessun cookie,
    nessun identificativo del lettore, nessun IP persistito — solo un
    contatore per (post, giorno UTC), incrementato da
    `POST /posts/{id}/read`. Il de-duplicare a breve termine passa dal rate
    limiting in Redis (volatile), non da dati salvati qui."""

    __tablename__ = "post_reads_daily"

    post_id: Mapped[uuid.UUID] = mapped_column(
        ForeignKey("posts.id", ondelete="CASCADE"), primary_key=True
    )
    day: Mapped[date] = mapped_column(Date, primary_key=True)
    reads: Mapped[int] = mapped_column(Integer, nullable=False, default=0)
