"""Tag dei post: massimo 5 in tutto, che vengano dal campo dedicato o da
hashtag scritti nel testo (#tag) — i due canali confluiscono nello stesso
insieme, senza distinzione per l'utente finale."""

import re

MAX_TAGS_PER_POST = 5

# minuscolo, cifre, trattini; niente trattino iniziale/finale o doppio,
# imposto in normalize_tag più che nella regex per messaggi d'errore chiari
_VALID_TAG_RE = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*$")
_MAX_TAG_LENGTH = 30

# #hashtag nel testo: lettere/cifre/underscore/trattino, non spezzato da
# markdown (es. non cattura dentro un URL preceduto da altri caratteri
# alfanumerici — il confine di parola prima di # basta per i casi comuni)
_HASHTAG_RE = re.compile(r"(?<!\w)#([A-Za-z0-9][A-Za-z0-9_-]{0,29})")


def normalize_tag(raw: str) -> str:
    value = raw.strip().lstrip("#").lower().replace("_", "-")
    value = re.sub(r"\s+", "-", value)
    if not value or len(value) > _MAX_TAG_LENGTH or not _VALID_TAG_RE.fullmatch(value):
        raise ValueError(
            f"Tag non valido: {raw!r} (solo lettere minuscole, cifre e trattini singoli, "
            f"max {_MAX_TAG_LENGTH} caratteri)."
        )
    return value


def extract_hashtags(content: str) -> list[str]:
    """Hashtag scritti nel testo del post, normalizzati e deduplicati
    nell'ordine in cui compaiono. Un hashtag malformato per i nostri vincoli
    (es. troppo lungo) viene semplicemente ignorato: nel testo libero non è
    l'utente a scegliere esplicitamente un tag, non ha senso rifiutare
    l'intero salvataggio per un # incidentale."""
    seen: list[str] = []
    for match in _HASHTAG_RE.finditer(content):
        try:
            normalized = normalize_tag(match.group(1))
        except ValueError:
            continue
        if normalized not in seen:
            seen.append(normalized)
    return seen


def resolve_tags(manual_tags: list[str], content: str) -> tuple[list[str], list[str]]:
    """Unisce i tag del campo dedicato (validati rigidamente: un tag scritto
    a mano lì è una scelta esplicita, un formato sbagliato è un errore da
    segnalare) con gli hashtag nel testo, deduplica, e impone il tetto
    massimo — in eccesso, errore esplicito (non troncamento silenzioso).

    Ritorna (manual_normalizzati, effettivi_totali)."""
    normalized_manual: list[str] = []
    for raw in manual_tags:
        normalized = normalize_tag(raw)
        if normalized not in normalized_manual:
            normalized_manual.append(normalized)

    effective = list(normalized_manual)
    for tag in extract_hashtags(content):
        if tag not in effective:
            effective.append(tag)

    if len(effective) > MAX_TAGS_PER_POST:
        raise ValueError(
            f"Massimo {MAX_TAGS_PER_POST} tag per post (campo dedicato e hashtag nel testo "
            f"insieme), trovati {len(effective)}: {', '.join(effective)}."
        )
    return normalized_manual, effective
