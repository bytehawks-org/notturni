"""merge head interessi utente e newsletter

Merge dei due head divergenti 4d23cdb0ccc3 (interessi utente:
platform_config.interests + users.interests) e 769f5009bad5 (newsletter:
iscritti, campagne, opt-out per blog), creati su branch paralleli senza una
migrazione di merge dedicata — stessa situazione già risolta in passato da
cf49d59f6471. Nessuna modifica di schema propria: solo un nodo di
convergenza nel grafo delle revisioni, necessario perché `alembic upgrade
head` funzioni di nuovo (con due head "head" è ambiguo, comando rifiutato).

Revision ID: a0579cc46db6
Revises: 4d23cdb0ccc3, 769f5009bad5
Create Date: 2026-09-19 17:19:11.767700

"""
from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op

# revision identifiers, used by Alembic.
revision: str = "a0579cc46db6"
down_revision: Union[str, Sequence[str], None] = ("4d23cdb0ccc3", "769f5009bad5")
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    pass


def downgrade() -> None:
    pass
