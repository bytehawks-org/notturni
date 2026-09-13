"""Coda richieste GDPR (todo/UX_REDESIGN.md B6): helper condivisi tra le
azioni self-service (registrate come completate) e il pannello admin."""

from datetime import datetime, timedelta, timezone

from sqlalchemy.ext.asyncio import AsyncSession

from app.models.gdpr_request import GdprRequest, GdprRequestStatus, GdprRequestType
from app.models.user import User

# Art. 12 GDPR: risposta entro un mese dalla ricezione
GDPR_DEADLINE_DAYS = 30


def new_request(*, user: User, type_: GdprRequestType, note: str | None = None, created_by_id=None) -> GdprRequest:
    now = datetime.now(timezone.utc)
    return GdprRequest(
        user_id=user.id,
        username=user.username,
        type=type_,
        status=GdprRequestStatus.OPEN,
        deadline_at=now + timedelta(days=GDPR_DEADLINE_DAYS),
        note=note,
        created_by_id=created_by_id,
    )


async def log_self_service_request(session: AsyncSession, *, user: User, type_: GdprRequestType, commit: bool = True) -> GdprRequest:
    """Export o cancellazione fatti dall'utente stesso: una riga già
    completata, per la traccia nel registro GDPR con la stessa scadenza."""
    req = new_request(user=user, type_=type_, note="self-service")
    req.status = GdprRequestStatus.COMPLETED
    req.completed_at = datetime.now(timezone.utc)
    session.add(req)
    if commit:
        await session.commit()
    return req
