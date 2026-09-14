"""note del blog come entità (todo/UX_REDESIGN.md B8)

Revision ID: a3c7d8e9f0b1
Revises: f2b6c7d8e9a0
Create Date: 2026-09-14 00:00:00

"""
from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


revision: str = 'a3c7d8e9f0b1'
down_revision: Union[str, None] = 'f2b6c7d8e9a0'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.create_table(
        'blog_notes',
        sa.Column('blog_id', sa.UUID(), nullable=False),
        sa.Column('content', sa.Text(), nullable=False),
        sa.Column('normalized', sa.String(length=600), nullable=False),
        sa.Column('kind', sa.String(length=10), nullable=False),
        sa.Column('url', sa.Text(), nullable=True),
        sa.Column('created_by_id', sa.UUID(), nullable=True),
        sa.Column('id', sa.UUID(), nullable=False),
        sa.Column('created_at', sa.DateTime(timezone=True), server_default=sa.text('now()'), nullable=False),
        sa.Column('updated_at', sa.DateTime(timezone=True), server_default=sa.text('now()'), nullable=False),
        sa.ForeignKeyConstraint(['blog_id'], ['blogs.id'], ondelete='CASCADE'),
        sa.ForeignKeyConstraint(['created_by_id'], ['users.id']),
        sa.PrimaryKeyConstraint('id'),
    )
    op.create_index(op.f('ix_blog_notes_blog_id'), 'blog_notes', ['blog_id'], unique=False)
    op.create_index(op.f('ix_blog_notes_normalized'), 'blog_notes', ['normalized'], unique=False)
    op.add_column('post_notes', sa.Column('note_id', sa.UUID(), nullable=True))
    op.create_foreign_key('fk_post_notes_note_id', 'post_notes', 'blog_notes', ['note_id'], ['id'], ondelete='SET NULL')
    op.create_index(op.f('ix_post_notes_note_id'), 'post_notes', ['note_id'], unique=False)


def downgrade() -> None:
    op.drop_index(op.f('ix_post_notes_note_id'), table_name='post_notes')
    op.drop_constraint('fk_post_notes_note_id', 'post_notes', type_='foreignkey')
    op.drop_column('post_notes', 'note_id')
    op.drop_index(op.f('ix_blog_notes_normalized'), table_name='blog_notes')
    op.drop_index(op.f('ix_blog_notes_blog_id'), table_name='blog_notes')
    op.drop_table('blog_notes')
