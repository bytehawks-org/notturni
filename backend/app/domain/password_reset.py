"""Flusso "password dimenticata" — stesso pattern OTP dell'MFA email
(app/domain/mfa.py): codice a 6 cifre, hash sha256, TTL 10 minuti, accodato
su RabbitMQ. Tabella dedicata (`password_reset_codes`) invece di riusare
`mfa_email_codes`, per non confondere un codice di login con uno che, se
intercettato, permette di *impostare* una nuova password."""

import logging
import secrets
from datetime import datetime, timedelta, timezone

from sqlalchemy import delete, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.core.broker import publish_email_otp
from app.core.security import hash_password, sha256_hex
from app.domain.passwords import validate_password_policy
from app.models.password_reset_code import PasswordResetCode
from app.models.user import User
from app.models.user_session import UserSession

logger = logging.getLogger(__name__)

PASSWORD_RESET_OTP_TTL_MINUTES = 10
PASSWORD_RESET_OTP_LENGTH = 6


async def request_password_reset(session: AsyncSession, email: str) -> None:
    """Se l'email corrisponde a un account, accoda un codice di reset.
    Non solleva né segnala in alcun modo se l'email non esiste (l'endpoint
    risponde sempre allo stesso modo, vedi app/api/v1/auth.py): altrimenti
    l'esistenza di un account sarebbe enumerabile tentando email a caso."""
    result = await session.execute(select(User).where(User.email == email.strip().lower()))
    user = result.scalar_one_or_none()
    if user is None or not user.is_active:
        return

    code = "".join(secrets.choice("0123456789") for _ in range(PASSWORD_RESET_OTP_LENGTH))
    session.add(
        PasswordResetCode(
            user_id=user.id,
            code_hash=sha256_hex(code),
            expires_at=datetime.now(timezone.utc) + timedelta(minutes=PASSWORD_RESET_OTP_TTL_MINUTES),
        )
    )
    await session.commit()
    try:
        publish_email_otp(user.email, code, purpose="password_reset")
    except Exception:
        # Il codice resta comunque valido/utilizzabile: solo l'invio
        # dell'email è accodato via RabbitMQ. Non propagare qui, altrimenti
        # un broker temporaneamente giù farebbe rispondere 500 per un'email
        # esistente e 202 per una inesistente — l'endpoint deve rispondere
        # allo stesso modo in entrambi i casi (vedi docstring sopra).
        logger.warning("Impossibile accodare l'email OTP per il reset password.", exc_info=True)


async def reset_password(session: AsyncSession, *, email: str, code: str, new_password: str) -> None:
    """Verifica il codice e imposta la nuova password. Solleva `ValueError`
    per ogni motivo di rifiuto (email/codice non validi, password fuori
    policy) — stesso messaggio generico per email sconosciuta e codice
    sbagliato, di nuovo per non rendere l'email enumerabile."""
    validate_password_policy(new_password)

    result = await session.execute(select(User).where(User.email == email.strip().lower()))
    user = result.scalar_one_or_none()
    if user is None or not user.is_active:
        raise ValueError("Codice non valido o scaduto.")

    # FOR UPDATE: senza lock, due richieste di reset concorrenti con lo stesso
    # codice possono leggere entrambe la riga non ancora consumata prima che
    # una delle due faccia commit, superare entrambe il controllo sotto e
    # impostare due password diverse — un OTP pensato come monouso
    # redimibile due volte. Il lock blocca la seconda transazione fino al
    # commit della prima, che marca `consumed_at`.
    result = await session.execute(
        select(PasswordResetCode)
        .where(PasswordResetCode.user_id == user.id, PasswordResetCode.consumed_at.is_(None))
        .order_by(PasswordResetCode.created_at.desc())
        .with_for_update()
    )
    pending = result.scalars().first()
    now = datetime.now(timezone.utc)
    if pending is None or pending.expires_at < now or pending.code_hash != sha256_hex(code):
        raise ValueError("Codice non valido o scaduto.")

    pending.consumed_at = now
    user.hashed_password = hash_password(new_password)
    # Una password compromessa al punto da richiedere il reset invalida
    # anche ogni sessione già aperta altrove (stesso principio della
    # cancellazione account, app/domain/gdpr.py) — non solo il refresh token
    # eventualmente rubato, tutte.
    await session.execute(delete(UserSession).where(UserSession.user_id == user.id))
    await session.commit()
