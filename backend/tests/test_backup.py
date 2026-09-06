import subprocess

import pytest

from app.core.config import settings
from app.workers import backup
from tests.conftest import FakeS3Client


def test_run_once_disabled_without_bucket(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(settings, "backup_s3_bucket", None)
    assert backup.run_once() == {}


def test_dump_postgres_invokes_pg_dump(monkeypatch: pytest.MonkeyPatch) -> None:
    captured: dict = {}

    def _fake_run(args, env, capture_output, check):
        captured["args"] = args
        captured["env"] = env
        return subprocess.CompletedProcess(args, 0, stdout=b"dumpdata", stderr=b"")

    monkeypatch.setattr(backup.subprocess, "run", _fake_run)

    result = backup.dump_postgres()

    assert result == b"dumpdata"
    args = captured["args"]
    assert args[0] == "pg_dump"
    assert settings.postgres_host in args
    assert settings.postgres_db in args
    assert "--format" in args and "custom" in args
    assert captured["env"]["PGPASSWORD"] == settings.postgres_password


def test_backup_postgres_uploads_dump(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(settings, "backup_s3_bucket", "notturni-backups-test")
    monkeypatch.setattr(backup, "dump_postgres", lambda: b"contenuto-dump")

    backup_client = FakeS3Client()
    key = backup.backup_postgres(backup_client)

    assert key.startswith("postgres/")
    assert backup_client.objects[("notturni-backups-test", key)] == b"contenuto-dump"


def test_mirror_minio_skips_when_localstorage(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(settings, "storage_backend", "localstorage")
    assert backup.mirror_minio(FakeS3Client()) == {}


def test_mirror_minio_copies_objects_between_buckets(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(settings, "storage_backend", "s3")
    monkeypatch.setattr(settings, "backup_s3_bucket", "notturni-backups-test")

    source = FakeS3Client()
    source.create_bucket(settings.s3_bucket_content)
    source.put_object(Bucket=settings.s3_bucket_content, Key="a/b.png", Body=b"immagine")
    # s3_bucket_avatars/s3_bucket_audit non ancora creati: NoSuchBucket lato reale

    monkeypatch.setattr(backup, "get_s3_client", lambda: source)

    backup_client = FakeS3Client()
    counts = backup.mirror_minio(backup_client)

    assert counts[settings.s3_bucket_content] == 1
    assert counts[settings.s3_bucket_avatars] == 0
    assert counts[settings.s3_bucket_audit] == 0
    assert backup_client.objects[
        ("notturni-backups-test", f"minio/{settings.s3_bucket_content}/a/b.png")
    ] == b"immagine"


def test_run_once_full_flow(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(settings, "backup_s3_bucket", "notturni-backups-test")
    monkeypatch.setattr(settings, "storage_backend", "s3")
    monkeypatch.setattr(backup, "dump_postgres", lambda: b"contenuto-dump")

    source = FakeS3Client()
    backup_client = FakeS3Client()
    monkeypatch.setattr(backup, "get_s3_client", lambda: source)
    monkeypatch.setattr(backup, "get_backup_s3_client", lambda: backup_client)

    result = backup.run_once()

    assert result["postgres_key"].startswith("postgres/")
    assert set(result["mirrored_objects"]) == {
        settings.s3_bucket_content,
        settings.s3_bucket_avatars,
        settings.s3_bucket_audit,
    }
    assert ("notturni-backups-test", result["postgres_key"]) in backup_client.objects
