"""letture dei post aggregate per giorno (todo/UX_REDESIGN.md B2)

Revision ID: a7c1d2e3f4b5
Revises: 139cf285ee44
Create Date: 2026-09-13 18:00:00

"""
from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


revision: str = 'a7c1d2e3f4b5'
down_revision: Union[str, None] = '139cf285ee44'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.create_table(
        'post_reads_daily',
        sa.Column('post_id', sa.UUID(), nullable=False),
        sa.Column('day', sa.Date(), nullable=False),
        sa.Column('reads', sa.Integer(), nullable=False),
        sa.ForeignKeyConstraint(['post_id'], ['posts.id'], ondelete='CASCADE'),
        sa.PrimaryKeyConstraint('post_id', 'day'),
    )


def downgrade() -> None:
    op.drop_table('post_reads_daily')
