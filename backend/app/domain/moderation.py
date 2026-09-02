"""Moderazione automatica delle immagini (nudità/contenuti sensibili),
self-hosted — vedi moderation/ (servizio separato, containerizzato apposta
per tenere le dipendenze ML fuori dall'immagine del backend).

Come per il backup dei post su S3 (vedi app/api/v1/posts.py:_backup_to_s3):
un problema di questo servizio ausiliario (irraggiungibile, timeout, errore)
non deve mai far fallire un upload altrimenti riuscito — si presume "non
sensibile" (fail open) e si logga soltanto. Non è pensato come barriera di
sicurezza legale, ma come aiuto automatico all'autore; resta comunque
possibile marcare/rimuovere un'immagine a mano."""

import logging

import httpx

from app.core.config import settings

logger = logging.getLogger(__name__)

CLASSIFY_TIMEOUT_SECONDS = 15.0


async def classify_image(content: bytes, filename: str, content_type: str) -> bool:
    """True se l'immagine è stata segnalata come possibile contenuto
    sensibile. Se il servizio di moderazione non è configurato
    (NOCT_MODERATION_SERVICE_URL assente) o non risponde, ritorna False."""
    if not settings.moderation_service_url:
        return False

    try:
        async with httpx.AsyncClient(timeout=CLASSIFY_TIMEOUT_SECONDS) as client:
            response = await client.post(
                f"{settings.moderation_service_url}/classify",
                files={"file": (filename, content, content_type or "application/octet-stream")},
            )
            response.raise_for_status()
            return bool(response.json()["is_sensitive"])
    except Exception:
        logger.warning("Servizio di moderazione non raggiungibile o in errore: nessun blocco.", exc_info=True)
        return False
