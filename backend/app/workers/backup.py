"""Backup infrastrutturale periodico di Postgres e MinIO/S3 verso uno storage
S3 **esterno** dedicato (ROADMAP.md §3), distinto dal backup applicativo dei
singoli post già gestito da `worker-post-backup` (quello scrive nello stesso
bucket applicativo, pensato come fallback rapido del Markdown — questo è un
backup infrastrutturale dell'intero database e di tutti gli oggetti S3, verso
una destinazione che in produzione deve poter essere un provider diverso da
quello che ospita i contenuti).

Come `audit_maintenance.py`: non è un consumer di coda, è un job idempotente
da eseguire a intervalli. `NOCT_BACKUP_S3_BUCKET` assente disattiva l'intero
worker (nessun tentativo di scrivere su un bucket non configurato).

Uso (dalla directory backend/, con il venv attivo, richiede `pg_dump` nel
PATH — incluso nell'immagine Docker):
    python -m app.workers.backup            # un giro e termina
    python -m app.workers.backup --loop     # giro ogni --interval secondi
"""

import argparse
import logging
import os
import subprocess
import time
from datetime import datetime, timezone
from typing import Any

from app.core.config import settings
from app.core.storage import build_s3_client, get_s3_client

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger("backup")

# I tre bucket applicativi S3-compatible da rispecchiare (vedi
# app/core/storage.py): contenuti/media dei post, avatar, archivi di audit.
MIRRORED_BUCKETS = ("s3_bucket_content", "s3_bucket_avatars", "s3_bucket_audit")


def get_backup_s3_client():
    return build_s3_client(
        endpoint_url=settings.backup_s3_endpoint_url,
        region=settings.backup_s3_region,
        access_key_id=settings.backup_s3_access_key_id,
        secret_access_key=settings.backup_s3_secret_access_key,
    )


def _ensure_backup_bucket(client) -> None:
    """Best-effort: alcuni provider S3 esterni non concedono CreateBucket a
    credenziali con privilegi minimi — un fallimento qui non deve bloccare il
    backup, semmai lo farà fallire più avanti con un errore più esplicito sul
    primo put_object se il bucket davvero non esiste."""
    try:
        client.head_bucket(Bucket=settings.backup_s3_bucket)
    except Exception:
        try:
            client.create_bucket(Bucket=settings.backup_s3_bucket)
        except Exception:
            logger.warning(
                "Impossibile verificare/creare il bucket di backup %s: proseguo comunque.",
                settings.backup_s3_bucket,
                exc_info=True,
            )


def _timestamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def dump_postgres() -> bytes:
    """`pg_dump` in formato custom (già compresso da pg_dump stesso, non
    serve gzippare a parte) del database applicativo."""
    env = {**os.environ, "PGPASSWORD": settings.postgres_password}
    result = subprocess.run(
        [
            "pg_dump",
            "--host",
            settings.postgres_host,
            "--port",
            str(settings.postgres_port),
            "--username",
            settings.postgres_user,
            "--format",
            "custom",
            settings.postgres_db,
        ],
        env=env,
        capture_output=True,
        check=True,
    )
    return result.stdout


def backup_postgres(backup_client) -> str:
    blob = dump_postgres()
    key = f"postgres/{settings.postgres_db}-{_timestamp()}.dump"
    backup_client.put_object(Bucket=settings.backup_s3_bucket, Key=key, Body=blob)
    logger.info("Backup Postgres scritto: %s (%d byte)", key, len(blob))
    return key


def _mirror_bucket(source_client, backup_client, bucket: str) -> int:
    count = 0
    paginator = source_client.get_paginator("list_objects_v2")
    try:
        for page in paginator.paginate(Bucket=bucket):
            for obj in page.get("Contents", []):
                key = obj["Key"]
                body = source_client.get_object(Bucket=bucket, Key=key)["Body"].read()
                backup_client.put_object(Bucket=settings.backup_s3_bucket, Key=f"minio/{bucket}/{key}", Body=body)
                count += 1
    except source_client.exceptions.ClientError as exc:
        if exc.response.get("Error", {}).get("Code") in {"NoSuchBucket", "404"}:
            logger.info("Bucket applicativo %s non ancora creato, nessun oggetto da copiare.", bucket)
        else:
            raise
    logger.info("Bucket %s rispecchiato: %d oggetti", bucket, count)
    return count


def mirror_minio(backup_client) -> dict[str, int]:
    if settings.storage_backend != "s3":
        logger.info(
            "NOCT_STORAGE_BACKEND=%s: nessun MinIO/S3 applicativo da rispecchiare, salto.",
            settings.storage_backend,
        )
        return {}
    source_client = get_s3_client()
    return {
        getattr(settings, attr): _mirror_bucket(source_client, backup_client, getattr(settings, attr))
        for attr in MIRRORED_BUCKETS
    }


def run_once() -> dict[str, Any]:
    if not settings.backup_s3_bucket:
        logger.info("NOCT_BACKUP_S3_BUCKET non configurato: backup infrastrutturale disattivato.")
        return {}

    backup_client = get_backup_s3_client()
    _ensure_backup_bucket(backup_client)
    postgres_key = backup_postgres(backup_client)
    mirrored = mirror_minio(backup_client)
    return {"postgres_key": postgres_key, "mirrored_objects": mirrored}


def _loop_forever(interval: int) -> None:
    while True:
        try:
            run_once()
        except Exception:
            logger.exception("Giro di backup fallito, riprovo al prossimo intervallo")
        time.sleep(interval)


def main() -> None:
    parser = argparse.ArgumentParser(description="Backup infrastrutturale di Postgres e MinIO/S3 verso S3 esterno.")
    parser.add_argument("--loop", action="store_true", help="esegue in continuo, un giro ogni --interval secondi")
    parser.add_argument("--interval", type=int, default=86400, help="secondi tra un giro e l'altro in modalità --loop")
    args = parser.parse_args()

    if args.loop:
        _loop_forever(args.interval)
    else:
        run_once()


if __name__ == "__main__":
    main()
