import re
import uuid

from sqlalchemy import func, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.blog import Blog

MAX_BLOGS_PER_USER = 5
MIN_SLUG_LENGTH = 4
# todo/BLOG.md #1
MAX_SUBTITLE_LENGTH = 64
MAX_DESCRIPTION_LENGTH = 256

# CLAUDE.md #2: nomi <= 3 caratteri riservati alla piattaforma (futura funzionalità premium),
# più le parole riservate ai sottodomini/servizi della piattaforma stessa.
RESERVED_BLOG_SLUGS = {
    "blog",
    "www",
    "mail",
    "journal",
    "api",
    "admin",
    "monitor",
    "stats",
    "status",
}

SLUG_PATTERN = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*$")


def validate_blog_slug(slug: str) -> None:
    if len(slug) < MIN_SLUG_LENGTH:
        raise ValueError(f"Lo slug del blog deve avere almeno {MIN_SLUG_LENGTH} caratteri.")
    if not SLUG_PATTERN.fullmatch(slug):
        raise ValueError("Lo slug può contenere solo lettere minuscole, numeri e trattini.")
    if slug in RESERVED_BLOG_SLUGS:
        raise ValueError(f"'{slug}' è un nome riservato alla piattaforma.")


def validate_blog_subtitle(subtitle: str) -> None:
    if len(subtitle) > MAX_SUBTITLE_LENGTH:
        raise ValueError(f"Il sottotitolo può avere al massimo {MAX_SUBTITLE_LENGTH} caratteri.")


def validate_blog_description(description: str) -> None:
    if len(description) > MAX_DESCRIPTION_LENGTH:
        raise ValueError(
            f"La descrizione può avere al massimo {MAX_DESCRIPTION_LENGTH} caratteri."
        )


async def assert_can_create_blog(session: AsyncSession, owner_id: uuid.UUID) -> None:
    count = await session.scalar(select(func.count()).select_from(Blog).where(Blog.owner_id == owner_id))
    if count is not None and count >= MAX_BLOGS_PER_USER:
        raise ValueError(f"Limite massimo di {MAX_BLOGS_PER_USER} blog per utente raggiunto.")
