import gzip
import json
from datetime import datetime, timedelta, timezone

import pytest
from sqlalchemy import select, text
from sqlalchemy.ext.asyncio import AsyncSession

from app.core.config import settings
from app.models.audit_archive_run import AuditArchiveRun
from app.models.audit_log import AuditActorType, AuditLog
from app.workers import audit_maintenance as am
from tests.conftest import FakeS3Client


def _utc(days_ago: float) -> datetime:
    return datetime.now(timezone.utc) - timedelta(days=days_ago)


async def _add_event(
    db_session: AsyncSession, *, occurred_at: datetime, action: str = "test.event"
) -> AuditLog:
    ev = AuditLog(
        occurred_at=occurred_at,
        actor_type=AuditActorType.SYSTEM,
        action=action,
        payload={},
    )
    db_session.add(ev)
    await db_session.commit()
    return ev


# ---- calcolo settimane ISO ------------------------------------------------


def test_iso_week_bounds_basic() -> None:
    start, end, label = am.iso_week_bounds(datetime(2026, 9, 5, 12, 0, tzinfo=timezone.utc))
    assert start == datetime(2026, 8, 31, tzinfo=timezone.utc)  # lunedì
    assert end == datetime(2026, 9, 7, tzinfo=timezone.utc)
    assert label == "2026w36"


def test_iso_week_bounds_year_boundary() -> None:
    # 2027-01-01 è un venerdì: settimana ISO 2026-W53, iniziata lunedì 2026-12-28
    start, _end, label = am.iso_week_bounds(datetime(2027, 1, 1, tzinfo=timezone.utc))
    assert label == "2026w53"
    assert start == datetime(2026, 12, 28, tzinfo=timezone.utc)


# ---- prune --------------------------------------------------------------


async def test_prune_deletes_only_old_rows_without_archive(
    db_session: AsyncSession, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(settings, "audit_archive_enabled", False)
    monkeypatch.setattr(settings, "audit_retention_days", 105)
    await _add_event(db_session, occurred_at=_utc(1))
    await _add_event(db_session, occurred_at=_utc(40))
    await _add_event(db_session, occurred_at=_utc(200))

    deleted = await am.prune(db_session)

    assert deleted == 1
    remaining = (await db_session.execute(select(AuditLog.occurred_at))).scalars().all()
    assert len(remaining) == 2


async def test_prune_never_deletes_beyond_archive_watermark(
    db_session: AsyncSession, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(settings, "audit_archive_enabled", True)
    monkeypatch.setattr(settings, "audit_retention_days", 30)
    # ultima settimana archiviata: fino a 100 giorni fa
    db_session.add(
        AuditArchiveRun(
            period_start=_utc(107),
            period_end=_utc(100),
            week_label="wtest",
            object_key=None,
            storage_backend="s3",
            row_count=0,
            byte_size=0,
            sha256=None,
        )
    )
    await db_session.commit()

    await _add_event(db_session, occurred_at=_utc(120))  # prima del watermark -> cancellato
    kept_unarchived = await _add_event(db_session, occurred_at=_utc(60))  # oltre retention ma non archiviato
    kept_recent = await _add_event(db_session, occurred_at=_utc(5))

    deleted = await am.prune(db_session)

    assert deleted == 1
    remaining = set((await db_session.execute(select(AuditLog.id))).scalars().all())
    assert remaining == {kept_unarchived.id, kept_recent.id}


async def test_prune_skipped_when_archive_enabled_but_nothing_archived(
    db_session: AsyncSession, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(settings, "audit_archive_enabled", True)
    monkeypatch.setattr(settings, "audit_retention_days", 30)
    await _add_event(db_session, occurred_at=_utc(500))

    deleted = await am.prune(db_session)

    assert deleted == 0
    assert (await db_session.execute(select(AuditLog.id))).scalars().all()


# ---- archive ----------------------------------------------------------


async def test_archive_writes_gzip_ndjson_and_bookkeeping(
    db_session: AsyncSession, fake_s3: FakeS3Client, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(settings, "audit_archive_enabled", True)
    when = datetime(2026, 8, 12, 10, 0, tzinfo=timezone.utc)  # settimana chiusa
    await _add_event(db_session, occurred_at=when, action="a.one")
    await _add_event(db_session, occurred_at=when + timedelta(hours=2), action="a.two")

    await am.archive(db_session)

    runs = (
        await db_session.execute(select(AuditArchiveRun).order_by(AuditArchiveRun.period_start))
    ).scalars().all()
    with_rows = [r for r in runs if r.row_count > 0]
    assert len(with_rows) == 1
    run = with_rows[0]
    assert run.row_count == 2
    assert run.object_key is not None
    assert run.sha256 is not None
    assert run.storage_backend == settings.storage_backend

    blob = fake_s3.objects[(settings.s3_bucket_audit, run.object_key)]
    lines = gzip.decompress(blob).decode("utf-8").splitlines()
    assert len(lines) == 2
    assert {json.loads(x)["action"] for x in lines} == {"a.one", "a.two"}


async def test_archive_skips_current_open_week(
    db_session: AsyncSession, fake_s3: FakeS3Client, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(settings, "audit_archive_enabled", True)
    await _add_event(db_session, occurred_at=datetime.now(timezone.utc) - timedelta(hours=1))

    summaries = await am.archive(db_session)

    assert summaries == []
    assert (await db_session.execute(select(AuditArchiveRun))).scalars().all() == []


async def test_archive_is_idempotent(
    db_session: AsyncSession, fake_s3: FakeS3Client, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(settings, "audit_archive_enabled", True)
    await _add_event(db_session, occurred_at=datetime(2026, 8, 12, 10, 0, tzinfo=timezone.utc))

    first = await am.archive(db_session)
    second = await am.archive(db_session)

    assert len(first) >= 1
    assert second == []


# ---- restore ---------------------------------------------------------


async def test_restore_roundtrip(
    db_session: AsyncSession, fake_s3: FakeS3Client, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(settings, "audit_archive_enabled", True)
    when = datetime(2026, 8, 12, 10, 0, tzinfo=timezone.utc)
    original = await _add_event(db_session, occurred_at=when, action="r.one")
    await am.archive(db_session)
    label = am.iso_week_bounds(when)[2]

    await db_session.execute(text("DELETE FROM audit_log"))
    await db_session.commit()

    records = am.parse_archive(label)
    inserted = await am.restore_records(db_session, records)
    assert inserted == 1

    # secondo giro: nessun doppione
    assert await am.restore_records(db_session, records) == 0

    rows = (await db_session.execute(select(AuditLog).where(AuditLog.action == "r.one"))).scalars().all()
    assert len(rows) == 1
    assert rows[0].id == original.id
