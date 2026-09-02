"""Validazione dei campi anagrafici/linguistici del profilo utente
(CLAUDE.md #1): nome, cognome, paese, lingua madre e lingue di fallback —
queste ultime pensate anche come le lingue verso cui l'utente potrà
eventualmente tradurre i propri contenuti (vedi ProfileOut in
app/api/v1/users.py)."""

import re

from app.domain.i18n import validate_locale

MAX_FALLBACK_LANGUAGES = 5

_COUNTRY_PATTERN = re.compile(r"^[A-Z]{2}$")


def validate_country_code(country: str) -> None:
    """Solo il formato (ISO 3166-1 alpha-2, es. IT, FR, DE): non c'è ancora
    un elenco ufficiale dei paesi validi nel dominio applicativo, per non
    dover mantenere una lista lunga e soggetta a cambiamenti geopolitici."""
    if not _COUNTRY_PATTERN.fullmatch(country):
        raise ValueError("Il codice paese deve essere ISO 3166-1 di 2 lettere maiuscole (es. IT, FR, DE).")


def validate_fallback_languages(languages: list[str]) -> None:
    if len(languages) > MAX_FALLBACK_LANGUAGES:
        raise ValueError(f"Massimo {MAX_FALLBACK_LANGUAGES} lingue di fallback.")
    for locale in languages:
        validate_locale(locale)
