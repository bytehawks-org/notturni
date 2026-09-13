"""libreria media per blog (todo/UX_REDESIGN.md B7)

Revision ID: f2b6c7d8e9a0
Revises: e1a5b6c7d8f9
Create Date: 2026-09-13 23:00:00

"""
from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


revision: str = 'f2b6c7d8e9a0'
down_revision: Union[str, None] = 'e1a5b6c7d8f9'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.create_table(
        'media_files',
        sa.Column('blog_id', sa.UUID(), nullable=False),
        sa.Column('uploader_id', sa.UUID(), nullable=True),
        sa.Column('object_key', sa.String(length=512), nullable=True),
        sa.Column('url', sa.Text(), nullable=False),
        sa.Column('content_type', sa.String(length=100), nullable=False),
        sa.Column('size_bytes', sa.Integer(), nullable=False),
        sa.Column('alt_text', sa.Text(), nullable=False),
        sa.Column('caption', sa.Text(), nullable=True),
        sa.Column('categories', sa.ARRAY(sa.String(length=20)), nullable=False),
        sa.Column('is_sensitive', sa.Boolean(), nullable=False),
        sa.Column('id', sa.UUID(), nullable=False),
        sa.Column('created_at', sa.DateTime(timezone=True), server_default=sa.text('now()'), nullable=False),
        sa.Column('updated_at', sa.DateTime(timezone=True), server_default=sa.text('now()'), nullable=False),
        sa.ForeignKeyConstraint(['blog_id'], ['blogs.id'], ondelete='CASCADE'),
        sa.ForeignKeyConstraint(['uploader_id'], ['users.id']),
        sa.PrimaryKeyConstraint('id'),
        sa.UniqueConstraint('url'),
    )
    op.create_index(op.f('ix_media_files_blog_id'), 'media_files', ['blog_id'], unique=False)


def downgrade() -> None:
    op.drop_index(op.f('ix_media_files_blog_id'), table_name='media_files')
    op.drop_table('media_files')
