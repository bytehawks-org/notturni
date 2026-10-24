"""modalita commenti per blog e post, thread di risposte

Revision ID: 7981bf8eb571
Revises: b1c2d3e4f5a6
Create Date: 2026-09-06 15:53:30.509848

"""
from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa
from sqlalchemy.dialects import postgresql


# revision identifiers, used by Alembic.
revision: str = '7981bf8eb571'
down_revision: Union[str, None] = 'b1c2d3e4f5a6'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None

comments_mode_enum = postgresql.ENUM('everyone', 'members', 'closed', name='comments_mode')


def upgrade() -> None:
    comments_mode_enum.create(op.get_bind(), checkfirst=True)

    # Sostituisce blogs.allow_anonymous_comments (bool): backfill prima di
    # imporre NOT NULL, così non fallisce su righe già esistenti.
    op.add_column('blogs', sa.Column('comments_mode', comments_mode_enum, nullable=True))
    op.execute(
        "UPDATE blogs SET comments_mode = CASE WHEN allow_anonymous_comments "
        "THEN 'everyone'::comments_mode ELSE 'members'::comments_mode END"
    )
    op.alter_column('blogs', 'comments_mode', nullable=False)
    op.drop_column('blogs', 'allow_anonymous_comments')

    # Thread di risposte: nullable, un commento di primo livello non ne ha.
    op.add_column('comments', sa.Column('parent_id', sa.UUID(), nullable=True))
    op.create_index(op.f('ix_comments_parent_id'), 'comments', ['parent_id'], unique=False)
    op.create_foreign_key(
        'comments_parent_id_fkey', 'comments', 'comments', ['parent_id'], ['id'], ondelete='CASCADE'
    )

    # Override per singolo post: NULL = eredita da Blog.comments_mode.
    op.add_column('posts', sa.Column('comments_mode', comments_mode_enum, nullable=True))


def downgrade() -> None:
    op.drop_column('posts', 'comments_mode')

    op.drop_constraint('comments_parent_id_fkey', 'comments', type_='foreignkey')
    op.drop_index(op.f('ix_comments_parent_id'), table_name='comments')
    op.drop_column('comments', 'parent_id')

    op.add_column(
        'blogs',
        sa.Column(
            'allow_anonymous_comments', sa.BOOLEAN(), server_default=sa.text('false'), nullable=False
        ),
    )
    op.execute("UPDATE blogs SET allow_anonymous_comments = (comments_mode = 'everyone')")
    op.drop_column('blogs', 'comments_mode')

    comments_mode_enum.drop(op.get_bind(), checkfirst=True)
