"""dominio verificato come opzione di "Firma i miei post come" + merge heads

Copre l'estensione di todo/USERS.md #2 (post_author_name_style): l'utente può
ora scegliere di firmare post/commenti/profilo con il proprio dominio custom
verificato invece che username/nome e cognome/alias.

- `users.post_author_name_style`: nuovo valore enum `verified_domain`.
- `users.verified_domain`: copia denormalizzata di `custom_domains.domain`,
  valorizzata solo quando lo stato è `verified` (tenuta in sync in
  app/api/v1/users.py — vedi verify_my_domain/delete_my_domain — così i
  tanti punti che risolvono il nome pubblico di un utente, incluso il
  render di post e commenti, non devono fare eager-load della relazione
  `custom_domain` solo per questo). Backfill dai domini già verificati.

Anche merge dei due head divergenti c1d2e3f4a5b6 (dominio custom/verifica
email) e c4d6e7f8a9b0 (campi BibTeX), creati su branch paralleli senza una
migrazione di merge dedicata.

Revision ID: cf49d59f6471
Revises: c1d2e3f4a5b6, c4d6e7f8a9b0
Create Date: 2026-09-16 19:30:00.000000

"""
from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op

# revision identifiers, used by Alembic.
revision: str = "cf49d59f6471"
down_revision: Union[str, Sequence[str], None] = ("c1d2e3f4a5b6", "c4d6e7f8a9b0")
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    # Alembic autogenerate non rileva l'aggiunta di valori a un enum nativo
    # Postgres esistente: va scritta a mano. ADD VALUE è supportato dentro
    # una transazione da Postgres 12+ (stesso pattern di 2807a24ea58f).
    op.execute("ALTER TYPE post_author_name_style ADD VALUE IF NOT EXISTS 'verified_domain'")

    op.add_column("users", sa.Column("verified_domain", sa.String(255), nullable=True))
    op.execute(
        """
        UPDATE users
        SET verified_domain = custom_domains.domain
        FROM custom_domains
        WHERE custom_domains.user_id = users.id
          AND custom_domains.status = 'verified'
        """
    )


def downgrade() -> None:
    op.drop_column("users", "verified_domain")
    # Postgres non supporta la rimozione di un valore da un enum nativo
    # (richiederebbe ricreare il tipo e la colonna): non implementato, stesso
    # limite documentato in 2807a24ea58f.
