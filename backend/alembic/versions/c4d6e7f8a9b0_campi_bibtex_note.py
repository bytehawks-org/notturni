"""campi compatibili BibTeX sulle note (tipo/editore-rivista/anno-data/url)

Estende i campi bibliografici opzionali già esistenti (titolo/autore/isbn/
doi/pagina, b3d5e6f7a8c9) per un round-trip BibTeX più fedele:

- `kind` (entrytype: libro/articolo/web/nota) — su `blog_notes` esisteva già
  (colonna NOT NULL, default "note"); qui aggiunta anche a `post_notes`,
  ma nullable: a livello di singolo post resta solo un suggerimento
  dell'autore, usato in `link_blog_notes` se presente, altrimenti stimato
  da `guess_kind()` come già accadeva.
- `source` (publisher/journal/container-title): editore per un libro,
  rivista/sito per un articolo o una pagina web.
- `issued` (year/date): solo l'anno per un libro, una data anche estesa per
  una risorsa web — stringa libera, non un tipo data (nessun vincolo di
  formato: non tutte le fonti citabili hanno una data ISO completa).
- `url`: su `blog_notes` esisteva già; qui aggiunta anche a `post_notes`,
  così l'autore può indicarlo direttamente nel modal "Nota" invece di
  affidarsi solo all'estrazione euristica (`extract_url()`) dal testo.

Revision ID: c4d6e7f8a9b0
Revises: b3d5e6f7a8c9
Create Date: 2026-09-15 00:00:00.000000

"""
from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op

# revision identifiers, used by Alembic.
revision: str = "c4d6e7f8a9b0"
down_revision: Union[str, None] = "b3d5e6f7a8c9"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.add_column("post_notes", sa.Column("kind", sa.String(10), nullable=True))
    op.add_column("post_notes", sa.Column("url", sa.Text(), nullable=True))
    for table in ("post_notes", "blog_notes"):
        op.add_column(table, sa.Column("source", sa.Text(), nullable=True))
        op.add_column(table, sa.Column("issued", sa.String(32), nullable=True))


def downgrade() -> None:
    for table in ("post_notes", "blog_notes"):
        op.drop_column(table, "issued")
        op.drop_column(table, "source")
    op.drop_column("post_notes", "url")
    op.drop_column("post_notes", "kind")
