"""username: menzioni @, stile nome autore sui post, toggle menzioni per blog

Copre todo/USERS.md e la parte @menzioni di todo/EDITOR.md:
- USERS.md #2: `users.post_author_name_style` (username | full_name | display_name)
- EDITOR.md: `blogs.mentions_enabled` (@menzioni trasformate in link, attive di default)

Il vincolo di formato sullo username (USERS.md #1) è solo applicativo
(app/domain/usernames.py), non un check a DB: gli username già esistenti
restano validi.

Revision ID: d8b3f1027a45
Revises: c4e1a7f2b830
Create Date: 2026-09-01 13:30:00.000000

"""
from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op

# revision identifiers, used by Alembic.
revision: str = "d8b3f1027a45"
down_revision: Union[str, None] = "c4e1a7f2b830"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


post_author_name_style = sa.Enum(
    "username", "full_name", "display_name", name="post_author_name_style"
)


def upgrade() -> None:
    bind = op.get_bind()

    post_author_name_style.create(bind, checkfirst=True)
    op.add_column(
        "users",
        sa.Column(
            "post_author_name_style",
            post_author_name_style,
            nullable=False,
            server_default="username",
        ),
    )

    op.add_column(
        "blogs",
        sa.Column(
            "mentions_enabled",
            sa.Boolean(),
            nullable=False,
            server_default=sa.true(),
        ),
    )


def downgrade() -> None:
    bind = op.get_bind()

    op.drop_column("blogs", "mentions_enabled")
    op.drop_column("users", "post_author_name_style")
    post_author_name_style.drop(bind, checkfirst=True)
