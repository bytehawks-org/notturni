"""pubblicazioni: capitoli ordinati sotto /{blog}/pub/{name} (todo/PUBLICATIONS.md, B9)

Revision ID: b4d8e9f0a1c2
Revises: a3c7d8e9f0b1
Create Date: 2026-09-14 01:00:00

"""
from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


revision: str = 'b4d8e9f0a1c2'
down_revision: Union[str, None] = 'a3c7d8e9f0b1'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.create_table(
        'publications',
        sa.Column('blog_id', sa.UUID(), nullable=False),
        sa.Column('name', sa.String(length=60), nullable=False),
        sa.Column('title', sa.String(length=255), nullable=False),
        sa.Column('description', sa.Text(), nullable=True),
        sa.Column('id', sa.UUID(), nullable=False),
        sa.Column('created_at', sa.DateTime(timezone=True), server_default=sa.text('now()'), nullable=False),
        sa.Column('updated_at', sa.DateTime(timezone=True), server_default=sa.text('now()'), nullable=False),
        sa.ForeignKeyConstraint(['blog_id'], ['blogs.id'], ondelete='CASCADE'),
        sa.PrimaryKeyConstraint('id'),
        sa.UniqueConstraint('blog_id', 'name', name='uq_publication_blog_name'),
    )
    op.create_index(op.f('ix_publications_blog_id'), 'publications', ['blog_id'], unique=False)
    op.add_column('posts', sa.Column('publication_id', sa.UUID(), nullable=True))
    op.add_column('posts', sa.Column('chapter_order', sa.Integer(), nullable=True))
    op.create_foreign_key('fk_posts_publication_id', 'posts', 'publications', ['publication_id'], ['id'], ondelete='SET NULL')
    op.create_index(op.f('ix_posts_publication_id'), 'posts', ['publication_id'], unique=False)


def downgrade() -> None:
    op.drop_index(op.f('ix_posts_publication_id'), table_name='posts')
    op.drop_constraint('fk_posts_publication_id', 'posts', type_='foreignkey')
    op.drop_column('posts', 'chapter_order')
    op.drop_column('posts', 'publication_id')
    op.drop_index(op.f('ix_publications_blog_id'), table_name='publications')
    op.drop_table('publications')
