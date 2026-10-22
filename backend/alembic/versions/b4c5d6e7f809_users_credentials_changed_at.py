"""users.credentials_changed_at: revoca degli access token JWT stateless

Le UserSession (refresh token) vengono già cancellate al cambio password
(POST /users/me/password) e al reset "password dimenticata"
(app/domain/password_reset.py), ma gli access token JWT sono stateless e
restano validi fino al loro `exp` naturale — nessuna revoca immediata era
possibile per loro. Questa colonna, confrontata con l'`iat` del token in
app/api/deps.py::get_current_user, chiude quella finestra.

Revision ID: b4c5d6e7f809
Revises: a0579cc46db6
Create Date: 2026-09-19 00:00:00.000000

"""
from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op

# revision identifiers, used by Alembic.
revision: str = "b4c5d6e7f809"
down_revision: Union[str, None] = "a0579cc46db6"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.add_column(
        "users",
        sa.Column("credentials_changed_at", sa.DateTime(timezone=True), nullable=True),
    )


def downgrade() -> None:
    op.drop_column("users", "credentials_changed_at")
