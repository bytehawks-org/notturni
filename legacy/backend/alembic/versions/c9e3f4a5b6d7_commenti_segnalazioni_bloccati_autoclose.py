"""commenti: segnalazione alla piattaforma, lista bloccati per blog, chiusura automatica (todo/UX_REDESIGN.md B4)

Revision ID: c9e3f4a5b6d7
Revises: b8d2e3f4a5c6
Create Date: 2026-09-13 20:00:00

"""
from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


revision: str = 'c9e3f4a5b6d7'
down_revision: Union[str, None] = 'b8d2e3f4a5c6'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.add_column('comments', sa.Column('reported_to_platform', sa.Boolean(), server_default=sa.false(), nullable=False))
    op.add_column('comments', sa.Column('report_note', sa.Text(), nullable=True))
    op.add_column('comments', sa.Column('reported_at', sa.DateTime(timezone=True), nullable=True))
    op.add_column('blogs', sa.Column('comments_auto_close_days', sa.Integer(), nullable=True))
    op.create_table(
        'blog_blocked_authors',
        sa.Column('blog_id', sa.UUID(), nullable=False),
        sa.Column('user_id', sa.UUID(), nullable=True),
        sa.Column('email_hash', sa.String(length=64), nullable=True),
        sa.Column('label', sa.String(length=255), nullable=False),
        sa.Column('note', sa.String(length=255), nullable=True),
        sa.Column('created_by_id', sa.UUID(), nullable=False),
        sa.Column('id', sa.UUID(), nullable=False),
        sa.Column('created_at', sa.DateTime(timezone=True), server_default=sa.text('now()'), nullable=False),
        sa.Column('updated_at', sa.DateTime(timezone=True), server_default=sa.text('now()'), nullable=False),
        sa.ForeignKeyConstraint(['blog_id'], ['blogs.id'], ondelete='CASCADE'),
        sa.ForeignKeyConstraint(['user_id'], ['users.id']),
        sa.ForeignKeyConstraint(['created_by_id'], ['users.id']),
        sa.PrimaryKeyConstraint('id'),
    )
    op.create_index(op.f('ix_blog_blocked_authors_blog_id'), 'blog_blocked_authors', ['blog_id'], unique=False)


def downgrade() -> None:
    op.drop_index(op.f('ix_blog_blocked_authors_blog_id'), table_name='blog_blocked_authors')
    op.drop_table('blog_blocked_authors')
    op.drop_column('blogs', 'comments_auto_close_days')
    op.drop_column('comments', 'reported_at')
    op.drop_column('comments', 'report_note')
    op.drop_column('comments', 'reported_to_platform')
