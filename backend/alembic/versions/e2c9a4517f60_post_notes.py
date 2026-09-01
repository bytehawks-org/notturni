"""note a piè di pagina dei post (bibliografia del blog)

todo/EDITOR.md: tabella `post_notes` (sorgente di verità delle note di un
post, riscritta ad ogni salvataggio) usata per l'elenco a piè di pagina nel
post e per la bibliografia automatica del blog.

Revision ID: e2c9a4517f60
Revises: d8b3f1027a45
Create Date: 2026-09-01 15:00:00.000000

"""
from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op

# revision identifiers, used by Alembic.
revision: str = "e2c9a4517f60"
down_revision: Union[str, None] = "d8b3f1027a45"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.create_table(
        "post_notes",
        sa.Column("post_id", sa.UUID(), nullable=False),
        sa.Column("idx", sa.Integer(), nullable=False),
        sa.Column("content", sa.Text(), nullable=False),
        sa.ForeignKeyConstraint(["post_id"], ["posts.id"], ondelete="CASCADE"),
        sa.PrimaryKeyConstraint("post_id", "idx"),
    )


def downgrade() -> None:
    op.drop_table("post_notes")
