"""Invalidazione on-demand della cache dei Server Component del frontend
(Next.js) dopo una modifica a un contenuto pubblico.

Fire-and-forget: il database resta la fonte di verità: un frontend
irraggiungibile, lento o che risponde con errore non deve mai far fallire la
richiesta che ha appena modificato il contenuto. In quel caso la cache si
riallinea comunque entro la finestra di rivalidazione a tempo del frontend
(`REVALIDATE_SECONDS` in `frontend/src/lib/revalidate.ts`).

Le stringhe dei tag prodotte qui devono restare identiche a quelle usate dal
frontend in `frontend/src/lib/revalidate.ts` — un disallineamento rompe
l'invalidazione mirata senza errori visibili.
"""

import logging

import httpx

from app.core.config import settings

logger = logging.getLogger(__name__)

_TIMEOUT = httpx.Timeout(2.0)


def feed_tag() -> str:
    return "feed"


def blog_tag(slug: str) -> str:
    return f"blog:{slug}"


def post_tag(blog_slug: str, post_slug: str) -> str:
    return f"post:{blog_slug}:{post_slug}"


def platform_pages_tag() -> str:
    return "platform-pages"


def platform_page_tag(slug: str) -> str:
    return f"platform-page:{slug}"


def blog_page_tag(blog_slug: str, page_slug: str) -> str:
    return f"blog-page:{blog_slug}:{page_slug}"


async def revalidate_frontend(tags: list[str]) -> None:
    """Invalida i tag indicati sulla cache del frontend. Da chiamare dopo il
    `commit` di un path di scrittura. Non solleva mai: logga un warning e
    prosegue. Quando il frontend è raggiungibile è una POST locale veloce; il
    timeout di 2s limita il caso in cui non lo sia."""
    if not settings.frontend_revalidate_url or not settings.revalidate_secret:
        return
    tags = [t for t in dict.fromkeys(tags) if t]
    if not tags:
        return
    try:
        async with httpx.AsyncClient(timeout=_TIMEOUT) as client:
            resp = await client.post(
                settings.frontend_revalidate_url,
                json={"tags": tags},
                headers={"Authorization": f"Bearer {settings.revalidate_secret}"},
            )
        if resp.status_code >= 400:
            logger.warning(
                "Revalidate frontend: risposta %s per i tag %s", resp.status_code, tags
            )
    except Exception:
        logger.warning(
            "Revalidate frontend non riuscito per i tag %s", tags, exc_info=True
        )
