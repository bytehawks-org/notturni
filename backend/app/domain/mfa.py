import secrets
from datetime import datetime, timedelta, timezone

import pyotp
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.core.broker import publish_email_otp
from app.core.security import sha256_hex
from app.models.mfa_email_code import MfaEmailCode
from app.models.user import User

EMAIL_OTP_TTL_MINUTES = 10
EMAIL_OTP_LENGTH = 6


def generate_totp_secret() -> str:
    return pyotp.random_base32()


def totp_provisioning_uri(secret: str, email: str) -> str:
    return pyotp.TOTP(secret).provisioning_uri(name=email, issuer_name="Notturni")


def verify_totp_code(secret: str, code: str) -> bool:
    return pyotp.TOTP(secret).verify(code, valid_window=1)


async def send_email_otp(session: AsyncSession, user: User) -> None:
    code = "".join(secrets.choice("0123456789") for _ in range(EMAIL_OTP_LENGTH))
    session.add(
        MfaEmailCode(
            user_id=user.id,
            code_hash=sha256_hex(code),
            expires_at=datetime.now(timezone.utc) + timedelta(minutes=EMAIL_OTP_TTL_MINUTES),
        )
    )
    await session.commit()
    publish_email_otp(user.email, code)


async def verify_email_otp(session: AsyncSession, user: User, code: str) -> bool:
    result = await session.execute(
        select(MfaEmailCode)
        .where(MfaEmailCode.user_id == user.id, MfaEmailCode.consumed_at.is_(None))
        .order_by(MfaEmailCode.created_at.desc())
    )
    pending = result.scalars().first()

    if pending is None or pending.expires_at < datetime.now(timezone.utc):
        return False
    if pending.code_hash != sha256_hex(code):
        return False

    pending.consumed_at = datetime.now(timezone.utc)
    await session.commit()
    return True
