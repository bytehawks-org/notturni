from sqlalchemy import ARRAY, Column, ForeignKey, Integer, String, Table, Text
from sqlalchemy.dialects.postgresql import UUID

from app.models.base import Base

# Media (oggi solo immagini) citati nel corpo di un post — CLAUDE.md #4.
# Modello a tabella (non classe ORM) di proposito, come `post_tags`/
# `post_notes`: riscritta per intero ad ogni salvataggio del post
# (app/api/v1/posts.py::_sync_post_media) a partire dal contenuto Markdown,
# unica fonte di verità (app/domain/content_media.py::extract_media) — mai
# una relazione ORM su `Post`, per non rischiare un flush/lazy-load sincrono
# fuori dal contesto async (stessa insidia di post_tags/post_notes).
#
# `position` è l'ordine di comparsa nel contenuto al momento del salvataggio
# (non un identificatore stabile nel tempo). `categories` è il sottoinsieme
# di app/domain/content_media.py::SENSITIVITY_CATEGORIES scelto dall'autore
# (vuoto se l'immagine non è segnalata, o segnalata solo dall'automoderazione
# senza una categoria specifica).
post_media = Table(
    "post_media",
    Base.metadata,
    Column("post_id", UUID(as_uuid=True), ForeignKey("posts.id", ondelete="CASCADE"), primary_key=True),
    Column("position", Integer, primary_key=True),
    Column("url", Text, nullable=False),
    Column("alt_text", Text, nullable=False, default=""),
    Column("categories", ARRAY(String(20)), nullable=False, default=list),
)
