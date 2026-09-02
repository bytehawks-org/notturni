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

MAX_NOTES_PER_POST = 100
MAX_NOTE_LENGTH = 2000
MAX_NOTE_IDX = 999


class NoteInput(NamedTuple):
    idx: int
    content: str


def normalize_notes(notes: list[NoteInput]) -> list[NoteInput]:
    """Valida e normalizza l'elenco di note. Solleva ValueError se non valido.
    Ritorna le note ordinate per `idx`, senza duplicati di `idx`, con il
    contenuto ripulito degli spazi ai bordi."""
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
        seen.add(note.idx)
        cleaned.append(NoteInput(idx=note.idx, content=text))

    cleaned.sort(key=lambda n: n.idx)
    return cleaned
