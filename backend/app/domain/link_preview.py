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

Cache a due livelli (todo/UX_REDESIGN.md, blocco "footer"/link preview):
Redis come cache calda (TTL breve, `REDIS_TTL_SECONDS`), `link_preview_cache`
come fonte persistente e deduplicata — una riga per URL (unique su
`url_hash`), condivisa da qualunque post/utente citi lo stesso link, non
rifatta a ogni rendering. Un'anteprima riuscita resta valida più a lungo di
un fallimento (`STALE_AFTER_OK`/`STALE_AFTER_EMPTY`) prima di essere
ricontrollata dal vivo — vedi `get_cached_or_fetch_link_preview`."""

import hashlib
import ipaddress
import json
import logging
import re
import socket
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from urllib.parse import urlparse, urlunparse

import httpx
from sqlalchemy import select
from sqlalchemy.dialects.postgresql import insert as pg_insert
from sqlalchemy.ext.asyncio import AsyncSession

from app.core.redis import get_redis
from app.models.link_preview import LinkPreviewCache

logger = logging.getLogger(__name__)

_TIMEOUT = httpx.Timeout(8.0, connect=5.0)
# Alcuni siti (es. YouTube) mettono un'enorme quantità di dati inline prima
# dei tag Open Graph nell'<head> — 512 KB non bastavano nemmeno ad arrivarci.
# Ci si ferma comunque appena si vede la chiusura di </head> (vedi sotto),
# questo è solo il tetto di sicurezza per pagine senza un head chiuso.
_MAX_BYTES = 3 * 1024 * 1024
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


def _resolve_pinned_ip(hostname: str) -> str | None:
    """Come `_resolve_is_safe`, ma ritorna l'indirizzo risolto invece di un
    booleano: usato per il *pinning* IP di `fetch_link_preview` — connettersi
    direttamente all'IP verificato invece di lasciare che sia httpx a
    ri-risolvere l'hostname al momento della richiesta chiude la finestra di
    DNS rebinding fra le due risoluzioni (un DNS malevolo può rispondere con
    un IP pubblico alla prima verifica e uno privato alla seconda) — non solo
    teorica: è quanto la query CodeQL `py/full-ssrf` segnala, perché l'URL
    che raggiunge il client HTTP resta comunque quello fornito dall'utente,
    a prescindere dal controllo fatto prima con `_resolve_is_safe`."""
    try:
        infos = socket.getaddrinfo(hostname, None)
    except OSError:
        return None
    for info in infos:
        ip = info[4][0]
        if not _is_blocked_ip(ip):
            return ip
    return None


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
    parsed = urlparse(url)

    # Pinning IP (vedi _resolve_pinned_ip): la richiesta va all'indirizzo
    # verificato qui, non a quello che httpx risolverebbe da sé passando
    # l'URL originale — mai un secondo giro di DNS fuori dal nostro controllo
    # fra la verifica e la connessione effettiva.
    pinned_ip = _resolve_pinned_ip(parsed.hostname)
    if pinned_ip is None:
        return LinkPreview(url=url)
    netloc = f"[{pinned_ip}]" if ":" in pinned_ip else pinned_ip
    if parsed.port:
        netloc += f":{parsed.port}"
    pinned_url = urlunparse(parsed._replace(netloc=netloc))

    try:
        async with httpx.AsyncClient(follow_redirects=False, timeout=_TIMEOUT) as client:
            async with client.stream(
                "GET",
                pinned_url,
                headers={"User-Agent": "NotturniLinkPreview/1.0", "Host": parsed.hostname},
                extensions={"sni_hostname": parsed.hostname},
            ) as resp:
                if resp.status_code >= 400:
                    return LinkPreview(url=url)
                content_type = resp.headers.get("content-type", "")
                if "text/html" not in content_type:
                    return LinkPreview(url=url)
                chunks = bytearray()
                async for chunk in resp.aiter_bytes():
                    chunks.extend(chunk)
                    if len(chunks) >= _MAX_BYTES or b"</head" in chunks[-len(chunk) - 8 :].lower():
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


# ---------------------------------------------------------------------------
# Cache (Redis calda + `link_preview_cache` persistente/deduplicata)
# ---------------------------------------------------------------------------

REDIS_TTL_SECONDS = 6 * 3600
# Un'anteprima con dati Open Graph reali resta valida una settimana prima di
# essere riverificata dal vivo; un fallimento (host irraggiungibile, nessun
# meta tag, ...) molto meno, per non restare bloccati su un URL momentaneamente
# giù più del necessario, ma comunque senza martellarlo a ogni richiesta.
STALE_AFTER_OK = timedelta(days=7)
STALE_AFTER_EMPTY = timedelta(hours=6)


def _url_hash(url: str) -> str:
    return hashlib.sha256(url.strip().encode("utf-8")).hexdigest()


def _cache_key(url_hash: str) -> str:
    return f"link-preview:{url_hash}"


async def _read_hot_cache(url_hash: str) -> LinkPreview | None:
    try:
        raw = await get_redis().get(_cache_key(url_hash))
    except Exception:
        logger.warning("Redis non raggiungibile per l'anteprima link: si prosegue senza cache calda.", exc_info=True)
        return None
    if raw is None:
        return None
    data = json.loads(raw)
    return LinkPreview(url=data["url"], title=data.get("title"), description=data.get("description"), image=data.get("image"))


async def _write_hot_cache(url_hash: str, preview: LinkPreview) -> None:
    try:
        await get_redis().set(
            _cache_key(url_hash),
            json.dumps({"url": preview.url, "title": preview.title, "description": preview.description, "image": preview.image}),
            ex=REDIS_TTL_SECONDS,
        )
    except Exception:
        logger.warning("Redis non raggiungibile: anteprima link non messa in cache calda.", exc_info=True)


async def get_cached_or_fetch_link_preview(session: AsyncSession, url: str) -> LinkPreview:
    """Redis (cache calda) → `link_preview_cache` (persistente, deduplicata
    per URL) → fetch dal vivo solo se assente o scaduta. Chi chiama deve aver
    già validato l'URL (`validate_previewable_url`) — qui non si rifà, sarebbe
    ridondante sia sul percorso cache sia su quello di fetch (che la richiama
    comunque)."""
    url_hash = _url_hash(url)

    hot = await _read_hot_cache(url_hash)
    if hot is not None:
        return hot

    row = (
        await session.execute(select(LinkPreviewCache).where(LinkPreviewCache.url_hash == url_hash))
    ).scalar_one_or_none()
    now = datetime.now(timezone.utc)
    if row is not None:
        stale_after = STALE_AFTER_OK if row.fetch_ok else STALE_AFTER_EMPTY
        if now - row.updated_at < stale_after:
            preview = LinkPreview(url=url, title=row.title, description=row.description, image=row.image)
            await _write_hot_cache(url_hash, preview)
            return preview

    preview = await fetch_link_preview(url)
    fetch_ok = bool(preview.title or preview.description or preview.image)

    # upsert: una riga per URL, condivisa da chiunque lo citi — l'unique
    # constraint su url_hash è la deduplica reale, questo è solo il modo di
    # scriverla senza un giro SELECT-poi-INSERT/UPDATE separato.
    stmt = (
        pg_insert(LinkPreviewCache)
        .values(url_hash=url_hash, url=url, title=preview.title, description=preview.description, image=preview.image, fetch_ok=fetch_ok)
        .on_conflict_do_update(
            index_elements=["url_hash"],
            set_={
                "title": preview.title,
                "description": preview.description,
                "image": preview.image,
                "fetch_ok": fetch_ok,
                "updated_at": now,
            },
        )
    )
    await session.execute(stmt)
    await session.commit()

    await _write_hot_cache(url_hash, preview)
    return preview
