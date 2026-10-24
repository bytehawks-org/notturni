"""impostazioni di piattaforma, coda richieste GDPR, lingua interfaccia utente (todo/UX_REDESIGN.md B6)

Revision ID: e1a5b6c7d8f9
Revises: d0f4a5b6c7e8
Create Date: 2026-09-13 22:00:00

"""
from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


revision: str = 'e1a5b6c7d8f9'
down_revision: Union[str, None] = 'd0f4a5b6c7e8'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.add_column('users', sa.Column('ui_locale', sa.String(length=2), nullable=True))
    op.create_table(
        'platform_config',
        sa.Column('id', sa.Integer(), nullable=False),
        sa.Column('default_locale', sa.String(length=2), nullable=False),
        sa.Column('registration_mode', sa.String(length=10), nullable=False),
        sa.Column('sso_providers', sa.ARRAY(sa.String(length=20)), nullable=False),
        sa.Column('mfa_required_for_admins', sa.Boolean(), nullable=False),
        sa.Column('reserved_blog_names', sa.ARRAY(sa.String(length=63)), nullable=False),
        sa.Column('moderation_threshold', sa.Float(), nullable=False),
        sa.Column('max_blogs_per_user', sa.Integer(), nullable=False),
        sa.Column('anonymous_comments_allowed', sa.Boolean(), nullable=False),
        sa.Column('updated_at', sa.DateTime(timezone=True), nullable=True),
        sa.Column('updated_by_id', sa.UUID(), nullable=True),
        sa.ForeignKeyConstraint(['updated_by_id'], ['users.id']),
        sa.PrimaryKeyConstraint('id'),
    )
    op.create_table(
        'gdpr_requests',
        sa.Column('user_id', sa.UUID(), nullable=False),
        sa.Column('username', sa.String(length=32), nullable=False),
        sa.Column('type', sa.Enum('export', 'deletion', name='gdpr_request_type'), nullable=False),
        sa.Column('status', sa.Enum('open', 'approved', 'completed', 'rejected', name='gdpr_request_status'), nullable=False),
        sa.Column('deadline_at', sa.DateTime(timezone=True), nullable=False),
        sa.Column('note', sa.Text(), nullable=True),
        sa.Column('created_by_id', sa.UUID(), nullable=True),
        sa.Column('approved_by_id', sa.UUID(), nullable=True),
        sa.Column('approved_at', sa.DateTime(timezone=True), nullable=True),
        sa.Column('completed_at', sa.DateTime(timezone=True), nullable=True),
        sa.Column('id', sa.UUID(), nullable=False),
        sa.Column('created_at', sa.DateTime(timezone=True), server_default=sa.text('now()'), nullable=False),
        sa.Column('updated_at', sa.DateTime(timezone=True), server_default=sa.text('now()'), nullable=False),
        sa.ForeignKeyConstraint(['user_id'], ['users.id']),
        sa.ForeignKeyConstraint(['created_by_id'], ['users.id']),
        sa.ForeignKeyConstraint(['approved_by_id'], ['users.id']),
        sa.PrimaryKeyConstraint('id'),
    )
    op.create_index(op.f('ix_gdpr_requests_user_id'), 'gdpr_requests', ['user_id'], unique=False)
    op.create_index(op.f('ix_gdpr_requests_status'), 'gdpr_requests', ['status'], unique=False)


def downgrade() -> None:
    op.drop_index(op.f('ix_gdpr_requests_status'), table_name='gdpr_requests')
    op.drop_index(op.f('ix_gdpr_requests_user_id'), table_name='gdpr_requests')
    op.drop_table('gdpr_requests')
    sa.Enum(name='gdpr_request_status').drop(op.get_bind(), checkfirst=True)
    sa.Enum(name='gdpr_request_type').drop(op.get_bind(), checkfirst=True)
    op.drop_table('platform_config')
    op.drop_column('users', 'ui_locale')
