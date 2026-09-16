from sqlalchemy import Column, ForeignKey, Integer, String, Table, Text
from sqlalchemy.dialects.postgresql import UUID

from app.models.base import Base

# Note a piè di pagina di un post (todo/EDITOR.md). Modello a tabella (non
# classe ORM) di proposito: come `post_tags`, viene riscritta per intero ad
# ogni salvataggio del post e letta con query esplicite (bibliografia del
# blog) — nessuna relazione ORM su `Post`, così un salvataggio non rischia un
# flush/lazy-load sincrono fuori dal contesto async (vedi
# app/api/v1/posts.py:_sync_post_notes, stessa insidia dei tag).
#
# `idx` è il numero della nota nel post (1-based), scelto dall'editor;
# `content` è Markdown inline (nessun rendering lato backend). PK composta
# (post_id, idx): una sola nota per numero, per post.
post_notes = Table(
    "post_notes",
    Base.metadata,
    Column("post_id", UUID(as_uuid=True), ForeignKey("posts.id", ondelete="CASCADE"), primary_key=True),
    Column("idx", Integer, primary_key=True),
    Column("content", Text, nullable=False),
    # B8: nota del blog a cui questa nota di post è agganciata (stesso testo
    # normalizzato), impostata alla sincronizzazione; null per righe legacy
    Column("note_id", UUID(as_uuid=True), ForeignKey("blog_notes.id", ondelete="SET NULL"), nullable=True, index=True),
    # Campi facoltativi per bibliografie strutturate (modal "Nota" nell'editor,
    # dietro il toggle "Aggiungi dettagli bibliografici") — tutti nullable,
    # nessuno di questi è mai obbligatorio: solo `content` lo è.
    Column("title", Text, nullable=True),
    Column("author", Text, nullable=True),
    Column("isbn", String(32), nullable=True),
    Column("doi", String(255), nullable=True),
    Column("page", String(32), nullable=True),
    # Compatibilità BibTeX (vedi app/domain/blog_notes.py): `kind` è
    # nullable qui (a differenza di blog_notes.kind, NOT NULL) — a livello
    # di singolo post resta solo un suggerimento dell'autore, non un dato
    # sempre presente; se assente `link_blog_notes` ricade su `guess_kind()`.
    Column("kind", String(10), nullable=True),
    Column("source", Text, nullable=True),
    Column("issued", String(32), nullable=True),
    Column("url", Text, nullable=True),
)
