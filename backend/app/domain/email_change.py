"""Cambio email con doppia verifica (vecchia + nuova casella).

Stesso pattern OTP dell'MFA email (app/domain/mfa.py): codice a 6 cifre,
hash sha256, TTL 10 minuti, accodato su RabbitMQ tramite lo stesso
`publish_email_otp` (payload-driven, nessuna modifica al worker consumer).
Tabella dedicata (`email_change_requests`) invece di riusare `mfa_email_codes`,
per non confondere i codici di login con quelli di cambio email dello stesso
utente in corsi paralleli."""

import secrets
from datetime import datetime, timedelta, timezone

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.core.broker import publish_email_otp
from app.core.security import sha256_hex
from app.models.email_change_request import EmailChangeRequest
from app.models.user import User

EMAIL_CHANGE_OTP_TTL_MINUTES = 10
EMAIL_CHANGE_OTP_LENGTH = 6


def _generate_code() -> str:
    return "".join(secrets.choice("0123456789") for _ in range(EMAIL_CHANGE_OTP_LENGTH))


async def _email_in_use(session: AsyncSession, email: str, *, exclude_user_id) -> bool:
    result = await session.execute(
        select(User.id).where(User.email == email, User.id != exclude_user_id)
    )
    return result.scalar_one_or_none() is not None


async def _get_pending_request(session: AsyncSession, user: User) -> EmailChangeRequest | None:
    result = await session.execute(
        select(EmailChangeRequest)
        .where(EmailChangeRequest.user_id == user.id, EmailChangeRequest.completed_at.is_(None))
        .order_by(EmailChangeRequest.created_at.desc())
    )
    return result.scalars().first()


async def request_email_change(session: AsyncSession, user: User, new_email: str) -> EmailChangeRequest:
    """Passo 1: invia un codice alla casella *attuale*, a prova che chi
    controlla la sessione controlla anche quella casella."""
    new_email = new_email.strip().lower()
    if new_email == user.email.lower():
        raise ValueError("Il nuovo indirizzo coincide con quello attuale.")
    if await _email_in_use(session, new_email, exclude_user_id=user.id):
        raise ValueError("Email già in uso da un altro account.")

    # una richiesta pending precedente non completata viene sostituita
    existing = await _get_pending_request(session, user)
    if existing is not None:
        await session.delete(existing)

    code = _generate_code()
    now = datetime.now(timezone.utc)
    change_request = EmailChangeRequest(
        user_id=user.id,
        new_email=new_email,
        old_code_hash=sha256_hex(code),
        old_expires_at=now + timedelta(minutes=EMAIL_CHANGE_OTP_TTL_MINUTES),
    )
    session.add(change_request)
    await session.commit()
    await session.refresh(change_request)
    publish_email_otp(user.email, code)
    return change_request


async def confirm_old_email(session: AsyncSession, user: User, code: str) -> EmailChangeRequest:
    """Passo 2: verifica il codice inviato alla vecchia casella, poi invia
    il codice alla nuova."""
    pending = await _get_pending_request(session, user)
    if pending is None:
        raise ValueError("Nessuna richiesta di cambio email in corso.")
    if pending.old_consumed_at is not None:
        raise ValueError("Passo già completato.")
    now = datetime.now(timezone.utc)
    if pending.old_expires_at < now or pending.old_code_hash != sha256_hex(code):
        raise ValueError("Codice non valido o scaduto.")
    if await _email_in_use(session, pending.new_email, exclude_user_id=user.id):
        raise ValueError("Email già in uso da un altro account.")

    new_code = _generate_code()
    pending.old_consumed_at = now
    pending.new_code_hash = sha256_hex(new_code)
    pending.new_expires_at = now + timedelta(minutes=EMAIL_CHANGE_OTP_TTL_MINUTES)
    await session.commit()
    await session.refresh(pending)
    publish_email_otp(pending.new_email, new_code)
    return pending


async def confirm_new_email(session: AsyncSession, user: User, code: str) -> str:
    """Passo 3: verifica il codice inviato alla nuova casella e applica il
    cambio. Ritorna la nuova email. **Non fa il commit**: lasciato al
    chiamante per poter aggiungere l'evento di audit nella stessa
    transazione (app/domain/audit.py, atomicità evento+log)."""
    pending = await _get_pending_request(session, user)
    if pending is None or pending.old_consumed_at is None or pending.new_code_hash is None:
        raise ValueError("Nessuna richiesta di cambio email pronta per la conferma finale.")
    now = datetime.now(timezone.utc)
    if pending.new_expires_at is None or pending.new_expires_at < now or pending.new_code_hash != sha256_hex(code):
        raise ValueError("Codice non valido o scaduto.")
    if await _email_in_use(session, pending.new_email, exclude_user_id=user.id):
        raise ValueError("Email già in uso da un altro account.")

    old_email = user.email
    user.email = pending.new_email
    pending.new_consumed_at = now
    pending.completed_at = now
    return old_email
