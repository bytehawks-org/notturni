import hashlib
import secrets
import uuid
from datetime import datetime, timedelta, timezone
from typing import Any, Literal

import jwt
from argon2 import PasswordHasher
from argon2.exceptions import VerifyMismatchError

from app.core.config import settings

_hasher = PasswordHasher()

JWT_ALGORITHM = "HS256"


def sha256_hex(value: str) -> str:
    """Hash generico per token opachi (API token, refresh token, codici OTP):
    non serve un derivation function costoso come per le password, il valore
    in chiaro ha già alta entropia (generato con secrets)."""
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def generate_opaque_secret(num_bytes: int = 32) -> str:
    return secrets.token_urlsafe(num_bytes)


def hash_password(plain: str) -> str:
    return _hasher.hash(plain)


def verify_password(plain: str, hashed: str) -> bool:
    try:
        return _hasher.verify(hashed, plain)
    except VerifyMismatchError:
        return False


def _encode(payload: dict[str, Any], ttl: timedelta) -> str:
    now = datetime.now(timezone.utc)
    to_encode = {**payload, "iat": now, "exp": now + ttl}
    return jwt.encode(to_encode, settings.jwt_secret, algorithm=JWT_ALGORITHM)


def _decode(token: str, expected_type: str) -> dict[str, Any]:
    try:
        payload = jwt.decode(token, settings.jwt_secret, algorithms=[JWT_ALGORITHM])
    except jwt.PyJWTError as exc:
        raise ValueError("Token non valido o scaduto.") from exc
    if payload.get("type") != expected_type:
        raise ValueError("Token di tipo inatteso.")
    return payload


def create_access_token(user_id: uuid.UUID) -> str:
    return _encode(
        {"sub": str(user_id), "type": "access"},
        timedelta(minutes=settings.jwt_access_token_ttl_minutes),
    )


def decode_access_token(token: str) -> uuid.UUID:
    payload = _decode(token, "access")
    return uuid.UUID(payload["sub"])


def create_mfa_challenge_token(
    user_id: uuid.UUID,
    method: Literal["totp", "email"],
    *,
    pending_sso_link: dict[str, str] | None = None,
) -> str:
    """pending_sso_link: se il challenge nasce da un login SSO in attesa di
    account linking (CLAUDE.md #3), porta {provider, provider_user_id, email}
    così che, dopo la verifica MFA, il collegamento possa essere completato."""
    payload: dict[str, Any] = {"sub": str(user_id), "type": "mfa_challenge", "method": method}
    if pending_sso_link is not None:
        payload["pending_sso_link"] = pending_sso_link
    return _encode(payload, timedelta(minutes=settings.jwt_mfa_challenge_ttl_minutes))


def decode_mfa_challenge_token(token: str) -> dict[str, Any]:
    return _decode(token, "mfa_challenge")
