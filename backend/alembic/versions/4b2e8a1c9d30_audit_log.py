"""audit log

Revision ID: 4b2e8a1c9d30
Revises: 2d7e4b8c1f6a
Create Date: 2026-09-05 00:00:00.000000

"""
from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa
from sqlalchemy.dialects import postgresql


# revision identifiers, used by Alembic.
revision: str = '4b2e8a1c9d30'
down_revision: Union[str, None] = '2d7e4b8c1f6a'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.create_table(
        'audit_log',
        sa.Column('id', sa.UUID(), nullable=False),
        sa.Column('occurred_at', sa.DateTime(timezone=True), server_default=sa.text('now()'), nullable=False),
        sa.Column(
            'actor_type',
            sa.Enum(
                'user', 'core_token', 'user_token', 'system', 'anonymous',
                name='audit_actor_type',
            ),
            nullable=False,
        ),
        sa.Column('actor_id', sa.UUID(), nullable=True),
        sa.Column('actor_label', sa.String(length=255), nullable=True),
        sa.Column('action', sa.String(length=100), nullable=False),
        sa.Column('target_type', sa.String(length=50), nullable=True),
        sa.Column('target_id', sa.UUID(), nullable=True),
        sa.Column('blog_id', sa.UUID(), nullable=True),
        sa.Column('ip', postgresql.INET(), nullable=True),
        sa.Column('user_agent', sa.String(length=500), nullable=True),
        sa.Column('payload', postgresql.JSONB(astext_type=sa.Text()), server_default=sa.text("'{}'::jsonb"), nullable=False),
        sa.PrimaryKeyConstraint('id'),
    )
    # occurred_at: indice per la cancellazione periodica per data e per le
    # query admin per intervallo temporale
    op.create_index(op.f('ix_audit_log_occurred_at'), 'audit_log', ['occurred_at'])
    op.create_index('ix_audit_log_actor_id_occurred_at', 'audit_log', ['actor_id', 'occurred_at'])
    op.create_index('ix_audit_log_blog_id_occurred_at', 'audit_log', ['blog_id', 'occurred_at'])


def downgrade() -> None:
    op.drop_index('ix_audit_log_blog_id_occurred_at', table_name='audit_log')
    op.drop_index('ix_audit_log_actor_id_occurred_at', table_name='audit_log')
    op.drop_index(op.f('ix_audit_log_occurred_at'), table_name='audit_log')
    op.drop_table('audit_log')
    sa.Enum(name='audit_actor_type').drop(op.get_bind())
