from sqlalchemy import Column, ForeignKey, Integer, Table, Text
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
)
