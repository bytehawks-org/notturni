"""Configurazione dei bucket (app/core/storage.py::ensure_*): accesso pubblico
via bucket policy (AWS/S3 generico) o via website access (Garage), e
memorizzazione dei bucket già configurati."""

import json

import pytest

from app.core import storage
from app.core.config import settings
from tests.conftest import FakeS3Client


@pytest.fixture(autouse=True)
def s3_backend(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(settings, "storage_backend", "s3")


def test_policy_mode_sets_get_object_policy(fake_s3: FakeS3Client, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(settings, "s3_public_access", "policy")

    storage.ensure_content_bucket("content")

    statement = json.loads(fake_s3.policies["content"])["Statement"][0]
    assert statement["Action"] == "s3:GetObject"
    assert statement["Resource"] == "arn:aws:s3:::content/*/userdata/*/*/media/*"
    assert fake_s3.websites == {}


def test_website_mode_enables_website_access_without_policy(
    fake_s3: FakeS3Client, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(settings, "s3_public_access", "website")

    storage.ensure_public_bucket("avatars")

    assert fake_s3.websites["avatars"] == {"IndexDocument": {"Suffix": "index.html"}}
    assert fake_s3.policies == {}


def test_private_bucket_gets_no_public_access(fake_s3: FakeS3Client, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(settings, "s3_public_access", "website")

    storage.ensure_private_bucket("audit")

    assert "audit" in fake_s3.buckets
    assert fake_s3.policies == {} and fake_s3.websites == {}


def test_ensure_runs_once_per_bucket(fake_s3: FakeS3Client, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(settings, "s3_public_access", "policy")
    calls: list[str] = []
    original = fake_s3.put_bucket_policy
    monkeypatch.setattr(fake_s3, "put_bucket_policy", lambda **kw: (calls.append(kw["Bucket"]), original(**kw)))

    storage.ensure_public_bucket("avatars")
    storage.ensure_public_bucket("avatars")
    storage.ensure_content_bucket("content")

    assert calls == ["avatars", "content"]


def test_ensure_is_noop_on_localstorage(fake_s3: FakeS3Client, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(settings, "storage_backend", "localstorage")

    storage.ensure_public_bucket("avatars")

    assert fake_s3.buckets == set()
