"""Menzioni `@username` nei contenuti (todo/USERS.md #1, todo/EDITOR.md).

Il rendering a link è responsabilità del frontend (come per tutto il Markdown,
vedi backend/API.md). Qui teniamo solo la definizione canonica della sintassi
e un estrattore, usato per test e per eventuali usi futuri (es. notifiche,
"brain map" dei concetti — non ancora implementati)."""

import re

# `@` preceduto da inizio stringa o da un carattere non-parola (spazio,
# punteggiatura), seguito da uno username nel formato di app/domain/usernames.py.
# Lo stesso pattern è replicato lato frontend in src/lib/markdown.ts.
MENTION_RE = re.compile(r"(?<![\w@])@([a-z0-9]+(?:[-_][a-z0-9]+)*)")


def extract_mentions(text: str) -> list[str]:
    """Username citati in `text`, in ordine di prima comparsa, senza duplicati."""
    seen: dict[str, None] = {}
    for match in MENTION_RE.finditer(text or ""):
        seen.setdefault(match.group(1), None)
    return list(seen)
