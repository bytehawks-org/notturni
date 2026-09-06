from fastapi import Request


def client_ip(request: Request | None) -> str | None:
    """IP del client, usato da audit log e rate limiting. In produzione il
    backend sta dietro Traefik: l'IP reale è nel primo hop di
    X-Forwarded-For, non in request.client (che sarebbe l'ingress)."""
    if request is None:
        return None
    forwarded = request.headers.get("x-forwarded-for")
    if forwarded:
        return forwarded.split(",")[0].strip() or None
    return request.client.host if request.client else None
