"""Note del blog come entità (B8): normalizzazione per la deduplica, stima
del tipo, export/import BibTeX minimale."""

import re
import unicodedata
import uuid
from typing import Iterable
from urllib.parse import urlparse

from app.models.blog_note import NOTE_KINDS, BlogNote

_DOI_RE = re.compile(r"10\.\d{4,9}/[^\s\"<>]+", re.I)
_URL_RE = re.compile(r"https?://[^\s)\]>\"]+", re.I)
_YEAR_RE = re.compile(r"\b(1[5-9]\d{2}|20\d{2})\b")


def normalize_note(content: str) -> str:
    text = unicodedata.normalize("NFKD", content).encode("ascii", "ignore").decode()
    # apostrofi e virgolette tipografiche non separano parole ("Devil's" ≡ "Devils")
    text = re.sub(r"['\u2019`\"]", "", text)
    text = re.sub(r"[^a-z0-9]+", " ", text.lower()).strip()
    return text[:600]


def extract_url(content: str) -> str | None:
    doi = _DOI_RE.search(content)
    if doi:
        return f"https://doi.org/{doi.group(0).rstrip('.')}"
    url = _URL_RE.search(content)
    return url.group(0).rstrip(".,;") if url else None


def _is_doi_url(url: str) -> bool:
    """Confronto sull'host effettivo, non una substring: `evil.com/?x=doi.org`
    o `notdoi.org` non devono essere scambiati per un link DOI."""
    hostname = urlparse(url).hostname
    return hostname is not None and (hostname == "doi.org" or hostname.endswith(".doi.org"))


def guess_kind(content: str, url: str | None) -> str:
    """Stima grossolana, sempre correggibile dalla libreria: DOI → articolo,
    URL → web, "anno + titolo" (es. "Zerby, C. The Devil's Details. 2002.")
    → libro, altrimenti nota dell'autore."""
    if url and _is_doi_url(url):
        return "article"
    if url:
        return "web"
    if _YEAR_RE.search(content) and "," in content:
        return "book"
    return "note"


def duplicate_key(normalized: str) -> str:
    """Due note sono "possibili duplicati" se coincidono sui primi 30 caratteri
    normalizzati (es. stessa opera citata con formattazioni diverse)."""
    return normalized[:30]


def find_duplicate_groups(notes: Iterable[BlogNote]) -> dict[uuid.UUID, list[uuid.UUID]]:
    groups: dict[str, list[BlogNote]] = {}
    for n in notes:
        key = duplicate_key(n.normalized)
        if key:
            groups.setdefault(key, []).append(n)
    out: dict[uuid.UUID, list[uuid.UUID]] = {}
    for members in groups.values():
        if len(members) > 1:
            for n in members:
                out[n.id] = [m.id for m in members if m.id != n.id]
    return out


def _bibtex_escape(text: str) -> str:
    return text.replace("{", "\\{").replace("}", "\\}")


# Campo BibTeX in cui va `source` (editore/rivista/sito) a seconda del tipo:
# publisher per un libro, journal per un articolo, organization altrimenti
# (web/nota — BibTeX classico non ha un campo "sito", organization è il più
# vicino tra quelli riconosciuti dai gestori bibliografici più diffusi).
_SOURCE_FIELD = {"book": "publisher", "article": "journal"}


def to_bibtex(notes: Iterable[BlogNote]) -> str:
    """Una voce per nota: `@book`/`@article`/`@misc` con i campi strutturati
    (title/author/publisher-o-journal/year/isbn/doi/url) quando presenti, più
    `note` col testo completo — che resta sempre la fonte di verità mostrata
    nella bibliografia pubblica, i campi strutturati sono un aiuto in più per
    i gestori bibliografici esterni. Un anno mancante viene comunque stimato
    dal testo (compatibilità con le note create prima di questo campo)."""
    entries = []
    for i, n in enumerate(notes, 1):
        kind = {"book": "book", "article": "article"}.get(n.kind, "misc")
        year = n.issued or (_YEAR_RE.search(n.content).group(0) if _YEAR_RE.search(n.content) else None)
        fields = []
        if n.title:
            fields.append(f"  title = {{{_bibtex_escape(n.title)}}}")
        if n.author:
            fields.append(f"  author = {{{_bibtex_escape(n.author)}}}")
        if n.source:
            source_field = _SOURCE_FIELD.get(n.kind, "organization")
            fields.append(f"  {source_field} = {{{_bibtex_escape(n.source)}}}")
        if year:
            fields.append(f"  year = {{{year}}}")
        if n.isbn:
            fields.append(f"  isbn = {{{n.isbn}}}")
        if n.doi:
            fields.append(f"  doi = {{{n.doi}}}")
        if n.page:
            fields.append(f"  pages = {{{_bibtex_escape(n.page)}}}")
        if n.url:
            fields.append(f"  url = {{{n.url}}}")
        fields.append(f"  note = {{{_bibtex_escape(n.content)}}}")
        entries.append(f"@{kind}{{notturni{i},\n" + ",\n".join(fields) + "\n}")
    return "\n\n".join(entries) + ("\n" if entries else "")


# Gruppi atomici `(?>...)` intorno agli `\s*` adiacenti a `[^,\s]*` (che non
# può mai sovrapporsi a uno spazio): senza, per un input con molti spazi e
# nessuna `,` di chiusura il motore prova ogni modo di dividere quella
# sequenza di spazi fra i due `\s*` prima di fallire — O(n²) per voce non
# valida, sfruttabile come DoS su un endpoint autenticato senza limite di
# lunghezza sull'input (CodeQL py/polynomial-redos, confermato empiricamente:
# ~0.8s per 40.000 spazi prima della fix, sub-millisecondo dopo).
_ENTRY_RE = re.compile(r"@(\w+)(?>\s*)\{(?>\s*)([^,\s]*)(?>\s*),(.*?)\n\}", re.S)
_FIELD_RE = re.compile(r"(\w+)\s*=\s*(\{(?:[^{}]|\{[^{}]*\})*\}|\"[^\"]*\")", re.S)

# Limite di lunghezza sull'input grezzo (difesa in profondità, oltre alla fix
# della regex sopra): un import BibTeX legittimo non ha bisogno di megabyte di
# testo, e mette un tetto assoluto al costo di qualunque pattern, noto o no,
# che dovesse rivelarsi comunque costoso su input patologici.
MAX_BIBTEX_LENGTH = 200_000


def parse_bibtex(text: str) -> list[dict]:
    """Parser minimale: per ogni voce ricava i campi strutturati (title/
    author/source/issued/isbn/doi/url) più tipo e testo (`note` se presente,
    altrimenti "Autore. Titolo. Editore, anno.", sintetizzato dagli stessi
    campi strutturati). Solleva `ValueError` oltre `MAX_BIBTEX_LENGTH` — vedi
    il commento su `_ENTRY_RE` per il perché di un tetto esplicito."""
    if len(text) > MAX_BIBTEX_LENGTH:
        raise ValueError(f"Testo BibTeX troppo lungo (massimo {MAX_BIBTEX_LENGTH} caratteri).")
    out = []
    for m in _ENTRY_RE.finditer(text):
        entry_type = m.group(1).lower()
        fields = {k.lower(): v.strip("{}\"").strip() for k, v in _FIELD_RE.findall(m.group(3))}
        url = fields.get("url") or (f"https://doi.org/{fields['doi']}" if fields.get("doi") else None)
        source = fields.get("publisher") or fields.get("journal") or fields.get("organization")
        if fields.get("note"):
            content = fields["note"]
        else:
            parts = [fields.get("author"), fields.get("title"), source, fields.get("year")]
            content = ". ".join(p for p in parts if p)
            if content:
                content += "."
        if not content:
            continue
        kind = "book" if entry_type == "book" else "article" if entry_type in ("article", "inproceedings") else ("web" if url else "note")
        out.append(
            {
                "content": content[:2000],
                "kind": kind if kind in NOTE_KINDS else "note",
                "url": url,
                "title": fields.get("title"),
                "author": fields.get("author"),
                "source": source,
                "issued": fields.get("year") or fields.get("date"),
                "isbn": fields.get("isbn"),
                "doi": fields.get("doi"),
                "page": fields.get("pages") or fields.get("page"),
            }
        )
    return out
