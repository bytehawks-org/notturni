"""Media e link citati nel corpo di un post (CLAUDE.md #4): estratti dal
Markdown allo stesso modo degli #hashtag (app/domain/tags.py) — il contenuto
resta l'unica fonte di verità, le tabelle `post_media`/`post_links`
(app/models/post_media.py, app/models/post_link.py) sono solo una cache
sincronizzata ad ogni salvataggio (vedi app/api/v1/posts.py::_sync_post_media/
_sync_post_links), usata per le query di bibliografia aggregate tra post
(`GET /blogs/{slug}/media-bibliography` e `.../links-bibliography`).

Le categorie di avviso sui contenuti (stile Bluesky, CLAUDE.md #3) viaggiano
nello stesso `title` Markdown già usato per "sensitive" (vedi
frontend/src/lib/markdown.ts): `![alt](url "sensitive")` per la sola
segnalazione automatica (categoria non nota) o
`![alt](url "sensitive:nudita,esplicito")` quando l'autore le sceglie
esplicitamente dal modal."""

import re
from typing import NamedTuple

# Stesso vocabolario del modal "Aggiungi un avviso sul contenuto" (stile
# Bluesky): 3 categorie "per adulti" + una generica.
SENSITIVITY_CATEGORIES = ("suggestive", "nudity", "explicit", "other")

# ![alt](url "title") — alt/title non possono contenere `]`/`"` in questa
# forma semplificata (stesso compromesso di tags.py:_HASHTAG_RE: copre l'uso
# reale dell'editor, non l'intera grammatica CommonMark).
_IMAGE_RE = re.compile(r'!\[([^\]]*)\]\(\s*(\S+?)(?:\s+"([^"]*)")?\s*\)')
# [testo](url "title") — il lookbehind su "!" esclude le immagini sopra.
_LINK_RE = re.compile(r'(?<!!)\[([^\]]*)\]\(\s*(\S+?)(?:\s+"([^"]*)")?\s*\)')


class MediaRef(NamedTuple):
    position: int
    url: str
    alt_text: str
    categories: tuple[str, ...]


class LinkRef(NamedTuple):
    position: int
    url: str
    link_text: str


def parse_sensitivity_categories(title: str | None) -> tuple[str, ...]:
    """`title` è `None`/assente (non segnalata), `"sensitive"` (automod, o
    modal senza una categoria specifica selezionata) o
    `"sensitive:cat1,cat2"` (categorie scelte dall'autore)."""
    if not title or not title.startswith("sensitive"):
        return ()
    _, _, raw = title.partition(":")
    if not raw:
        return ()
    return tuple(c for c in (part.strip() for part in raw.split(",")) if c in SENSITIVITY_CATEGORIES)


def is_flagged_sensitive(title: str | None) -> bool:
    return bool(title) and title.startswith("sensitive")


def build_sensitive_title(categories: list[str]) -> str:
    """Costruisce il valore da usare come `title` Markdown dell'immagine a
    partire dalle categorie scelte nel modal lato editor."""
    valid = [c for c in categories if c in SENSITIVITY_CATEGORIES]
    return "sensitive:" + ",".join(valid) if valid else "sensitive"


def extract_media(content: str) -> list[MediaRef]:
    return [
        MediaRef(
            position=i,
            url=match.group(2),
            alt_text=match.group(1),
            categories=parse_sensitivity_categories(match.group(3)),
        )
        for i, match in enumerate(_IMAGE_RE.finditer(content))
    ]


def extract_links(content: str) -> list[LinkRef]:
    """Esclude i marcatori interni delle note a piè di pagina (`#nota-n`,
    vedi app/domain/notes.py) — non sono link "citati" nel senso della
    bibliografia."""
    refs = []
    for i, match in enumerate(_LINK_RE.finditer(content)):
        url = match.group(2)
        if url.startswith("#"):
            continue
        refs.append(LinkRef(position=i, url=url, link_text=match.group(1)))
    return refs
