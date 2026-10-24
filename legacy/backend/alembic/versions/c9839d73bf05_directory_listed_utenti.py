"""opt-out directory pubblica utenti (directory_listed)

Revision ID: c9839d73bf05
Revises: b2c3d4e5f6a7
Create Date: 2026-09-19 12:00:00.000000

"""
from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


# revision identifiers, used by Alembic.
revision: str = 'c9839d73bf05'
down_revision: Union[str, None] = 'b2c3d4e5f6a7'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    # server_default per popolare le righe esistenti (users non è vuota) al
    # momento dell'ALTER TABLE; il default a livello applicativo resta in
    # app/models/user.py, qui serve solo per la migrazione stessa (stesso
    # schema di Blog.search_indexing_enabled).
    op.add_column(
        'users',
        sa.Column('directory_listed', sa.Boolean(), nullable=False, server_default=sa.text('true')),
    )
    op.alter_column('users', 'directory_listed', server_default=None)


def downgrade() -> None:
    op.drop_column('users', 'directory_listed')
