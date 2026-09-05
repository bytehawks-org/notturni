"""audit archive runs

Revision ID: 5c3f9a71e0d2
Revises: 4b2e8a1c9d30
Create Date: 2026-09-05 00:00:01.000000

"""
from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


# revision identifiers, used by Alembic.
revision: str = '5c3f9a71e0d2'
down_revision: Union[str, None] = '4b2e8a1c9d30'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.create_table(
        'audit_archive_runs',
        sa.Column('id', sa.UUID(), nullable=False),
        sa.Column('period_start', sa.DateTime(timezone=True), nullable=False),
        sa.Column('period_end', sa.DateTime(timezone=True), nullable=False),
        sa.Column('week_label', sa.String(length=16), nullable=False),
        sa.Column('object_key', sa.String(length=512), nullable=True),
        sa.Column('storage_backend', sa.String(length=20), nullable=False),
        sa.Column('row_count', sa.Integer(), nullable=False),
        sa.Column('byte_size', sa.Integer(), server_default='0', nullable=False),
        sa.Column('sha256', sa.String(length=64), nullable=True),
        sa.Column('archived_at', sa.DateTime(timezone=True), server_default=sa.text('now()'), nullable=False),
        sa.PrimaryKeyConstraint('id'),
        sa.UniqueConstraint('period_start', name='uq_audit_archive_runs_period_start'),
    )


def downgrade() -> None:
    op.drop_table('audit_archive_runs')
