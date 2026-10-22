"""post_media.is_sensitive: separa il flag "segnalata" dalle categorie scelte

`post_media.categories` è vuoto sia per un'immagine non segnalata sia per
una segnalata dalla sola automoderazione senza una categoria specifica
(app/domain/content_media.py) — `bool(categories)` non basta a distinguere
i due casi, e la bibliografia media pubblica del blog mostrava la seconda
sfocata solo nel rendering del post, non nella griglia media (bug corretto,
vedi app/api/v1/blogs/bibliography.py). Backfill: `true` dove `categories`
non è vuoto (approssimazione ragionevole, non peggiore del comportamento
precedente) — il prossimo salvataggio di ciascun post ricalcola comunque la
riga per intero da `_sync_post_media`, correggendo anche i casi
automod-senza-categoria non recuperabili qui.

Revision ID: b2c3d4e5f6a7
Revises: a1c2b3d4e5f6
Create Date: 2026-09-18 00:00:00.000000

"""
from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op

# revision identifiers, used by Alembic.
revision: str = "b2c3d4e5f6a7"
down_revision: Union[str, None] = "a1c2b3d4e5f6"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.add_column("post_media", sa.Column("is_sensitive", sa.Boolean(), nullable=True))
    op.execute("UPDATE post_media SET is_sensitive = (array_length(categories, 1) > 0)")
    op.execute("UPDATE post_media SET is_sensitive = false WHERE is_sensitive IS NULL")
    op.alter_column("post_media", "is_sensitive", nullable=False)


def downgrade() -> None:
    op.drop_column("post_media", "is_sensitive")
