"""Rate limiting basato su Redis (ROADMAP.md §3: "Redis per rate limiting e
lock distribuiti", finora deployato ma non usato da nessuna logica
applicativa).

Fail open, come app/domain/moderation.py: un Redis irraggiungibile non deve
mai bloccare una richiesta altrimenti legittima — un limite di frequenza è
una protezione aggiuntiva, non l'unica barriera di sicurezza."""

import logging

from fastapi import HTTPException, status

from app.core.redis import get_redis

logger = logging.getLogger(__name__)


async def enforce_rate_limit(key: str, *, limit: int, window_seconds: int, message: str) -> None:
    """Contatore a finestra fissa (INCR + EXPIRE sul primo incremento della
    finestra): oltre `limit` incrementi in `window_seconds` secondi su `key`
    solleva `429`."""
    try:
        redis = get_redis()
        count = await redis.incr(key)
        if count == 1:
            await redis.expire(key, window_seconds)
    except HTTPException:
        raise
    except Exception:
        logger.warning(
            "Redis non raggiungibile: rate limiting disattivato per questa richiesta.", exc_info=True
        )
        return

    if count > limit:
        raise HTTPException(status.HTTP_429_TOO_MANY_REQUESTS, message)
