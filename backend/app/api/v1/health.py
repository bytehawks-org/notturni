from fastapi import APIRouter, Depends
from sqlalchemy import text
from sqlalchemy.ext.asyncio import AsyncSession

from app.core.config import settings
from app.core.oauth import configured_providers
from app.domain.platform_config import get_platform_config
from app.core.database import get_session
from app.domain.seo import AI_CRAWLER_USER_AGENTS, build_crawl_directives, build_sitemap_entries

router = APIRouter()


@router.get("/health")
async def health(session: AsyncSession = Depends(get_session)) -> dict[str, str]:
    await session.execute(text("SELECT 1"))
    return {"status": "ok"}


@router.get("/config")
async def public_config(session: AsyncSession = Depends(get_session)) -> dict[str, object]:
    """Pubblico, nessuna auth: usato dal dashboard per sapere se nascondere
    la voce Utenti in modalità "solo" senza dover già avere una sessione
    (CLAUDE.md #8, NOCT_DEPLOYMENT_MODE). `turnstile_site_key`: la site key
    di Cloudflare Turnstile (pensata per essere pubblica, a differenza della
    secret key — mai esposta) serve al frontend per sapere se può mostrare
    il widget captcha sui commenti aperti a tutti; `null` se non configurata."""
    platform = await get_platform_config(session)
    return {
        "deployment_mode": settings.deployment_mode,
        "turnstile_site_key": settings.turnstile_site_key,
        # B6: valori pubblici di platform_config (lingua predefinita
        # dell'interfaccia, registrazioni aperte, provider SSO abilitati)
        "default_locale": platform.default_locale,
        "registration_mode": platform.registration_mode,
        "sso_providers": sorted(
            configured_providers() if not platform.sso_providers else configured_providers() & set(platform.sso_providers)
        ),
    }


@router.get("/footer")
async def public_footer(session: AsyncSession = Depends(get_session)) -> dict[str, str | None]:
    """Pubblico, nessuna auth: footer mostrato su ogni pagina pubblica di
    piattaforma e di ogni blog (richiesta esplicita). Markdown grezzo, non
    ancora renderizzato (il frontend lo fa al momento della lettura, stesso
    principio dei post — vedi API.md). Le colonne 1/2 sono solo il *default*:
    un blog può sovrascriverle per sé in `blog_configs.footer` (mai la
    colonna 3 né `bottom_bar`, sempre e solo di piattaforma)."""
    platform = await get_platform_config(session)
    return {
        "column1": platform.footer_column1_markdown,
        "column2": platform.footer_column2_markdown,
        "column3": platform.footer_column3_markdown,
        "bottom_bar": platform.footer_bottom_bar_markdown,
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
