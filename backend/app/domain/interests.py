"""Interessi di piattaforma (blocco "interessi utente"): tag fissi
multilingua — chiave canonica non linguistica (es. "music"), mai scelta
liberamente dall'utente, con un'etichetta per lingua. L'elenco stesso è
configurabile: seminato da `NOCT_DEFAULT_INTERESTS` (JSON) all'avvio, poi
`platform_config.interests` resta l'unica sorgente di verità, modificabile
da un Super Admin in qualsiasi momento (stesso principio di
`reserved_blog_names`, vedi app/domain/platform_config.py)."""

import json
import logging
import re

logger = logging.getLogger(__name__)

MAX_USER_INTERESTS = 5
MAX_PLATFORM_INTERESTS = 200
INTEREST_KEY_PATTERN = re.compile(r"^[a-z0-9_-]{1,40}$")

# Curato a mano, traduzioni nelle stesse lingue già usate come filtro rapido
# di feed/directory lato frontend (FEED_LOCALES/DIRECTORY_LOCALES: it/en/de/fr).
DEFAULT_INTERESTS: list[dict] = [
    {"key": "music", "translations": {"it": "Musica", "en": "Music", "de": "Musik", "fr": "Musique"}},
    {"key": "technology", "translations": {"it": "Tecnologia", "en": "Technology", "de": "Technologie", "fr": "Technologie"}},
    {"key": "photography", "translations": {"it": "Fotografia", "en": "Photography", "de": "Fotografie", "fr": "Photographie"}},
    {"key": "literature", "translations": {"it": "Letteratura", "en": "Literature", "de": "Literatur", "fr": "Littérature"}},
    {"key": "cinema", "translations": {"it": "Cinema", "en": "Cinema", "de": "Kino", "fr": "Cinéma"}},
    {"key": "travel", "translations": {"it": "Viaggi", "en": "Travel", "de": "Reisen", "fr": "Voyages"}},
    {"key": "food", "translations": {"it": "Cucina", "en": "Food", "de": "Kochen", "fr": "Cuisine"}},
    {"key": "art", "translations": {"it": "Arte", "en": "Art", "de": "Kunst", "fr": "Art"}},
    {"key": "nature", "translations": {"it": "Natura", "en": "Nature", "de": "Natur", "fr": "Nature"}},
    {"key": "science", "translations": {"it": "Scienza", "en": "Science", "de": "Wissenschaft", "fr": "Science"}},
    {"key": "sports", "translations": {"it": "Sport", "en": "Sports", "de": "Sport", "fr": "Sport"}},
    {"key": "gaming", "translations": {"it": "Videogiochi", "en": "Gaming", "de": "Gaming", "fr": "Jeux vidéo"}},
    {"key": "fashion", "translations": {"it": "Moda", "en": "Fashion", "de": "Mode", "fr": "Mode"}},
    {"key": "politics", "translations": {"it": "Politica", "en": "Politics", "de": "Politik", "fr": "Politique"}},
    {"key": "philosophy", "translations": {"it": "Filosofia", "en": "Philosophy", "de": "Philosophie", "fr": "Philosophie"}},
    {"key": "history", "translations": {"it": "Storia", "en": "History", "de": "Geschichte", "fr": "Histoire"}},
]


def validate_interest_list(items: list[dict]) -> list[dict]:
    """Valida/normalizza un elenco di interessi (formato ENV/admin): chiave
    canonica univoca in formato slug, almeno una traduzione non vuota per
    voce. Solleva `ValueError` con un messaggio pronto per l'utente finale."""
    if len(items) > MAX_PLATFORM_INTERESTS:
        raise ValueError(f"Al massimo {MAX_PLATFORM_INTERESTS} interessi.")
    seen: set[str] = set()
    cleaned: list[dict] = []
    for item in items:
        key = str(item.get("key", "")).strip().lower()
        if not INTEREST_KEY_PATTERN.fullmatch(key):
            raise ValueError(f"Chiave interesse non valida: '{key}'.")
        if key in seen:
            raise ValueError(f"Chiave interesse duplicata: '{key}'.")
        translations = {
            str(loc).strip().lower(): str(label).strip()
            for loc, label in (item.get("translations") or {}).items()
            if str(label).strip()
        }
        if not translations:
            raise ValueError(f"L'interesse '{key}' non ha nessuna traduzione.")
        seen.add(key)
        cleaned.append({"key": key, "translations": translations})
    return cleaned


def default_interests_from_env(raw: str | None) -> list[dict] | None:
    """Legge `NOCT_DEFAULT_INTERESTS` (JSON, stesso schema di
    `platform_config.interests`) come seed alternativo ai `DEFAULT_INTERESTS`
    builtin. `None` se non impostata o non valida — un valore malformato non
    deve impedire l'avvio, solo far ricadere sui default con un avviso nei log."""
    if not raw:
        return None
    try:
        parsed = json.loads(raw)
        if not isinstance(parsed, list):
            raise ValueError("deve essere un elenco JSON")
        return validate_interest_list(parsed)
    except (ValueError, TypeError, json.JSONDecodeError) as exc:
        logger.warning("NOCT_DEFAULT_INTERESTS non valida, uso i default builtin: %s", exc)
        return None


def validate_user_interest_keys(keys: list[str], available: set[str]) -> list[str]:
    """Valida la scelta dell'utente (`PATCH /users/me`): al più
    `MAX_USER_INTERESTS`, tutte chiavi esistenti nell'elenco di piattaforma
    corrente. Deduplica preservando l'ordine di invio."""
    deduped = list(dict.fromkeys(k.strip().lower() for k in keys if k.strip()))
    if len(deduped) > MAX_USER_INTERESTS:
        raise ValueError(f"Al massimo {MAX_USER_INTERESTS} interessi.")
    unknown = [k for k in deduped if k not in available]
    if unknown:
        raise ValueError(f"Interessi sconosciuti: {', '.join(unknown)}.")
    return deduped
