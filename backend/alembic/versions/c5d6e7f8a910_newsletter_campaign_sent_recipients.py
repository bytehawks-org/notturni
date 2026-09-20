"""newsletter_campaigns.sent_to_subscriber_ids: idempotenza su redelivery

Il consumer (app/workers/newsletter_consumer.py) fa nack/requeue del
messaggio su qualunque eccezione non gestita: se il processo viene
interrotto a metà del ciclo sui destinatari, il broker riconsegna lo stesso
messaggio e, senza questa colonna, _process_campaign rispedirebbe l'email
anche a chi l'ha già ricevuta nel tentativo precedente.

Revision ID: c5d6e7f8a910
Revises: b4c5d6e7f809
Create Date: 2026-09-19 00:05:00.000000

"""
from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op
from sqlalchemy.dialects.postgresql import UUID

# revision identifiers, used by Alembic.
revision: str = "c5d6e7f8a910"
down_revision: Union[str, None] = "b4c5d6e7f809"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.add_column(
        "newsletter_campaigns",
        sa.Column(
            "sent_to_subscriber_ids",
            sa.ARRAY(UUID(as_uuid=True)),
            nullable=False,
            server_default="{}",
        ),
    )
    op.alter_column("newsletter_campaigns", "sent_to_subscriber_ids", server_default=None)


def downgrade() -> None:
    op.drop_column("newsletter_campaigns", "sent_to_subscriber_ids")
