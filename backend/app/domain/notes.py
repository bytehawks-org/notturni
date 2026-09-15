"""Note a piè di pagina dei post (todo/EDITOR.md).

Una nota è testo Markdown inline (nessun rendering lato backend) con un
numero `idx` scelto dall'editor. Nel corpo del post il riferimento alla nota
è il marcatore `[idx](#nota-idx)` (un link, così sopravvive al round-trip del
serializzatore Markdown dell'editor) oppure la forma testuale `[^idx]` per
chi scrive via API diretta. La resa come elenco a piè di pagina + tooltip è
del frontend (`src/lib/markdown.ts`), che si basa su questo elenco
strutturato, non sul parsing del corpo.
"""

from typing import NamedTuple

from app.models.blog_note import NOTE_KINDS

MAX_NOTES_PER_POST = 100
MAX_NOTE_LENGTH = 2000
MAX_NOTE_IDX = 999

# Limiti dei campi facoltativi per bibliografie strutturate (modal "Nota"
# nell'editor, dietro il toggle "Aggiungi dettagli bibliografici") — nessuno
# di questi è mai obbligatorio, solo `content` lo è.
MAX_NOTE_TITLE_LENGTH = 300
MAX_NOTE_AUTHOR_LENGTH = 300
MAX_NOTE_ISBN_LENGTH = 32
MAX_NOTE_DOI_LENGTH = 255
MAX_NOTE_PAGE_LENGTH = 32
# Compatibilità BibTeX (app/domain/blog_notes.py): editore/rivista/sito,
# anno/data, URL — `kind` non ha un limite di lunghezza, è validato contro
# NOTE_KINDS sotto.
MAX_NOTE_SOURCE_LENGTH = 300
MAX_NOTE_ISSUED_LENGTH = 32
MAX_NOTE_URL_LENGTH = 2000


class NoteInput(NamedTuple):
    idx: int
    content: str
    title: str | None = None
    author: str | None = None
    isbn: str | None = None
    doi: str | None = None
    page: str | None = None
    kind: str | None = None
    source: str | None = None
    issued: str | None = None
    url: str | None = None


def _clean_optional(value: str | None, *, max_length: int, label: str) -> str | None:
    if value is None:
        return None
    text = " ".join(value.split())
    if not text:
        return None
    if len(text) > max_length:
        raise ValueError(f"Il campo '{label}' della nota può avere al massimo {max_length} caratteri.")
    return text


def normalize_notes(notes: list[NoteInput]) -> list[NoteInput]:
    """Valida e normalizza l'elenco di note. Solleva ValueError se non valido.
    Ritorna le note ordinate per `idx`, senza duplicati di `idx`, con il
    contenuto (e gli eventuali campi bibliografici opzionali) ripuliti degli
    spazi ai bordi — una stringa vuota/solo spazi in un campo opzionale
    equivale ad assente (`None`)."""
    if len(notes) > MAX_NOTES_PER_POST:
        raise ValueError(f"Massimo {MAX_NOTES_PER_POST} note per post.")

    seen: set[int] = set()
    cleaned: list[NoteInput] = []
    for note in notes:
        if not (1 <= note.idx <= MAX_NOTE_IDX):
            raise ValueError(f"Il numero della nota deve essere tra 1 e {MAX_NOTE_IDX}.")
        if note.idx in seen:
            raise ValueError(f"Numero di nota duplicato: {note.idx}.")
        # nota = testo inline breve: gli spazi multipli/interruzioni di riga
        # non hanno significato e complicherebbero la deduplica in bibliografia.
        text = " ".join(note.content.split())
        if not text:
            raise ValueError("Il testo della nota non può essere vuoto.")
        if len(text) > MAX_NOTE_LENGTH:
            raise ValueError(f"Una nota può avere al massimo {MAX_NOTE_LENGTH} caratteri.")
        kind = _clean_optional(note.kind, max_length=10, label="tipo")
        if kind is not None and kind not in NOTE_KINDS:
            raise ValueError(f"Tipo di nota non valido: usare uno tra {', '.join(NOTE_KINDS)}.")
        seen.add(note.idx)
        cleaned.append(
            NoteInput(
                idx=note.idx,
                content=text,
                title=_clean_optional(note.title, max_length=MAX_NOTE_TITLE_LENGTH, label="titolo"),
                author=_clean_optional(note.author, max_length=MAX_NOTE_AUTHOR_LENGTH, label="autore"),
                isbn=_clean_optional(note.isbn, max_length=MAX_NOTE_ISBN_LENGTH, label="ISBN"),
                doi=_clean_optional(note.doi, max_length=MAX_NOTE_DOI_LENGTH, label="DOI"),
                page=_clean_optional(note.page, max_length=MAX_NOTE_PAGE_LENGTH, label="pagina"),
                kind=kind,
                source=_clean_optional(note.source, max_length=MAX_NOTE_SOURCE_LENGTH, label="editore/rivista/sito"),
                issued=_clean_optional(note.issued, max_length=MAX_NOTE_ISSUED_LENGTH, label="anno/data"),
                url=_clean_optional(note.url, max_length=MAX_NOTE_URL_LENGTH, label="URL"),
            )
        )

    cleaned.sort(key=lambda n: n.idx)
    return cleaned
