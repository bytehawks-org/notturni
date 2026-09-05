"""blog is_suspended

Revision ID: 1c9f6a2d3b4e
Revises: 98da258b5f92
Create Date: 2026-09-04 00:00:00.000000

"""
from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


# revision identifiers, used by Alembic.
revision: str = '1c9f6a2d3b4e'
down_revision: Union[str, None] = '98da258b5f92'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    # server_default: blogs ha già righe, non c'è modo di aggiungere una
    # colonna NOT NULL senza un default su una tabella non vuota.
    op.add_column(
        'blogs', sa.Column('is_suspended', sa.Boolean(), nullable=False, server_default='false')
    )


def downgrade() -> None:
    op.drop_column('blogs', 'is_suspended')
