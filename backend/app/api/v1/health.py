from fastapi import APIRouter, Depends
from sqlalchemy import text
from sqlalchemy.ext.asyncio import AsyncSession

from app.core.config import settings
from app.core.database import get_session

router = APIRouter()


@router.get("/health")
async def health(session: AsyncSession = Depends(get_session)) -> dict[str, str]:
    await session.execute(text("SELECT 1"))
    return {"status": "ok"}


@router.get("/config")
async def public_config() -> dict[str, str]:
    """Pubblico, nessuna auth: usato da frontend/admin/ per sapere se
    nascondere le sezioni multi-utente in modalità "solo" senza dover già
    avere una sessione (CLAUDE.md #8, NOCT_DEPLOYMENT_MODE)."""
    return {"deployment_mode": settings.deployment_mode}
