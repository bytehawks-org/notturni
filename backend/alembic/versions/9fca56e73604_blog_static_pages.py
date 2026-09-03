"""pagine statiche per blog (opt-in)

CLAUDE.md #1: "Add capability to blog owner (Author) to create static pages",
estende le pagine statiche (finora solo per il sito principale, vedi
d8b3f1027a45 e precedenti) ai blog utente:
- `pages.blog_id` (nullable): NULL = pagina di piattaforma, valorizzato =
  pagina di un blog.
- `blogs.static_pages_enabled`: feature opt-in per blog, disattiva di
  default (a differenza di `mentions_enabled`) — sempre attiva invece per le
  pagine di piattaforma.
- Il vincolo unico globale su (slug, locale) delle pagine di piattaforma
  diventa un indice parziale (blog_id IS NULL): un vincolo unique su
  (blog_id, slug, locale) non basterebbe da solo, perché in Postgres due
  NULL su blog_id non collidono.

Revision ID: 9fca56e73604
Revises: e2c9a4517f60
Create Date: 2026-09-02 12:00:00.000000

"""
from typing import Sequence, Union

import sqlalchemy as sa
from alembic import op

# revision identifiers, used by Alembic.
revision: str = "9fca56e73604"
down_revision: Union[str, None] = "e2c9a4517f60"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.add_column(
        "blogs",
        sa.Column("static_pages_enabled", sa.Boolean(), nullable=False, server_default=sa.false()),
    )

    op.add_column("pages", sa.Column("blog_id", sa.UUID(), nullable=True))
    op.create_foreign_key(
        "fk_pages_blog_id_blogs", "pages", "blogs", ["blog_id"], ["id"], ondelete="CASCADE"
    )
    op.create_index("ix_pages_blog_id", "pages", ["blog_id"])

    op.drop_constraint("uq_page_slug_locale", "pages", type_="unique")
    op.create_unique_constraint(
        "uq_page_blog_slug_locale", "pages", ["blog_id", "slug", "locale"]
    )
    op.create_index(
        "uq_page_slug_locale_platform",
        "pages",
        ["slug", "locale"],
        unique=True,
        postgresql_where=sa.text("blog_id IS NULL"),
    )


def downgrade() -> None:
    op.drop_index("uq_page_slug_locale_platform", table_name="pages")
    op.drop_constraint("uq_page_blog_slug_locale", "pages", type_="unique")
    op.create_unique_constraint("uq_page_slug_locale", "pages", ["slug", "locale"])

    op.drop_index("ix_pages_blog_id", table_name="pages")
    op.drop_constraint("fk_pages_blog_id_blogs", "pages", type_="foreignkey")
    op.drop_column("pages", "blog_id")

    op.drop_column("blogs", "static_pages_enabled")
