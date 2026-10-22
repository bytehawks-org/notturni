"""posts/blogs.cover_image_alt_text: alt text sulle immagini di copertina

Blocco "libreria media": né Post.cover_image_url né Blog.cover_image_url
avevano un campo alt text, a differenza delle immagini incorporate nel
contenuto (post_media.alt_text) e della libreria media del blog
(MediaFile.alt_text).

Revision ID: e7f8a9b0c132
Revises: d6e7f8a9b021
Create Date: 2026-09-19 00:15:00.000000

"""
from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op

# revision identifiers, used by Alembic.
revision: str = "e7f8a9b0c132"
down_revision: Union[str, None] = "d6e7f8a9b021"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    for table in ("posts", "blogs"):
        op.add_column(
            table,
            sa.Column("cover_image_alt_text", sa.String(300), nullable=False, server_default=""),
        )
        op.alter_column(table, "cover_image_alt_text", server_default=None)


def downgrade() -> None:
    for table in ("posts", "blogs"):
        op.drop_column(table, "cover_image_alt_text")
