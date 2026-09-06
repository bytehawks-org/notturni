"""Permalink leggibili per i post, senza UUID nell'URL pubblico.

Formato: /{blog_slug}/{post_slug}. Unicità garantita a livello di dominio da
(blog_id, slug, locale) su Post (vedi app/models/post.py) — non serve altro
nell'URL per disambiguare. Non sostituisce l'UUID come chiave primaria, che
resta invariata (CLAUDE.md #1).
"""

from app.models.post import Post

# Segmenti statici già usati sotto /{blog_slug}/... dal frontend (pagina di
# bibliografia/media/link del blog, elenco pagine statiche) — tolta la data
# dal permalink, uno slug di post identico a uno di questi resterebbe
# irraggiungibile (il segmento statico vince sempre su quello dinamico).
RESERVED_POST_SLUGS = {"bibliografia", "link", "media", "pagina"}


def build_permalink(blog_slug: str, post: Post) -> str:
    return f"/{blog_slug}/{post.slug}"


def validate_post_slug_not_reserved(slug: str) -> None:
    if slug in RESERVED_POST_SLUGS:
        raise ValueError(
            f"'{slug}' è riservato dal frontend (bibliografia/media/link/pagina) e non può essere "
            "usato come slug di un post."
        )
