"""Cache (Redis + link_preview_cache) dell'anteprima Open Graph di un link,
condivisa e deduplicata tra chiunque citi lo stesso URL."""

from collections.abc import Callable
from datetime import datetime, timedelta, timezone

import pytest
from httpx import AsyncClient
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.domain.link_preview import LinkPreview
from app.models.link_preview import LinkPreviewCache
from tests.conftest import AuthedUser


@pytest.fixture(autouse=True)
def _no_real_dns(monkeypatch: pytest.MonkeyPatch) -> None:
    """`validate_previewable_url` risolve davvero l'host per escludere IP
    privati/riservati (SSRF, vedi app/domain/link_preview.py) — qui il fetch
    stesso è comunque mockato (`_fake_fetch`), quindi una risoluzione DNS
    reale servirebbe solo a rendere il test fragile in un ambiente senza
    rete in uscita, senza validare nulla in più."""
    monkeypatch.setattr("app.domain.link_preview._resolve_is_safe", lambda hostname: True)


def _fake_fetch(calls: list[str], *, title: str = "Titolo", ok: bool = True):
    async def _fetch(url: str) -> LinkPreview:
        calls.append(url)
        if not ok:
            return LinkPreview(url=url)
        return LinkPreview(url=url, title=title, description="Descrizione", image="https://example.com/img.png")

    return _fetch


async def test_second_request_for_same_url_does_not_refetch(
    client: AsyncClient, monkeypatch: pytest.MonkeyPatch, db_session: AsyncSession
) -> None:
    calls: list[str] = []
    monkeypatch.setattr("app.domain.link_preview.fetch_link_preview", _fake_fetch(calls))

    url = "https://example.com/article-a"
    first = await client.get("/api/v1/link-preview", params={"url": url})
    assert first.status_code == 200
    assert first.json()["title"] == "Titolo"
    assert calls == [url]

    second = await client.get("/api/v1/link-preview", params={"url": url})
    assert second.status_code == 200
    assert second.json() == first.json()
    # nessun secondo fetch: servito da Redis/DB
    assert calls == [url]

    rows = (await db_session.execute(select(LinkPreviewCache).where(LinkPreviewCache.url == url))).scalars().all()
    assert len(rows) == 1
    assert rows[0].fetch_ok is True


async def test_two_users_sharing_the_same_link_dedupe_to_one_row(
    client: AsyncClient, make_user: Callable, monkeypatch: pytest.MonkeyPatch, db_session: AsyncSession
) -> None:
    calls: list[str] = []
    monkeypatch.setattr("app.domain.link_preview.fetch_link_preview", _fake_fetch(calls))

    url = "https://example.com/shared-link"
    _u1: AuthedUser = await make_user("lp-user-1")
    _u2: AuthedUser = await make_user("lp-user-2")

    res1 = await client.get("/api/v1/link-preview", params={"url": url})
    res2 = await client.get("/api/v1/link-preview", params={"url": url})
    assert res1.status_code == res2.status_code == 200
    assert calls == [url]  # un solo fetch reale, la seconda "utente" ha riusato la cache

    rows = (await db_session.execute(select(LinkPreviewCache).where(LinkPreviewCache.url == url))).scalars().all()
    assert len(rows) == 1


async def test_stale_successful_preview_is_refetched(
    client: AsyncClient, monkeypatch: pytest.MonkeyPatch, db_session: AsyncSession
) -> None:
    calls: list[str] = []
    monkeypatch.setattr("app.domain.link_preview.fetch_link_preview", _fake_fetch(calls, title="Nuovo titolo"))

    url = "https://example.com/stale-ok"
    from app.domain.link_preview import _url_hash

    db_session.add(
        LinkPreviewCache(
            url_hash=_url_hash(url),
            url=url,
            title="Vecchio titolo",
            description="Vecchia descrizione",
            image=None,
            fetch_ok=True,
        )
    )
    await db_session.commit()
    # retrodata updated_at oltre STALE_AFTER_OK (7 giorni)
    await db_session.execute(
        LinkPreviewCache.__table__.update()
        .where(LinkPreviewCache.url_hash == _url_hash(url))
        .values(updated_at=datetime.now(timezone.utc) - timedelta(days=8))
    )
    await db_session.commit()

    res = await client.get("/api/v1/link-preview", params={"url": url})
    assert res.status_code == 200
    assert res.json()["title"] == "Nuovo titolo"
    assert calls == [url]  # rifatto perché scaduto


async def test_failed_fetch_is_cached_but_retried_sooner(
    client: AsyncClient, monkeypatch: pytest.MonkeyPatch, db_session: AsyncSession
) -> None:
    calls: list[str] = []
    monkeypatch.setattr("app.domain.link_preview.fetch_link_preview", _fake_fetch(calls, ok=False))

    url = "https://example.com/unreachable"
    res = await client.get("/api/v1/link-preview", params={"url": url})
    assert res.status_code == 200
    assert res.json()["title"] is None
    assert calls == [url]

    row = (await db_session.execute(select(LinkPreviewCache).where(LinkPreviewCache.url == url))).scalar_one()
    assert row.fetch_ok is False
