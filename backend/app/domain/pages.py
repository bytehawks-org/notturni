"""Pagine statiche: slug e permalink, sia per il sito principale (CLAUDE.md
#1, `Page.blog_id` NULL) sia per un blog (feature opt-in, vedi
`Blog.static_pages_enabled` in app/models/blog.py)."""

import re

from app.models.page import Page

_SLUG_RE = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*$")
MAX_SLUG_LENGTH = 80


def validate_page_slug(slug: str) -> None:
    if not slug or len(slug) > MAX_SLUG_LENGTH or not _SLUG_RE.fullmatch(slug):
        raise ValueError(
            f"Lo slug della pagina deve contenere solo lettere minuscole, cifre e "
            f"trattini singoli, max {MAX_SLUG_LENGTH} caratteri."
        )


def build_page_permalink(blog_slug: str, page: Page) -> str:
    """Permalink pubblico di una pagina di blog: niente data, a differenza dei
    post (le pagine statiche non sono cronologiche) — vedi app/domain/permalinks.py."""
    return f"/{blog_slug}/pagina/{page.slug}"


def build_platform_page_permalink(page: Page) -> str:
    """Permalink pubblico di una pagina di piattaforma: prefisso dedicato
    `/pages/` per non collidere con gli slug dei blog raggiungibili senza
    sottodominio su `/{blog_slug}/...` (vedi app/domain/blog_rules.py)."""
    return f"/pages/{page.slug}"
