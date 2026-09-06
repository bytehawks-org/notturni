from redis.asyncio import Redis

from app.core.config import settings

_redis: Redis | None = None


def get_redis() -> Redis:
    """Client Redis condiviso (rate limiting, app/domain/rate_limit.py — non
    ancora usato per altro). Connessione lazy, riusata tra le richieste."""
    global _redis
    if _redis is None:
        _redis = Redis(
            host=settings.redis_host, port=settings.redis_port, db=settings.redis_db, decode_responses=True
        )
    return _redis
