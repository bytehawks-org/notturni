"""blog: sottotitolo/descrizione, visibilità, inviti collaboratori, alias autore

Copre todo/BLOG.md:
- #1: `blogs.subtitle` (max 64), `blogs.description` (max 256)
- #2: `blogs.visibility` (public/members/private)
- #3: tabella `blog_invitations` (+ enum `blog_invitation_status`)
- #4: `users.display_name` (alias globale) e
  `blog_memberships.author_display_name` (alias per-blog)

Revision ID: c4e1a7f2b830
Revises: b16963e9cdcb
Create Date: 2026-09-01 12:00:00.000000

"""
from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op
from sqlalchemy.dialects import postgresql

# revision identifiers, used by Alembic.
revision: str = "c4e1a7f2b830"
down_revision: Union[str, None] = "b16963e9cdcb"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


blog_visibility = sa.Enum("public", "members", "private", name="blog_visibility")
blog_invitation_status = sa.Enum(
    "pending", "accepted", "declined", "revoked", name="blog_invitation_status"
)


def upgrade() -> None:
    bind = op.get_bind()

    # #4 — alias globale sul profilo utente
    op.add_column("users", sa.Column("display_name", sa.String(length=255), nullable=True))

    # #1 — sottotitolo e descrizione breve
    op.add_column("blogs", sa.Column("subtitle", sa.String(length=64), nullable=True))
    op.add_column("blogs", sa.Column("description", sa.String(length=256), nullable=True))

    # #2 — visibilità del blog
    blog_visibility.create(bind, checkfirst=True)
    op.add_column(
        "blogs",
        sa.Column(
            "visibility",
            blog_visibility,
            nullable=False,
            server_default="public",
        ),
    )

    # #4 — alias per-blog sulla membership
    op.add_column(
        "blog_memberships",
        sa.Column("author_display_name", sa.String(length=255), nullable=True),
    )

    # #3 — inviti a collaborare
    blog_invitation_status.create(bind, checkfirst=True)
    op.create_table(
        "blog_invitations",
        sa.Column("blog_id", sa.UUID(), nullable=False),
        sa.Column("invited_user_id", sa.UUID(), nullable=False),
        sa.Column("invited_by_id", sa.UUID(), nullable=False),
        sa.Column(
            "role",
            postgresql.ENUM(
                "autore", "co_autore", "revisore", "mediatore",
                name="blog_role",
                create_type=False,
            ),
            nullable=False,
        ),
        sa.Column(
            "status",
            postgresql.ENUM(name="blog_invitation_status", create_type=False),
            nullable=False,
            server_default="pending",
        ),
        sa.Column("responded_at", sa.DateTime(timezone=True), nullable=True),
        sa.Column("id", sa.UUID(), nullable=False),
        sa.Column(
            "created_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False
        ),
        sa.Column(
            "updated_at", sa.DateTime(timezone=True), server_default=sa.text("now()"), nullable=False
        ),
        sa.ForeignKeyConstraint(["blog_id"], ["blogs.id"], ondelete="CASCADE"),
        sa.ForeignKeyConstraint(["invited_user_id"], ["users.id"]),
        sa.ForeignKeyConstraint(["invited_by_id"], ["users.id"]),
        sa.PrimaryKeyConstraint("id"),
        sa.UniqueConstraint(
            "blog_id", "invited_user_id", name="uq_blog_invitation_blog_user"
        ),
    )


def downgrade() -> None:
    bind = op.get_bind()

    op.drop_table("blog_invitations")
    blog_invitation_status.drop(bind, checkfirst=True)

    op.drop_column("blog_memberships", "author_display_name")

    op.drop_column("blogs", "visibility")
    blog_visibility.drop(bind, checkfirst=True)

    op.drop_column("blogs", "description")
    op.drop_column("blogs", "subtitle")

    op.drop_column("users", "display_name")
