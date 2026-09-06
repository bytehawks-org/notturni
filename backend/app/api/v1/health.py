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
async def public_config() -> dict[str, str | None]:
    """Pubblico, nessuna auth: usato dal dashboard per sapere se nascondere
    la voce Utenti in modalità "solo" senza dover già avere una sessione
    (CLAUDE.md #8, NOCT_DEPLOYMENT_MODE). `turnstile_site_key`: la site key
    di Cloudflare Turnstile (pensata per essere pubblica, a differenza della
    secret key — mai esposta) serve al frontend per sapere se può mostrare
    il widget captcha sui commenti aperti a tutti; `null` se non configurata."""
    return {
        "deployment_mode": settings.deployment_mode,
        "turnstile_site_key": settings.turnstile_site_key,
    }
