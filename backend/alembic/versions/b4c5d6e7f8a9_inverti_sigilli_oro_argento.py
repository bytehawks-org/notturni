"""platform_config: inverte i sigilli oro/argento

Richiesta esplicita dopo il blocco precedente: il sigillo ORO deve
contraddistinguere le entità verificate a mano dalla piattaforma (testate,
agenzie, organizzazioni, personalità note — prima ARGENTO), l'ARGENTO i
sostenitori economici del progetto (prima ORO). Scambio puro dei nomi delle
due colonne (dato preservato, si sposta con la colonna): il codice applicativo
(app/domain/verification.py) non cambia, controlla `verification_gold_identifiers`
per primo e assegna GOLD, `verification_silver_identifiers` per secondo e
assegna SILVER — è il contenuto delle due colonne che qui si scambia.
Passaggio per una colonna temporanea: un rename diretto A<->B non è
esprimibile in un solo ALTER TABLE.

Revision ID: b4c5d6e7f8a9
Revises: a3f4b5c6d7e8
Create Date: 2026-09-20 11:00:00.000000

"""
from typing import Sequence, Union

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "b4c5d6e7f8a9"
down_revision: Union[str, None] = "a3f4b5c6d7e8"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def _swap() -> None:
    op.alter_column("platform_config", "verification_gold_identifiers", new_column_name="verification_tmp_swap")
    op.alter_column("platform_config", "verification_silver_identifiers", new_column_name="verification_gold_identifiers")
    op.alter_column("platform_config", "verification_tmp_swap", new_column_name="verification_silver_identifiers")


def upgrade() -> None:
    _swap()


def downgrade() -> None:
    # Lo scambio è la propria inversa.
    _swap()
