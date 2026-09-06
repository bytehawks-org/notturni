from fastapi import APIRouter, HTTPException, Request, status
from pydantic import BaseModel

from app.core.http import client_ip
from app.domain.link_preview import fetch_link_preview, validate_previewable_url
from app.domain.rate_limit import enforce_rate_limit

router = APIRouter()

# Pubblico e senza cache (ogni chiamata fa una fetch server-side dell'URL
# indicato): un limite più stretto degli altri endpoint pubblici per ridurre
# il rischio di abuso come proxy/scanner verso terzi (ROADMAP.md §3).
LINK_PREVIEW_RATE_LIMIT = 30
LINK_PREVIEW_RATE_LIMIT_WINDOW_SECONDS = 60


class LinkPreviewOut(BaseModel):
    url: str
    title: str | None = None
    description: str | None = None
    image: str | None = None


@router.get("", response_model=LinkPreviewOut)
async def get_link_preview(url: str, request: Request) -> LinkPreviewOut:
    """Pubblico, senza autenticazione (CLAUDE.md #1): usato sia dall'editor
    (anteprima mentre si scrive) sia dal rendering della pagina pubblica del
    post per i link marcati come card (`[testo](url "card")`, vedi
    frontend/src/lib/markdown.ts)."""
    ip = client_ip(request)
    if ip is not None:
        await enforce_rate_limit(
            f"ratelimit:link-preview:{ip}",
            limit=LINK_PREVIEW_RATE_LIMIT,
            window_seconds=LINK_PREVIEW_RATE_LIMIT_WINDOW_SECONDS,
            message="Troppe richieste di anteprima link. Riprova tra qualche istante.",
        )

    try:
        validate_previewable_url(url)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    preview = await fetch_link_preview(url)
    return LinkPreviewOut(
        url=preview.url, title=preview.title, description=preview.description, image=preview.image
    )
