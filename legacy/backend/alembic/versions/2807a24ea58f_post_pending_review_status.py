"""post pending review status

Revision ID: 2807a24ea58f
Revises: a7bbd274e2af
Create Date: 2026-08-30 13:06:58.168134

"""
from typing import Sequence, Union

from alembic import op

# revision identifiers, used by Alembic.
revision: str = '2807a24ea58f'
down_revision: Union[str, None] = 'a7bbd274e2af'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    # Alembic autogenerate non rileva l'aggiunta di valori a un enum nativo
    # Postgres esistente: va scritta a mano. ADD VALUE è supportato dentro
    # una transazione da Postgres 12+.
    op.execute("ALTER TYPE post_status ADD VALUE IF NOT EXISTS 'pending_review'")


def downgrade() -> None:
    # Postgres non supporta la rimozione di un valore da un enum nativo
    # (richiederebbe ricreare il tipo e la colonna): non implementato.
    pass
