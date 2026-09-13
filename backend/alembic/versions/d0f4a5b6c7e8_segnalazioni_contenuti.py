"""segnalazioni di blog e post dai lettori (todo/UX_REDESIGN.md B5)

Revision ID: d0f4a5b6c7e8
Revises: c9e3f4a5b6d7
Create Date: 2026-09-13 21:00:00

"""
from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


revision: str = 'd0f4a5b6c7e8'
down_revision: Union[str, None] = 'c9e3f4a5b6d7'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.create_table(
        'content_reports',
        sa.Column('reporter_id', sa.UUID(), nullable=False),
        sa.Column('target_type', sa.Enum('blog', 'post', name='report_target_type'), nullable=False),
        sa.Column('target_id', sa.UUID(), nullable=False),
        sa.Column('blog_id', sa.UUID(), nullable=False),
        sa.Column('reason', sa.Enum('spam', 'abuse', 'illegal', 'other', name='report_reason'), nullable=False),
        sa.Column('note', sa.String(length=500), nullable=True),
        sa.Column('status', sa.Enum('open', 'dismissed', 'actioned', name='report_status'), nullable=False),
        sa.Column('resolved_at', sa.DateTime(timezone=True), nullable=True),
        sa.Column('resolved_by_id', sa.UUID(), nullable=True),
        sa.Column('id', sa.UUID(), nullable=False),
        sa.Column('created_at', sa.DateTime(timezone=True), server_default=sa.text('now()'), nullable=False),
        sa.Column('updated_at', sa.DateTime(timezone=True), server_default=sa.text('now()'), nullable=False),
        sa.ForeignKeyConstraint(['blog_id'], ['blogs.id'], ondelete='CASCADE'),
        sa.ForeignKeyConstraint(['reporter_id'], ['users.id']),
        sa.ForeignKeyConstraint(['resolved_by_id'], ['users.id']),
        sa.PrimaryKeyConstraint('id'),
        sa.UniqueConstraint('reporter_id', 'target_type', 'target_id', name='uq_content_report_per_reporter'),
    )
    op.create_index(op.f('ix_content_reports_blog_id'), 'content_reports', ['blog_id'], unique=False)
    op.create_index(op.f('ix_content_reports_reporter_id'), 'content_reports', ['reporter_id'], unique=False)
    op.create_index(op.f('ix_content_reports_status'), 'content_reports', ['status'], unique=False)
    op.create_index(op.f('ix_content_reports_target_id'), 'content_reports', ['target_id'], unique=False)


def downgrade() -> None:
    op.drop_index(op.f('ix_content_reports_target_id'), table_name='content_reports')
    op.drop_index(op.f('ix_content_reports_status'), table_name='content_reports')
    op.drop_index(op.f('ix_content_reports_reporter_id'), table_name='content_reports')
    op.drop_index(op.f('ix_content_reports_blog_id'), table_name='content_reports')
    op.drop_table('content_reports')
    sa.Enum(name='report_status').drop(op.get_bind(), checkfirst=True)
    sa.Enum(name='report_reason').drop(op.get_bind(), checkfirst=True)
    sa.Enum(name='report_target_type').drop(op.get_bind(), checkfirst=True)
