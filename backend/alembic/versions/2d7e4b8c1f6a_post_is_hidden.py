"""post is_hidden

Revision ID: 2d7e4b8c1f6a
Revises: 1c9f6a2d3b4e
Create Date: 2026-09-04 00:00:00.000000

"""
from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


# revision identifiers, used by Alembic.
revision: str = '2d7e4b8c1f6a'
down_revision: Union[str, None] = '1c9f6a2d3b4e'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    # server_default: posts ha già righe, non c'è modo di aggiungere una
    # colonna NOT NULL senza un default su una tabella non vuota.
    op.add_column(
        'posts', sa.Column('is_hidden', sa.Boolean(), nullable=False, server_default='false')
    )


def downgrade() -> None:
    op.drop_column('posts', 'is_hidden')
