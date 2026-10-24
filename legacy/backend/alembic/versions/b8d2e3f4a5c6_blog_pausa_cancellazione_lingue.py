"""blog: pausa, cancellazione con tolleranza, lingue secondarie (todo/UX_REDESIGN.md B3)

Revision ID: b8d2e3f4a5c6
Revises: a7c1d2e3f4b5
Create Date: 2026-09-13 19:00:00

"""
from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


revision: str = 'b8d2e3f4a5c6'
down_revision: Union[str, None] = 'a7c1d2e3f4b5'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.add_column('blogs', sa.Column('is_paused', sa.Boolean(), server_default=sa.false(), nullable=False))
    op.add_column('blogs', sa.Column('deleted_at', sa.DateTime(timezone=True), nullable=True))
    op.add_column(
        'blogs',
        sa.Column('extra_locales', sa.ARRAY(sa.String(length=2)), server_default='{}', nullable=False),
    )


def downgrade() -> None:
    op.drop_column('blogs', 'extra_locales')
    op.drop_column('blogs', 'deleted_at')
    op.drop_column('blogs', 'is_paused')
