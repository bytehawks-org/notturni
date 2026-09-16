"""campi bibliografici opzionali sulle note (titolo/autore/isbn/doi/pagina)

Modal "Nota" nell'editor con toggle "Aggiungi dettagli bibliografici":
solo il testo libero (`content`) resta obbligatorio, gli altri 5 campi sono
sempre facoltativi. Aggiunti sia a `post_notes` (la nota del singolo post)
sia a `blog_notes` (la libreria del blog, B8, a cui ogni nota si aggancia
automaticamente per testo — altrimenti il dato sparirebbe silenziosamente
alla sincronizzazione).

Revision ID: b3d5e6f7a8c9
Revises: f1b2c3d4e5a6
Create Date: 2026-09-16 00:00:00.000000

"""
from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op

# revision identifiers, used by Alembic.
revision: str = "b3d5e6f7a8c9"
down_revision: Union[str, None] = "f1b2c3d4e5a6"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    for table in ("post_notes", "blog_notes"):
        op.add_column(table, sa.Column("title", sa.Text(), nullable=True))
        op.add_column(table, sa.Column("author", sa.Text(), nullable=True))
        op.add_column(table, sa.Column("isbn", sa.String(32), nullable=True))
        op.add_column(table, sa.Column("doi", sa.String(255), nullable=True))
        op.add_column(table, sa.Column("page", sa.String(32), nullable=True))


def downgrade() -> None:
    for table in ("post_notes", "blog_notes"):
        op.drop_column(table, "page")
        op.drop_column(table, "doi")
        op.drop_column(table, "isbn")
        op.drop_column(table, "author")
        op.drop_column(table, "title")
