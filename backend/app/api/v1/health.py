from fastapi import APIRouter, Depends
from sqlalchemy import text
from sqlalchemy.ext.asyncio import AsyncSession

from app.core.config import settings
from app.core.database import get_session
from app.domain.seo import AI_CRAWLER_USER_AGENTS, build_crawl_directives, build_sitemap_entries

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


@router.get("/seo/crawl-directives")
async def crawl_directives(session: AsyncSession = Depends(get_session)) -> dict[str, list[str]]:
    """Pubblico, nessuna auth: usato da `frontend/src/app/robots.ts` per
    generare robots.txt (app/domain/seo.py). `ai_user_agents` è l'elenco fisso
    di crawler IA/LLM noti da abbinare a `ai_disallow`; i motori di ricerca
    tradizionali restano sul gruppo `User-agent: *` con `search_disallow`."""
    directives = await build_crawl_directives(session)
    return {**directives, "ai_user_agents": AI_CRAWLER_USER_AGENTS}


@router.get("/seo/sitemap-entries")
async def sitemap_entries(session: AsyncSession = Depends(get_session)) -> dict[str, list[dict[str, str]]]:
    """Pubblico, nessuna auth: usato da `frontend/src/app/sitemap.ts` per
    generare `/sitemap.xml` (app/domain/seo.py::build_sitemap_entries) — solo
    contenuto effettivamente indicizzabile (coerente con `search_disallow` di
    /seo/crawl-directives sopra)."""
    return await build_sitemap_entries(session)
