from fastapi import Request


def client_ip(request: Request | None) -> str | None:
    """IP del client, usato da audit log e rate limiting. In produzione il
    backend sta dietro Traefik, unico hop noto/fidato davanti al backend
    (nessun altro proxy nella catena): è Traefik stesso — non il client — a
    scrivere l'ultimo valore di X-Forwarded-For, appendendolo a quanto
    ricevuto. Fidarsi del *primo* valore (come faceva questo helper prima)
    permette a un client di spoofare l'IP mettendone uno a piacere in testa
    all'header; l'ultimo valore è invece sempre quello scritto dall'hop
    fidato. Se l'header manca (accesso diretto, es. da un port-forward in
    sviluppo) si ricade su request.client, l'IP del peer TCP diretto."""
    if request is None:
        return None
    forwarded = request.headers.get("x-forwarded-for")
    if forwarded:
        parts = [part.strip() for part in forwarded.split(",") if part.strip()]
        if parts:
            return parts[-1]
    return request.client.host if request.client else None
