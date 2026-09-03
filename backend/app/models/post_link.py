from sqlalchemy import Column, ForeignKey, Integer, Table, Text
from sqlalchemy.dialects.postgresql import UUID

from app.models.base import Base

# Link citati nel corpo di un post — CLAUDE.md #4. Stessa filosofia di
# post_media/post_tags/post_notes: tabella Core riscritta per intero ad ogni
# salvataggio (app/api/v1/posts.py::_sync_post_links) a partire dal contenuto
# Markdown, unica fonte di verità (app/domain/content_media.py::extract_links).
post_links = Table(
    "post_links",
    Base.metadata,
    Column("post_id", UUID(as_uuid=True), ForeignKey("posts.id", ondelete="CASCADE"), primary_key=True),
    Column("position", Integer, primary_key=True),
    Column("url", Text, nullable=False),
    Column("link_text", Text, nullable=False, default=""),
)
