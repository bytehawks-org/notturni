"""Note del blog come entità (B8): normalizzazione per la deduplica, stima
del tipo, export/import BibTeX minimale."""

import re
import unicodedata
import uuid
from typing import Iterable

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


def guess_kind(content: str, url: str | None) -> str:
    """Stima grossolana, sempre correggibile dalla libreria: DOI → articolo,
    URL → web, "anno + titolo" (es. "Zerby, C. The Devil's Details. 2002.")
    → libro, altrimenti nota dell'autore."""
    if url and "doi.org" in url:
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


def to_bibtex(notes: Iterable[BlogNote]) -> str:
    """Una voce per nota: `@book`/`@article`/`@misc` con `note` = testo
    completo (le note non sono strutturate per campi: il testo resta la
    fonte, i campi opzionali `url`/`year` aiutano i gestori bibliografici)."""
    entries = []
    for i, n in enumerate(notes, 1):
        kind = {"book": "book", "article": "article"}.get(n.kind, "misc")
        year = _YEAR_RE.search(n.content)
        fields = [f"  note = {{{_bibtex_escape(n.content)}}}"]
        if n.url:
            fields.append(f"  url = {{{n.url}}}")
        if year:
            fields.append(f"  year = {{{year.group(0)}}}")
        entries.append(f"@{kind}{{notturni{i},\n" + ",\n".join(fields) + "\n}")
    return "\n\n".join(entries) + ("\n" if entries else "")


_ENTRY_RE = re.compile(r"@(\w+)\s*\{\s*([^,\s]*)\s*,(.*?)\n\}", re.S)
_FIELD_RE = re.compile(r"(\w+)\s*=\s*(\{(?:[^{}]|\{[^{}]*\})*\}|\"[^\"]*\")", re.S)


def parse_bibtex(text: str) -> list[dict]:
    """Parser minimale: per ogni voce ricava tipo (book/article/altro → web se
    c'è un URL, altrimenti note), testo (`note` se presente, altrimenti
    "Autore. Titolo. Editore, anno.") e URL/DOI."""
    out = []
    for m in _ENTRY_RE.finditer(text):
        entry_type = m.group(1).lower()
        fields = {k.lower(): v.strip("{}\"").strip() for k, v in _FIELD_RE.findall(m.group(3))}
        url = fields.get("url") or (f"https://doi.org/{fields['doi']}" if fields.get("doi") else None)
        if fields.get("note"):
            content = fields["note"]
        else:
            parts = [fields.get("author"), fields.get("title"), fields.get("journal") or fields.get("publisher"), fields.get("year")]
            content = ". ".join(p for p in parts if p)
            if content:
                content += "."
        if not content:
            continue
        kind = "book" if entry_type == "book" else "article" if entry_type in ("article", "inproceedings") else ("web" if url else "note")
        out.append({"content": content[:2000], "kind": kind if kind in NOTE_KINDS else "note", "url": url})
    return out
