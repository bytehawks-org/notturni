"""Markdown → HTML sicuro per le email di campagna (newsletter, blocco
"editor newsletter"): l'unico punto del backend che renderizza Markdown a
HTML — i post lo fanno solo lato frontend (frontend/src/lib/markdown.ts),
qui serve perché l'email va costruita nel worker, non nel browser di chi la
riceve. `nh3.clean` sanifica l'HTML prodotto da `markdown` (stessa cautela
di app/domain/notes.py sugli URL: mai fidarsi di uno schema arbitrario in
un `href`/`src`, qui in più niente `<script>`/attributi `on*` nel caso il
sorgente Markdown contenga HTML incorporato, che `markdown` lascia passare
invariato)."""

import markdown as _markdown
import nh3

_ALLOWED_TAGS = {
    "p", "br", "hr",
    "strong", "em", "s", "code", "pre",
    "a",
    "ul", "ol", "li",
    "blockquote",
    "h1", "h2", "h3", "h4", "h5", "h6",
    "table", "thead", "tbody", "tr", "th", "td",
    "img",
}
_ALLOWED_ATTRIBUTES = {
    "a": {"href", "title"},
    "img": {"src", "alt", "title"},
}
_ALLOWED_URL_SCHEMES = {"http", "https", "mailto"}


def render_markdown_to_safe_html(text: str) -> str:
    html = _markdown.markdown(text, extensions=["tables", "sane_lists"])
    return nh3.clean(
        html,
        tags=_ALLOWED_TAGS,
        attributes=_ALLOWED_ATTRIBUTES,
        url_schemes=_ALLOWED_URL_SCHEMES,
        link_rel="noopener noreferrer",
    )
