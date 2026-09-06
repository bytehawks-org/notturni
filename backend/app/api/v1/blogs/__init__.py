"""Package `blogs`: l'API dei blog, divisa per risorsa. Tutti i sotto-moduli
registrano le proprie rotte sullo stesso `router` (in `_router.py`); qui li si
importa nell'ordine in cui le rotte vanno registrate.

Ordine rilevante solo per le rotte che si sovrappongono: `invitations` va
prima di `crud` perché `GET /received-invitations` non venga catturato da
`GET /{slug}`. Tutte le altre rotte hanno un secondo segmento letterale
(`/config`, `/media`, `/categories`, ...) e non collidono con `/{slug}`."""

from app.api.v1.blogs._router import router

from app.api.v1.blogs import (  # noqa: F401,E402  (import per side-effect: registra le rotte)
    invitations,
    crud,
    config,
    media,
    categories,
    bibliography,
    pages,
    members,
)

__all__ = ["router"]
