"""Registro di audit (append-only) delle azioni sensibili: autenticazione,
amministrazione di piattaforma, moderazione.

`record()` aggiunge la riga alla **stessa sessione/transazione** dell'azione
che si sta tracciando, senza `flush` né `commit` propri: la riga di audit
viene persistita dallo stesso `commit` del chiamante, quindi esiste se e solo
se quell'azione è andata a buon fine. È una scelta deliberata — a differenza
del backup dei post o dell'OTP email (accodati su RabbitMQ), qui l'atomicità
con l'evento conta più del disaccoppiamento, e un log di sicurezza non deve
poter divergere dallo stato reale.

Retention a database (cancellazione oltre `NOCT_AUDIT_RETENTION_DAYS`) e
scarico periodico su storage: blocchi successivi, vedi ROADMAP § 3.
"""

import uuid
from typing import Any

from fastapi import Request
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.audit_log import AuditActorType, AuditLog
from app.models.user import User


def _client_ip(request: Request | None) -> str | None:
    if request is None:
        return None
    # in produzione il backend sta dietro Traefik: l'IP reale del client è nel
    # primo hop di X-Forwarded-For, non in request.client (che sarebbe l'ingress)
    forwarded = request.headers.get("x-forwarded-for")
    if forwarded:
        return forwarded.split(",")[0].strip() or None
    return request.client.host if request.client else None


def actor_label_for(user: User) -> str:
    """Snapshot leggibile dell'utente da salvare in `audit_log.actor_label`."""
    return f"{user.username} <{user.email}>"


async def record(
    session: AsyncSession,
    *,
    action: str,
    actor: User | None = None,
    actor_type: AuditActorType | None = None,
    actor_id: uuid.UUID | None = None,
    actor_label: str | None = None,
    target_type: str | None = None,
    target_id: uuid.UUID | None = None,
    blog_id: uuid.UUID | None = None,
    request: Request | None = None,
    payload: dict[str, Any] | None = None,
) -> None:
    """Registra un evento. Passare `actor` (un `User`) popola in automatico
    `actor_type=user`, `actor_id` e `actor_label`; in alternativa si passano
    i campi `actor_*` espliciti (accesso via API token, richiesta anonima,
    processo di sistema). Senza nessuno dei due l'attore è `system`."""
    if actor is not None:
        actor_type = actor_type or AuditActorType.USER
        actor_id = actor_id if actor_id is not None else actor.id
        actor_label = actor_label or actor_label_for(actor)
    elif actor_type is None:
        actor_type = AuditActorType.SYSTEM

    session.add(
        AuditLog(
            action=action,
            actor_type=actor_type,
            actor_id=actor_id,
            actor_label=actor_label,
            target_type=target_type,
            target_id=target_id,
            blog_id=blog_id,
            ip=_client_ip(request),
            user_agent=(request.headers.get("user-agent") if request is not None else None),
            payload=payload or {},
        )
    )
