"""Manutenzione periodica di `audit_log`: scarico su storage delle settimane
ISO chiuse e cancellazione dal database degli eventi oltre la retention.

A differenza degli altri worker non è un consumer di coda: è un job idempotente
da eseguire a intervalli (giornaliero va bene). Ogni giro fa, in ordine:

1. **archive** — per ogni settimana ISO completamente conclusa e non ancora
   archiviata, riversa gli eventi in un oggetto NDJSON gzippato sullo storage
   attivo (S3/localstorage, bucket privato) e registra la cosa in
   `audit_archive_runs`. Disattivabile con `NOCT_AUDIT_ARCHIVE_ENABLED=false`.
2. **prune** — cancella a batch gli eventi con `occurred_at` più vecchio di
   `NOCT_AUDIT_RETENTION_DAYS`, ma **mai** più recenti dell'ultima settimana
   archiviata: il watermark `max(audit_archive_runs.period_end)` è il limite
   duro, così non si perde nulla che non sia già su storage.

Uso (dalla directory backend/, con il venv attivo):
    python -m app.workers.audit_maintenance            # un giro e termina
    python -m app.workers.audit_maintenance --loop     # giro ogni --interval secondi
    python -m app.workers.audit_maintenance --restore 2026w36   # rimette a DB una settimana
"""

import argparse
import asyncio
import gzip
import hashlib
import json
import logging
from datetime import datetime, timedelta, timezone
from typing import Any

from sqlalchemy import func, select, text
from sqlalchemy.dialects.postgresql import insert as pg_insert
from sqlalchemy.ext.asyncio import AsyncSession

from app.core.config import settings
from app.core.database import SessionLocal
from app.core.storage import get_audit_archive, upload_audit_archive
from app.models.audit_archive_run import AuditArchiveRun
from app.models.audit_log import AuditLog

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger("audit_maintenance")

PRUNE_BATCH_SIZE = 5000
# tappo di sicurezza: se il watermark fosse molto indietro, non iterare
# all'infinito costruendo settimane (10 anni di settimane)
MAX_WEEKS_PER_RUN = 520


def _now_utc() -> datetime:
    return datetime.now(timezone.utc)


def iso_week_bounds(dt: datetime) -> tuple[datetime, datetime, str]:
    """(inizio, fine, etichetta) della settimana ISO che contiene `dt`, in
    UTC. L'inizio è il lunedì a mezzanotte, la fine il lunedì successivo
    (intervallo semiaperto), l'etichetta è "AAAAwSS" (anno ISO + settimana
    ISO, es. "2026w36")."""
    d = dt.astimezone(timezone.utc)
    monday = d - timedelta(days=d.weekday())
    start = monday.replace(hour=0, minute=0, second=0, microsecond=0)
    end = start + timedelta(days=7)
    label = f"{start.strftime('%G')}w{start.strftime('%V')}"
    return start, end, label


def _row_to_dict(row: AuditLog) -> dict[str, Any]:
    return {
        "id": str(row.id),
        "occurred_at": row.occurred_at.astimezone(timezone.utc).isoformat(),
        "actor_type": row.actor_type.value,
        "actor_id": str(row.actor_id) if row.actor_id is not None else None,
        "actor_label": row.actor_label,
        "action": row.action,
        "target_type": row.target_type,
        "target_id": str(row.target_id) if row.target_id is not None else None,
        "blog_id": str(row.blog_id) if row.blog_id is not None else None,
        "ip": str(row.ip) if row.ip is not None else None,
        "user_agent": row.user_agent,
        "payload": row.payload,
    }


async def _archive_window_start(session: AsyncSession) -> datetime | None:
    """Da quale lunedì ripartire con l'archiviazione: il watermark se
    esiste, altrimenti l'inizio della settimana del primo evento presente.
    None se non c'è alcun evento da archiviare."""
    watermark = await session.scalar(select(func.max(AuditArchiveRun.period_end)))
    if watermark is not None:
        return watermark.astimezone(timezone.utc)
    earliest = await session.scalar(select(func.min(AuditLog.occurred_at)))
    if earliest is None:
        return None
    return iso_week_bounds(earliest)[0]


async def _archive_one_week(
    session: AsyncSession, start: datetime, end: datetime, label: str
) -> dict[str, Any]:
    result = await session.execute(
        select(AuditLog)
        .where(AuditLog.occurred_at >= start, AuditLog.occurred_at < end)
        .order_by(AuditLog.occurred_at, AuditLog.id)
    )
    rows = list(result.scalars().all())

    object_key: str | None = None
    digest: str | None = None
    byte_size = 0
    if rows:
        payload = "\n".join(json.dumps(_row_to_dict(r), ensure_ascii=False) for r in rows) + "\n"
        blob = gzip.compress(payload.encode("utf-8"))
        digest = hashlib.sha256(blob).hexdigest()
        byte_size = len(blob)
        object_key = upload_audit_archive(week_label=label, content=blob)

    session.add(
        AuditArchiveRun(
            period_start=start,
            period_end=end,
            week_label=label,
            object_key=object_key,
            storage_backend=settings.storage_backend,
            row_count=len(rows),
            byte_size=byte_size,
            sha256=digest,
        )
    )
    await session.commit()
    logger.info("Settimana %s archiviata: %d eventi, %d byte", label, len(rows), byte_size)
    return {"week": label, "rows": len(rows), "bytes": byte_size, "object_key": object_key}


async def archive(session: AsyncSession) -> list[dict[str, Any]]:
    if not settings.audit_archive_enabled:
        return []

    window_start = await _archive_window_start(session)
    if window_start is None:
        return []

    current_week_start = iso_week_bounds(_now_utc())[0]
    summaries: list[dict[str, Any]] = []
    week_start = window_start
    guard = 0
    while week_start < current_week_start and guard < MAX_WEEKS_PER_RUN:
        start, end, label = iso_week_bounds(week_start)
        summaries.append(await _archive_one_week(session, start, end, label))
        week_start = end
        guard += 1
    if guard >= MAX_WEEKS_PER_RUN:
        logger.warning("Raggiunto il tetto di %d settimane per giro, continuo al prossimo", MAX_WEEKS_PER_RUN)
    return summaries


async def prune(session: AsyncSession) -> int:
    if settings.audit_retention_days < 7:
        logger.warning(
            "NOCT_AUDIT_RETENTION_DAYS=%d è meno di una settimana: valore ignorato per la cancellazione",
            settings.audit_retention_days,
        )
        return 0

    cutoff = _now_utc() - timedelta(days=settings.audit_retention_days)
    floor = cutoff

    if settings.audit_archive_enabled:
        watermark = await session.scalar(select(func.max(AuditArchiveRun.period_end)))
        if watermark is None:
            logger.warning("Archiviazione attiva ma nessuna settimana archiviata: cancellazione saltata")
            return 0
        floor = min(cutoff, watermark.astimezone(timezone.utc))

    total = 0
    while True:
        result = await session.execute(
            text(
                "DELETE FROM audit_log WHERE id IN ("
                "  SELECT id FROM audit_log WHERE occurred_at < :floor"
                "  ORDER BY occurred_at LIMIT :batch"
                ")"
            ),
            {"floor": floor, "batch": PRUNE_BATCH_SIZE},
        )
        await session.commit()
        deleted = result.rowcount or 0
        total += deleted
        if deleted < PRUNE_BATCH_SIZE:
            break
    if total:
        logger.info("Cancellati %d eventi di audit più vecchi di %s", total, floor.isoformat())
    return total


def parse_archive(label: str) -> list[dict[str, Any]]:
    """Legge dallo storage l'archivio di una settimana e ne ricostruisce le
    righe pronte per l'insert (timestamp riportato a datetime)."""
    blob = get_audit_archive(week_label=label)
    records: list[dict[str, Any]] = []
    for line in gzip.decompress(blob).decode("utf-8").splitlines():
        if not line.strip():
            continue
        d = json.loads(line)
        d["occurred_at"] = datetime.fromisoformat(d["occurred_at"])
        records.append(d)
    return records


async def restore_records(session: AsyncSession, records: list[dict[str, Any]]) -> int:
    """Reinserisce righe di audit. Idempotente: gli id già presenti a
    database non vengono toccati."""
    if not records:
        return 0
    stmt = (
        pg_insert(AuditLog)
        .on_conflict_do_nothing(index_elements=["id"])
        .returning(AuditLog.id)
    )
    result = await session.execute(stmt, records)
    inserted = len(result.scalars().all())
    await session.commit()
    return inserted


async def restore_week(label: str) -> int:
    """Rimette a database gli eventi di una settimana archiviata."""
    records = parse_archive(label)
    async with SessionLocal() as session:
        inserted = await restore_records(session, records)
    logger.info(
        "Ripristino %s: %d eventi reinseriti (%d già presenti)", label, inserted, len(records) - inserted
    )
    return inserted


async def run_once() -> dict[str, Any]:
    async with SessionLocal() as session:
        archived = await archive(session)
        deleted = await prune(session)
    return {"archived_weeks": archived, "deleted_rows": deleted}


async def _loop_forever(interval: int) -> None:
    while True:
        try:
            await run_once()
        except Exception:
            logger.exception("Giro di manutenzione audit fallito, riprovo al prossimo intervallo")
        await asyncio.sleep(interval)


def main() -> None:
    parser = argparse.ArgumentParser(description="Manutenzione di audit_log (archivio + cancellazione).")
    parser.add_argument("--loop", action="store_true", help="esegue in continuo, un giro ogni --interval secondi")
    parser.add_argument("--interval", type=int, default=86400, help="secondi tra un giro e l'altro in modalità --loop")
    parser.add_argument("--restore", metavar="AAAAwSS", help="reimporta a database una settimana archiviata e termina")
    args = parser.parse_args()

    if args.restore:
        asyncio.run(restore_week(args.restore))
    elif args.loop:
        asyncio.run(_loop_forever(args.interval))
    else:
        asyncio.run(run_once())


if __name__ == "__main__":
    main()
