"""platform_config: spazio massimo per blog + elenchi di verifica manuale

Blocco "opt-in directory utenti" + "impostazioni di piattaforma": lo
spazio massimo per blog (null = nessun limite) e i tre elenchi che
assegnano i sigilli di verifica GOLD/SILVER/BLU a mano dal Super Admin
(app/domain/verification.py) — GOLD e SILVER per email/username, BLU per
domini email fidati. `User.directory_listed` esiste già (migrazione
c9839d73bf05), qui non serve nessuna nuova colonna per l'opt-out utente,
solo l'esclusione hardcoded del Super Admin dalla query (app/api/v1/users.py).

Revision ID: a3f4b5c6d7e8
Revises: e7f8a9b0c132
Create Date: 2026-09-20 09:00:00.000000

"""
from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op
from sqlalchemy.dialects import postgresql

# revision identifiers, used by Alembic.
revision: str = "a3f4b5c6d7e8"
down_revision: Union[str, None] = "e7f8a9b0c132"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.add_column("platform_config", sa.Column("max_blog_storage_mb", sa.Integer(), nullable=True))
    for column in (
        "verification_gold_identifiers",
        "verification_silver_identifiers",
        "verification_blue_domains",
    ):
        op.add_column(
            "platform_config",
            sa.Column(column, postgresql.ARRAY(sa.String(255)), nullable=False, server_default="{}"),
        )
        op.alter_column("platform_config", column, server_default=None)


def downgrade() -> None:
    op.drop_column("platform_config", "verification_blue_domains")
    op.drop_column("platform_config", "verification_silver_identifiers")
    op.drop_column("platform_config", "verification_gold_identifiers")
    op.drop_column("platform_config", "max_blog_storage_mb")
