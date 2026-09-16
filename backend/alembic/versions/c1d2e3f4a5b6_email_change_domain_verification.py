"""cambio email verificato, cooldown username, dominio custom verificato via DNS, badge di verifica

Copre 4 delle 6 richieste del blocco "profilo utente" (le altre due sono solo
frontend/calcolate a runtime, nessuna migrazione):
- `users.username_changed_at`: cooldown di 5 giorni tra due cambi username
  (PATCH /api/v1/users/me).
- `email_change_requests`: cambio email a due passi (codice alla vecchia
  casella, poi alla nuova) — tabella dedicata, non riusa `mfa_email_codes`
  per non confondere codici di login con codici di cambio email.
- `custom_domains` + `users.verification_tier`: dominio personalizzato
  verificato via record TXT sul DNS (stile Bluesky), assegna il sigillo di
  verifica "bronzo" — silver/gold/blue riservati per future integrazioni,
  nessuna logica li assegna oggi.

Revision ID: c1d2e3f4a5b6
Revises: f1b2c3d4e5a6
Create Date: 2026-09-15 00:05:00.000000

"""
from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op
from sqlalchemy.dialects.postgresql import UUID
from sqlalchemy.dialects import postgresql

# revision identifiers, used by Alembic.
revision: str = "c1d2e3f4a5b6"
down_revision: Union[str, None] = "f1b2c3d4e5a6"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


# create_type=False sui tipi usati nelle colonne: il tipo viene creato
# esplicitamente una sola volta sotto (.create(checkfirst=True)) — altrimenti
# il compilatore DDL di postgres prova a ri-creare il tipo anche in fase di
# CREATE TABLE/ADD COLUMN, fallendo con "type already exists" nella stessa
# transazione.
verification_tier = postgresql.ENUM(
    "none", "bronze", "silver", "gold", "blue", name="verification_tier", create_type=False
)
custom_domain_status = postgresql.ENUM(
    "pending", "verified", "failed", name="custom_domain_status", create_type=False
)


def upgrade() -> None:
    bind = op.get_bind()

    op.add_column("users", sa.Column("username_changed_at", sa.DateTime(timezone=True), nullable=True))

    verification_tier.create(bind, checkfirst=True)
    op.add_column(
        "users",
        sa.Column("verification_tier", verification_tier, nullable=False, server_default="none"),
    )

    op.create_table(
        "email_change_requests",
        sa.Column("id", UUID(as_uuid=True), primary_key=True),
        sa.Column("user_id", UUID(as_uuid=True), sa.ForeignKey("users.id"), nullable=False),
        sa.Column("new_email", sa.String(255), nullable=False),
        sa.Column("old_code_hash", sa.String(64), nullable=False),
        sa.Column("old_expires_at", sa.DateTime(timezone=True), nullable=False),
        sa.Column("old_consumed_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("new_code_hash", sa.String(64), nullable=True),
        sa.Column("new_expires_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("new_consumed_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("completed_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("created_at", sa.DateTime(timezone=True), server_default=sa.func.now(), nullable=False),
        sa.Column(
            "updated_at",
            sa.DateTime(timezone=True),
            server_default=sa.func.now(),
            onupdate=sa.func.now(),
            nullable=False,
        ),
    )
    op.create_index(
        "ix_email_change_requests_user_id", "email_change_requests", ["user_id"]
    )

    custom_domain_status.create(bind, checkfirst=True)
    op.create_table(
        "custom_domains",
        sa.Column("id", UUID(as_uuid=True), primary_key=True),
        sa.Column("user_id", UUID(as_uuid=True), sa.ForeignKey("users.id"), nullable=False, unique=True),
        sa.Column("domain", sa.String(255), nullable=False, unique=True),
        sa.Column("verification_token", sa.String(64), nullable=False),
        sa.Column("status", custom_domain_status, nullable=False, server_default="pending"),
        sa.Column("verified_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("created_at", sa.DateTime(timezone=True), server_default=sa.func.now(), nullable=False),
        sa.Column(
            "updated_at",
            sa.DateTime(timezone=True),
            server_default=sa.func.now(),
            onupdate=sa.func.now(),
            nullable=False,
        ),
    )


def downgrade() -> None:
    bind = op.get_bind()

    op.drop_table("custom_domains")
    custom_domain_status.drop(bind, checkfirst=True)

    op.drop_index("ix_email_change_requests_user_id", table_name="email_change_requests")
    op.drop_table("email_change_requests")

    op.drop_column("users", "verification_tier")
    verification_tier.drop(bind, checkfirst=True)

    op.drop_column("users", "username_changed_at")
