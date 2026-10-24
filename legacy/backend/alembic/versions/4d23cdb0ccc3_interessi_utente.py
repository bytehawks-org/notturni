"""interessi utente: platform_config.interests + users.interests

Revision ID: 4d23cdb0ccc3
Revises: c9839d73bf05
Create Date: 2026-09-19 13:00:00.000000

"""
from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa
from sqlalchemy.dialects import postgresql


# revision identifiers, used by Alembic.
revision: str = '4d23cdb0ccc3'
down_revision: Union[str, None] = 'c9839d73bf05'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


DEFAULT_INTERESTS = '[{"key": "music", "translations": {"it": "Musica", "en": "Music", "de": "Musik", "fr": "Musique"}}, {"key": "technology", "translations": {"it": "Tecnologia", "en": "Technology", "de": "Technologie", "fr": "Technologie"}}, {"key": "photography", "translations": {"it": "Fotografia", "en": "Photography", "de": "Fotografie", "fr": "Photographie"}}, {"key": "literature", "translations": {"it": "Letteratura", "en": "Literature", "de": "Literatur", "fr": "Litt\\u00e9rature"}}, {"key": "cinema", "translations": {"it": "Cinema", "en": "Cinema", "de": "Kino", "fr": "Cin\\u00e9ma"}}, {"key": "travel", "translations": {"it": "Viaggi", "en": "Travel", "de": "Reisen", "fr": "Voyages"}}, {"key": "food", "translations": {"it": "Cucina", "en": "Food", "de": "Kochen", "fr": "Cuisine"}}, {"key": "art", "translations": {"it": "Arte", "en": "Art", "de": "Kunst", "fr": "Art"}}, {"key": "nature", "translations": {"it": "Natura", "en": "Nature", "de": "Natur", "fr": "Nature"}}, {"key": "science", "translations": {"it": "Scienza", "en": "Science", "de": "Wissenschaft", "fr": "Science"}}, {"key": "sports", "translations": {"it": "Sport", "en": "Sports", "de": "Sport", "fr": "Sport"}}, {"key": "gaming", "translations": {"it": "Videogiochi", "en": "Gaming", "de": "Gaming", "fr": "Jeux vid\\u00e9o"}}, {"key": "fashion", "translations": {"it": "Moda", "en": "Fashion", "de": "Mode", "fr": "Mode"}}, {"key": "politics", "translations": {"it": "Politica", "en": "Politics", "de": "Politik", "fr": "Politique"}}, {"key": "philosophy", "translations": {"it": "Filosofia", "en": "Philosophy", "de": "Philosophie", "fr": "Philosophie"}}, {"key": "history", "translations": {"it": "Storia", "en": "History", "de": "Geschichte", "fr": "Histoire"}}]'


def upgrade() -> None:
    op.add_column(
        'platform_config',
        sa.Column('interests', postgresql.JSONB(astext_type=sa.Text()), nullable=False, server_default=sa.text("'[]'::jsonb")),
    )
    op.alter_column('platform_config', 'interests', server_default=None)
    # Backfill dei builtin (app/domain/interests.py::DEFAULT_INTERESTS,
    # snapshot congelato qui come da convenzione delle migrazioni dati) sulla
    # riga già esistente — senza, un'installazione che aggiorna resterebbe
    # con un elenco vuoto finché un Super Admin non lo popola a mano: solo
    # una nuova riga (installazione nuova) passa dal seed applicativo in
    # app/domain/platform_config.py::get_platform_config.
    op.execute(
        sa.text("UPDATE platform_config SET interests = CAST(:interests AS jsonb) WHERE interests = '[]'::jsonb").bindparams(
            interests=DEFAULT_INTERESTS
        )
    )
    op.add_column(
        'users',
        sa.Column('interests', sa.ARRAY(sa.String(length=40)), nullable=False, server_default='{}'),
    )
    op.alter_column('users', 'interests', server_default=None)


def downgrade() -> None:
    op.drop_column('users', 'interests')
    op.drop_column('platform_config', 'interests')
