"""Sigilli di verifica del profilo (`User.verification_tier`, stile Bluesky/
Instagram/Twitter): calcolo unico condiviso da tutti i punti che possono
farli variare (registrazione, cambio email/username, verifica/rimozione
dominio custom, modifica delle liste di piattaforma da un Super Admin).

Priorità in caso di più criteri soddisfatti contemporaneamente, dalla più
alta: GOLD (sostenitori economici, elenco a mano) > SILVER (entità
verificate a mano dalla piattaforma) > BLUE (dominio email di piattaforma o
di un'organizzazione approvata) > BRONZE (dominio custom del blog
verificato via DNS, `app/domain/custom_domains.py`) > NONE. Un tier più alto
non è mai perso "per errore": se nessun criterio manuale/dominio è più
soddisfatto l'utente ricade sul tier immediatamente inferiore che ancora si
applica (es. rimosso dall'elenco GOLD ma dominio custom ancora verificato ->
BRONZE), mai lasciato bloccato sul tier più alto una volta assegnato."""

import re

from app.core.config import settings
from app.models.platform_config import PlatformConfig
from app.models.user import User, VerificationTier

DOMAIN_PATTERN = re.compile(r"^(?=.{4,255}$)([a-z0-9]([a-z0-9-]*[a-z0-9])?\.)+[a-z]{2,}$")


def _email_domain(email: str) -> str:
    return email.rsplit("@", 1)[-1].lower() if "@" in email else ""


def _matches_identifier(identifiers: list[str], *, email: str, username: str, domain: str) -> bool:
    """Un elenco misto (email intere, username, domini) matcha per
    uguaglianza case-insensitive su email/username, o se la voce è il
    dominio dell'email dell'utente."""
    normalized = {value.strip().lower() for value in identifiers if value.strip()}
    return email.lower() in normalized or username.lower() in normalized or (domain and domain in normalized)


def compute_verification_tier(user: User, config: PlatformConfig) -> VerificationTier:
    """Tier che dovrebbe avere l'utente ora, ricalcolato da zero (non
    incrementale): non legge `user.verification_tier` corrente."""
    domain = _email_domain(user.email)
    if _matches_identifier(config.verification_gold_identifiers, email=user.email, username=user.username, domain=domain):
        return VerificationTier.GOLD
    if _matches_identifier(config.verification_silver_identifiers, email=user.email, username=user.username, domain=domain):
        return VerificationTier.SILVER
    blue_domains = {settings.instance_fqdn.lower()} | {d.strip().lower() for d in config.verification_blue_domains if d.strip()}
    if domain and domain in blue_domains:
        return VerificationTier.BLUE
    if user.verified_domain:
        return VerificationTier.BRONZE
    return VerificationTier.NONE


def sync_verification_tier(user: User, config: PlatformConfig) -> bool:
    """Applica `compute_verification_tier` a `user.verification_tier`.
    Ritorna True se il tier è cambiato (utile per decidere se serve un
    audit/log a chiamata)."""
    computed = compute_verification_tier(user, config)
    if user.verification_tier == computed:
        return False
    user.verification_tier = computed
    return True
