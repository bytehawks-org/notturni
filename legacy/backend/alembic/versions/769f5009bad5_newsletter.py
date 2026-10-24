"""newsletter/mailing-list: iscritti, campagne, opt-out per blog

Revision ID: 769f5009bad5
Revises: cf49d59f6471
Create Date: 2026-09-19 12:00:00

"""
from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op

# revision identifiers, used by Alembic.
revision: str = "769f5009bad5"
down_revision: Union[str, None] = "cf49d59f6471"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.add_column(
        "blogs",
        sa.Column(
            "newsletter_auto_notify_enabled", sa.Boolean(), nullable=False, server_default=sa.true()
        ),
    )
    # server_default solo per il backfill delle righe esistenti: la colonna
    # ORM non lo porta (stesso pattern di search_indexing_enabled/
    # ai_crawling_enabled, che non hanno server_default qui in alembic).
    op.alter_column("blogs", "newsletter_auto_notify_enabled", server_default=None)

    op.create_table(
        "newsletter_subscribers",
        sa.Column("blog_id", sa.UUID(), nullable=True),
        sa.Column("email", sa.String(length=255), nullable=False),
        sa.Column("locale", sa.String(length=2), nullable=True),
        sa.Column(
            "status",
            sa.Enum("pending", "confirmed", "unsubscribed", name="newsletter_subscriber_status"),
            nullable=False,
        ),
        sa.Column("user_id", sa.UUID(), nullable=True),
        sa.Column("confirm_token_hash", sa.String(length=64), nullable=True),
        sa.Column("confirm_token_expires_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("consent_ip", sa.String(length=64), nullable=True),
        sa.Column("consent_user_agent", sa.String(length=512), nullable=True),
        sa.Column("confirmed_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("unsubscribed_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("unsubscribe_reason", sa.String(length=500), nullable=True),
        sa.Column("id", sa.UUID(), nullable=False),
        sa.Column("created_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
        sa.Column("updated_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
        sa.ForeignKeyConstraint(["blog_id"], ["blogs.id"], ondelete="CASCADE"),
        sa.ForeignKeyConstraint(["user_id"], ["users.id"], ondelete="SET NULL"),
        sa.PrimaryKeyConstraint("id"),
    )
    op.create_index(
        op.f("ix_newsletter_subscribers_confirm_token_hash"),
        "newsletter_subscribers",
        ["confirm_token_hash"],
        unique=False,
    )
    # Una sola riga per email per lista: per un blog specifico (blog_id non
    # null) o per il digest di piattaforma (blog_id null) — due indici
    # parziali invece di un unique constraint semplice perché NULL non è mai
    # uguale a se stesso in un vincolo UNIQUE standard di Postgres (due
    # iscrizioni al digest con la stessa email passerebbero altrimenti).
    op.create_index(
        "uq_newsletter_subscriber_blog_email",
        "newsletter_subscribers",
        ["blog_id", sa.text("lower(email)")],
        unique=True,
        postgresql_where=sa.text("blog_id IS NOT NULL"),
    )
    op.create_index(
        "uq_newsletter_subscriber_platform_email",
        "newsletter_subscribers",
        [sa.text("lower(email)")],
        unique=True,
        postgresql_where=sa.text("blog_id IS NULL"),
    )

    op.create_table(
        "newsletter_campaigns",
        sa.Column("blog_id", sa.UUID(), nullable=True),
        sa.Column(
            "kind",
            sa.Enum("post_notification", "manual", name="newsletter_campaign_kind"),
            nullable=False,
        ),
        sa.Column("post_id", sa.UUID(), nullable=True),
        sa.Column("created_by_id", sa.UUID(), nullable=True),
        sa.Column("subject", sa.String(length=255), nullable=False),
        sa.Column("body_markdown", sa.Text(), nullable=True),
        sa.Column(
            "status",
            sa.Enum(
                "draft", "scheduled", "sending", "sent", "canceled", "failed",
                name="newsletter_campaign_status",
            ),
            nullable=False,
        ),
        sa.Column("scheduled_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("sent_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("recipient_count", sa.Integer(), nullable=False),
        sa.Column("failed_count", sa.Integer(), nullable=False),
        sa.Column("id", sa.UUID(), nullable=False),
        sa.Column("created_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
        sa.Column("updated_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False),
        sa.ForeignKeyConstraint(["blog_id"], ["blogs.id"], ondelete="CASCADE"),
        sa.ForeignKeyConstraint(["post_id"], ["posts.id"], ondelete="CASCADE"),
        sa.ForeignKeyConstraint(["created_by_id"], ["users.id"], ondelete="SET NULL"),
        sa.PrimaryKeyConstraint("id"),
    )
    # Una sola campagna automatica per post: impedisce il doppio invio se la
    # pubblicazione viene richiamata più volte (es. ritorno in bozza e
    # ripubblicazione) — vedi app/api/v1/posts.py:_queue_newsletter_notification.
    op.create_index(
        "uq_newsletter_campaign_post",
        "newsletter_campaigns",
        ["post_id"],
        unique=True,
        postgresql_where=sa.text("post_id IS NOT NULL"),
    )


def downgrade() -> None:
    op.drop_index("uq_newsletter_campaign_post", table_name="newsletter_campaigns")
    op.drop_table("newsletter_campaigns")
    sa.Enum(name="newsletter_campaign_status").drop(op.get_bind(), checkfirst=True)
    sa.Enum(name="newsletter_campaign_kind").drop(op.get_bind(), checkfirst=True)

    op.drop_index("uq_newsletter_subscriber_platform_email", table_name="newsletter_subscribers")
    op.drop_index("uq_newsletter_subscriber_blog_email", table_name="newsletter_subscribers")
    op.drop_index(
        op.f("ix_newsletter_subscribers_confirm_token_hash"), table_name="newsletter_subscribers"
    )
    op.drop_table("newsletter_subscribers")
    sa.Enum(name="newsletter_subscriber_status").drop(op.get_bind(), checkfirst=True)

    op.drop_column("blogs", "newsletter_auto_notify_enabled")
