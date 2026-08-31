"""Permalink leggibili per i post, senza UUID nell'URL pubblico.

Formato: /{blog_slug}/{YYYYMMDD}/{post_slug} (stile WordPress). La data è
solo un elemento di disambiguazione/leggibilità nell'URL — l'unicità reale
è già garantita a livello di dominio da (blog_id, slug, locale) su Post
(vedi app/models/post.py). Non sostituisce l'UUID come chiave primaria,
che resta invariata (CLAUDE.md #1).
"""

import re
from datetime import date

from app.models.post import Post

PERMALINK_DATE_FORMAT = "%Y%m%d"
_DATE_RE = re.compile(r"^\d{8}$")


def permalink_date(post: Post) -> date:
    """Data usata nel permalink: quella di pubblicazione se pubblicato,
    altrimenti quella di creazione (permette comunque un link di anteprima
    per una bozza, visibile solo a chi ha accesso in scrittura al blog)."""
    if post.published_at is not None:
        return post.published_at.date()
    return post.created_at.date()


def build_permalink(blog_slug: str, post: Post) -> str:
    d = permalink_date(post)
    return f"/{blog_slug}/{d.strftime(PERMALINK_DATE_FORMAT)}/{post.slug}"


def is_valid_permalink_date(value: str) -> bool:
    return bool(_DATE_RE.fullmatch(value))
