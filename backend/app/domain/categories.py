"""Categorie di un blog: tassonomia definita dal proprietario/autori, a
differenza dei tag (liberi, vedi app/domain/tags.py) un post appartiene al
più a UNA categoria — pensata come classificazione principale dei
contenuti (CLAUDE.md), non descrittiva."""

import re

_SLUG_RE = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*$")
MAX_SLUG_LENGTH = 60
MAX_NAME_LENGTH = 50


def validate_category_slug(slug: str) -> None:
    if not slug or len(slug) > MAX_SLUG_LENGTH or not _SLUG_RE.fullmatch(slug):
        raise ValueError(
            f"Lo slug della categoria deve contenere solo lettere minuscole, cifre e "
            f"trattini singoli, max {MAX_SLUG_LENGTH} caratteri."
        )


def validate_category_name(name: str) -> None:
    if not name.strip() or len(name) > MAX_NAME_LENGTH:
        raise ValueError(f"Il nome della categoria deve essere non vuoto, max {MAX_NAME_LENGTH} caratteri.")
