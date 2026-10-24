"""blogs/platform_config: nome mittente e banner delle campagne newsletter

Blocco "configurazione newsletter" (banner immagine, nome mittente):
newsletter_sender_name/newsletter_banner_url/newsletter_banner_alt_text su
`blogs` (per singolo blog) e sulle stesse colonne di `platform_config` (per
il digest di piattaforma, blog_id=None).

Revision ID: d6e7f8a9b021
Revises: c5d6e7f8a910
Create Date: 2026-09-19 00:10:00.000000

"""
from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op

# revision identifiers, used by Alembic.
revision: str = "d6e7f8a9b021"
down_revision: Union[str, None] = "c5d6e7f8a910"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    for table in ("blogs", "platform_config"):
        op.add_column(table, sa.Column("newsletter_sender_name", sa.String(120), nullable=True))
        op.add_column(table, sa.Column("newsletter_banner_url", sa.String(2048), nullable=True))
        op.add_column(
            table,
            sa.Column("newsletter_banner_alt_text", sa.String(300), nullable=False, server_default=""),
        )
        op.alter_column(table, "newsletter_banner_alt_text", server_default=None)


def downgrade() -> None:
    for table in ("blogs", "platform_config"):
        op.drop_column(table, "newsletter_banner_alt_text")
        op.drop_column(table, "newsletter_banner_url")
        op.drop_column(table, "newsletter_sender_name")
