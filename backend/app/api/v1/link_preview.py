from fastapi import APIRouter, HTTPException, status
from pydantic import BaseModel

from app.domain.link_preview import fetch_link_preview, validate_previewable_url

router = APIRouter()


class LinkPreviewOut(BaseModel):
    url: str
    title: str | None = None
    description: str | None = None
    image: str | None = None


@router.get("", response_model=LinkPreviewOut)
async def get_link_preview(url: str) -> LinkPreviewOut:
    """Pubblico, senza autenticazione (CLAUDE.md #1): usato sia dall'editor
    (anteprima mentre si scrive) sia dal rendering della pagina pubblica del
    post per i link marcati come card (`[testo](url "card")`, vedi
    frontend/src/lib/markdown.ts)."""
    try:
        validate_previewable_url(url)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    preview = await fetch_link_preview(url)
    return LinkPreviewOut(
        url=preview.url, title=preview.title, description=preview.description, image=preview.image
    )
