"""Dominio custom dell'utente verificato via record TXT sul DNS (stile
Bluesky): l'utente dimostra il possesso del dominio pubblicando un record
`_notturni-challenge.<dominio>` con un token generato lato server. Verificato
con successo assegna il sigillo di verifica "bronzo" (User.verification_tier,
CLAUDE.md #5) — priorità e assegnazione di silver/gold/blue (elenchi manuali/
dominio email, gestiti da un Super Admin) sono in app/domain/verification.py.

Nessuna dipendenza da HTTP/SSRF (solo lookup DNS): il rischio è al più un
abuso della frequenza di query, mitigato con lo stesso rate limiting fail-open
già usato altrove (app/domain/rate_limit.py)."""

import re
import secrets

import dns.asyncresolver
import dns.exception

from app.core.config import settings

DOMAIN_CHALLENGE_PREFIX = "_notturni-challenge"
DNS_LOOKUP_TIMEOUT_SECONDS = 5

_HOSTNAME_RE = re.compile(
    r"^(?=.{4,255}$)([a-z0-9]([a-z0-9-]*[a-z0-9])?\.)+[a-z]{2,}$"
)


def validate_domain(domain: str) -> str:
    """Normalizza e valida il formato del dominio. Solleva `ValueError` con
    un messaggio pronto per l'utente."""
    normalized = domain.strip().lower().rstrip(".")
    if not _HOSTNAME_RE.match(normalized):
        raise ValueError("Formato dominio non valido.")

    instance_domain = settings.instance_fqdn.lower()
    if normalized == instance_domain or normalized.endswith(f".{instance_domain}"):
        raise ValueError("Non puoi usare un (sotto)dominio della piattaforma stessa.")

    return normalized


def generate_verification_token() -> str:
    return secrets.token_hex(16)


def txt_record_name(domain: str) -> str:
    return f"{DOMAIN_CHALLENGE_PREFIX}.{domain}"


def txt_record_value(token: str) -> str:
    return f"notturni-verify={token}"


async def verify_domain_dns(domain: str, expected_token: str) -> bool:
    """Interroga il TXT record atteso. Fallimento/timeout/DNS irraggiungibile
    => False, mai un'eccezione che fa fallire la richiesta (fail open sul
    solo esito della singola verifica, non sulla disponibilità della
    feature — stesso principio della moderazione immagini)."""
    resolver = dns.asyncresolver.Resolver()
    resolver.timeout = DNS_LOOKUP_TIMEOUT_SECONDS
    resolver.lifetime = DNS_LOOKUP_TIMEOUT_SECONDS
    expected_value = txt_record_value(expected_token)
    try:
        answer = await resolver.resolve(txt_record_name(domain), "TXT")
    except dns.exception.DNSException:
        return False
    except Exception:
        return False

    for record in answer:
        value = b"".join(record.strings).decode("utf-8", errors="ignore")
        if value == expected_value:
            return True
    return False
