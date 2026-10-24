"""Verifica Cloudflare Turnstile (ROADMAP.md §1) per i commenti aperti a
autori non registrati (`Blog`/`Post.comments_mode == "everyone"`)."""

import logging

import httpx

from app.core.config import settings

logger = logging.getLogger(__name__)

VERIFY_URL = "https://challenges.cloudflare.com/turnstile/v0/siteverify"
VERIFY_TIMEOUT_SECONDS = 10.0


def turnstile_configured() -> bool:
    return bool(settings.turnstile_secret_key)


async def verify_turnstile(token: str, remote_ip: str | None) -> bool:
    """True se il token del widget Turnstile è valido. Se il servizio non è
    raggiungibile o risponde in errore, la verifica fallisce (fail closed —
    a differenza della moderazione immagini, qui l'obiettivo è proprio
    bloccare lo spam, non un aiuto best-effort)."""
    if not settings.turnstile_secret_key:
        return False

    payload = {"secret": settings.turnstile_secret_key, "response": token}
    if remote_ip:
        payload["remoteip"] = remote_ip

    try:
        async with httpx.AsyncClient(timeout=VERIFY_TIMEOUT_SECONDS) as client:
            response = await client.post(VERIFY_URL, data=payload)
            response.raise_for_status()
            return bool(response.json().get("success"))
    except Exception:
        logger.warning("Verifica Turnstile fallita (servizio irraggiungibile o errore).", exc_info=True)
        return False
