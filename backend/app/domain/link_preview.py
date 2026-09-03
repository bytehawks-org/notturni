"""Anteprima di un link incollato nell'editor (stile Bluesky, CLAUDE.md #1):
recupera titolo/descrizione/immagine Open Graph di una pagina esterna.

Endpoint pubblico e senza autenticazione (serve anche al rendering della
pagina pubblica del post, non solo all'editor) — per questo il fetch è
volutamente rigido contro SSRF: solo http/https, redirect non seguiti in
automatico, IP risolto verificato contro gli intervalli privati/riservati
prima della richiesta, timeout stretto, corpo troncato. Non è pensato come
barriera di sicurezza assoluta (un DNS rebinding fra la verifica e la
richiesta resta teoricamente possibile, stessa complessità accettata altrove
nel progetto — vedi CLAUDE.md #4 sulla moderazione automatica) ma alza
comunque il costo di un attacco banale verso indirizzi interni.

Nessuna cache: ogni chiamata rifà il fetch. Buon candidato per Redis quando
verrà usato per la prima volta nel progetto (oggi deployato ma non ancora
sfruttato — vedi ROADMAP.md)."""

import ipaddress
import re
import socket
from dataclasses import dataclass
from urllib.parse import urlparse

import httpx

_TIMEOUT = httpx.Timeout(5.0, connect=5.0)
_MAX_BYTES = 512 * 1024
_ALLOWED_SCHEMES = {"http", "https"}


@dataclass
class LinkPreview:
    url: str
    title: str | None = None
    description: str | None = None
    image: str | None = None


def _is_blocked_ip(ip_str: str) -> bool:
    ip = ipaddress.ip_address(ip_str)
    return (
        ip.is_private
        or ip.is_loopback
        or ip.is_link_local
        or ip.is_multicast
        or ip.is_reserved
        or ip.is_unspecified
    )


def _resolve_is_safe(hostname: str) -> bool:
    try:
        infos = socket.getaddrinfo(hostname, None)
    except OSError:
        return False
    if not infos:
        return False
    return all(not _is_blocked_ip(info[4][0]) for info in infos)


def validate_previewable_url(url: str) -> str:
    """Solleva ValueError se l'URL non è idoneo (schema non http/https,
    hostname mancante, o che risolve a un indirizzo privato/riservato)."""
    parsed = urlparse(url)
    if parsed.scheme not in _ALLOWED_SCHEMES or not parsed.hostname:
        raise ValueError("URL non valido: solo http/https con un host esplicito.")
    if not _resolve_is_safe(parsed.hostname):
        raise ValueError("URL non raggiungibile per l'anteprima.")
    return url


_META_RE = re.compile(
    r'<meta\s+[^>]*?(?:property|name)=["\'](og:title|og:description|og:image|description)["\'][^>]*?'
    r'content=["\']([^"\']*)["\'][^>]*?/?>',
    re.IGNORECASE,
)
_META_RE_REVERSED = re.compile(
    r'<meta\s+[^>]*?content=["\']([^"\']*)["\'][^>]*?(?:property|name)=["\'](og:title|og:description|og:image|description)["\'][^>]*?/?>',
    re.IGNORECASE,
)
_TITLE_RE = re.compile(r"<title[^>]*>([^<]*)</title>", re.IGNORECASE)


def _parse_meta(html: str) -> dict[str, str]:
    values: dict[str, str] = {}
    for key, value in _META_RE.findall(html):
        values.setdefault(key.lower(), value)
    for value, key in _META_RE_REVERSED.findall(html):
        values.setdefault(key.lower(), value)
    return values


async def fetch_link_preview(url: str) -> LinkPreview:
    """Non solleva mai per problemi di rete/parsing (URL irraggiungibile,
    HTML malformato, ecc.): ritorna un LinkPreview con solo `url` valorizzato,
    così chi chiama può sempre ripiegare su un link semplice invece che far
    fallire il salvataggio del post. Solleva ValueError solo per un URL non
    idoneo in partenza (vedi validate_previewable_url)."""
    validate_previewable_url(url)

    try:
        async with httpx.AsyncClient(follow_redirects=False, timeout=_TIMEOUT) as client:
            async with client.stream("GET", url, headers={"User-Agent": "NotturniLinkPreview/1.0"}) as resp:
                if resp.status_code >= 400:
                    return LinkPreview(url=url)
                content_type = resp.headers.get("content-type", "")
                if "text/html" not in content_type:
                    return LinkPreview(url=url)
                chunks = bytearray()
                async for chunk in resp.aiter_bytes():
                    chunks.extend(chunk)
                    if len(chunks) >= _MAX_BYTES:
                        break
                html = chunks.decode("utf-8", errors="ignore")
    except (httpx.HTTPError, OSError):
        return LinkPreview(url=url)

    meta = _parse_meta(html)
    title = meta.get("og:title")
    if not title:
        title_match = _TITLE_RE.search(html)
        title = title_match.group(1).strip() if title_match else None
    description = meta.get("og:description") or meta.get("description")
    image = meta.get("og:image")

    return LinkPreview(url=url, title=title, description=description, image=image)
