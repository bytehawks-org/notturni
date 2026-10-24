"""Logica di dominio della newsletter (app/models/newsletter.py): generazione
dei token e (per la disiscrizione) la loro verifica.

Due schemi di token distinti, per due esigenze diverse:

- **Conferma iscrizione**: token opaco ad alta entropia, hash sha256 in DB
  (`NewsletterSubscriber.confirm_token_hash`), stesso schema di api_tokens/MFA
  — un lookup per hash va benissimo. Una volta confermato l'iscritto, l'hash
  resta in DB invece di essere azzerato: serve a rendere l'endpoint di
  conferma idempotente (un secondo click sullo stesso link, comune con i
  client email che pre-caricano i link, risponde "already_confirmed" invece
  di "invalid" — vedi app/api/v1/newsletter.py:confirm_subscription).
- **Disiscrizione**: deve restare valido e riproducibile in *ogni* email
  inviata per sempre (il worker lo ricostruisce ad ogni invio di campagna,
  non solo alla conferma), quindi non può essere un token opaco di cui in DB
  resta solo l'hash — non c'è modo di ricostruirlo dall'hash. È invece
  firmato in modo stateless via HMAC-SHA256 sul solo subscriber id: chiunque
  conosca `NOCT_JWT_SECRET` (già usato per firmare gli access token, stessa
  superficie di rischio) può verificarlo senza alcun giro a DB, e non serve
  persistere nulla."""

import hashlib
import hmac
import uuid
from datetime import datetime, timedelta, timezone

from app.core.config import settings
from app.core.security import generate_opaque_secret, sha256_hex

CONFIRM_TOKEN_TTL_HOURS = 48

_UNSUBSCRIBE_TOKEN_SEPARATOR = "."


def generate_confirm_token() -> tuple[str, str]:
    """(valore in chiaro da mandare via email, hash sha256 da salvare)."""
    plaintext = generate_opaque_secret()
    return plaintext, sha256_hex(plaintext)


def confirm_token_expiry() -> datetime:
    return datetime.now(timezone.utc) + timedelta(hours=CONFIRM_TOKEN_TTL_HOURS)


def _unsubscribe_signature(subscriber_id: uuid.UUID) -> str:
    return hmac.new(
        settings.jwt_secret.encode("utf-8"), subscriber_id.bytes, hashlib.sha256
    ).hexdigest()


def sign_unsubscribe_token(subscriber_id: uuid.UUID) -> str:
    """Token stateless per il link "un click" di disiscrizione/cancellazione:
    `<subscriber_id>.<firma>`, nessuna scadenza (deve restare valido finché
    l'iscrizione esiste)."""
    return f"{subscriber_id}{_UNSUBSCRIBE_TOKEN_SEPARATOR}{_unsubscribe_signature(subscriber_id)}"


def verify_unsubscribe_token(token: str) -> uuid.UUID | None:
    """None se il token è malformato o la firma non corrisponde."""
    raw_id, _, signature = token.partition(_UNSUBSCRIBE_TOKEN_SEPARATOR)
    if not signature:
        return None
    try:
        subscriber_id = uuid.UUID(raw_id)
    except ValueError:
        return None
    expected = _unsubscribe_signature(subscriber_id)
    if not hmac.compare_digest(expected, signature):
        return None
    return subscriber_id
