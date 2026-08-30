from app.core.security import generate_opaque_secret, sha256_hex

TOKEN_PREFIX = "noct_"
PREFIX_DISPLAY_LENGTH = 16


def generate_api_token() -> tuple[str, str, str]:
    """Genera un token: (valore in chiaro, prefisso identificativo, hash sha256).

    Solo il valore in chiaro va restituito al chiamante, una sola volta, alla
    creazione: in DB si persiste esclusivamente l'hash."""
    plaintext = f"{TOKEN_PREFIX}{generate_opaque_secret()}"
    return plaintext, plaintext[:PREFIX_DISPLAY_LENGTH], hash_token(plaintext)


def hash_token(plaintext: str) -> str:
    return sha256_hex(plaintext)
