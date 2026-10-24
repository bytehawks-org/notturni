"""indici su relazioni calde del feed

Revision ID: b1c2d3e4f5a6
Revises: 5c3f9a71e0d2
Create Date: 2026-09-05 00:00:00.000000

Analisi del branch feat/code-optimization (ROADMAP.md sezione 6): diverse
foreign key interrogate a ogni richiesta pubblica erano senza indice. Qui si
aggiungono, oltre all'indice composito (status, published_at) usato
dall'ordinamento del feed della homepage e delle tendenze.

`posts.blog_id` NON viene indicizzata: è già la colonna di testa dello
UniqueConstraint (blog_id, slug, locale), che Postgres usa anche per i soli
lookup per blog_id. Idem per `*.follower_id` sulle tabelle di follow.
"""
from typing import Sequence, Union

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "b1c2d3e4f5a6"
down_revision: Union[str, None] = "5c3f9a71e0d2"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.create_index("ix_comments_post_id", "comments", ["post_id"])
    op.create_index(
        "ix_user_follows_followed_user_id", "user_follows", ["followed_user_id"]
    )
    op.create_index("ix_blog_follows_blog_id", "blog_follows", ["blog_id"])
    op.create_index("ix_posts_author_id", "posts", ["author_id"])
    op.create_index("ix_posts_category_id", "posts", ["category_id"])
    op.create_index(
        "ix_posts_status_published_at", "posts", ["status", "published_at"]
    )


def downgrade() -> None:
    op.drop_index("ix_posts_status_published_at", table_name="posts")
    op.drop_index("ix_posts_category_id", table_name="posts")
    op.drop_index("ix_posts_author_id", table_name="posts")
    op.drop_index("ix_blog_follows_blog_id", table_name="blog_follows")
    op.drop_index("ix_user_follows_followed_user_id", table_name="user_follows")
    op.drop_index("ix_comments_post_id", table_name="comments")
