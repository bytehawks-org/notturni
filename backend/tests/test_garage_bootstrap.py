"""scripts/garage_bootstrap.py contro un finto Garage (httpx.MockTransport):
nessun Garage reale richiesto, come per gli altri test che sostituiscono S3."""

import json

import httpx
import pytest

from app.core.config import settings
from scripts import garage_bootstrap as bootstrap

NODE_ID = "ab" * 32


class FakeGarageAdmin:
    def __init__(self, *, role=None) -> None:
        self.role = role
        self.layout_version = 0
        self.staged: list[dict] = []
        self.keys: dict[str, dict] = {}
        self.buckets: dict[str, dict] = {}  # alias globale -> stato
        self.calls: list[tuple[str, str]] = []

    def __call__(self, request: httpx.Request) -> httpx.Response:
        path, params = request.url.path, dict(request.url.params)
        body = json.loads(request.content) if request.content else {}
        self.calls.append((request.method, path))

        if path == "/v2/GetClusterStatus":
            node = {"id": NODE_ID, "isUp": True, "role": self.role}
            return httpx.Response(200, json={"layoutVersion": self.layout_version, "nodes": [node]})
        if path == "/v2/UpdateClusterLayout":
            self.staged = body["roles"]
            return httpx.Response(200, json={})
        if path == "/v2/ApplyClusterLayout":
            assert body["version"] == self.layout_version + 1
            self.layout_version = body["version"]
            self.role = self.staged[0]
            return httpx.Response(200, json={})
        if path == "/v2/GetKeyInfo":
            return httpx.Response(200 if params["id"] in self.keys else 404, json={})
        if path == "/v2/ImportKey":
            self.keys[body["accessKeyId"]] = body
            return httpx.Response(200, json={})
        if path == "/v2/GetBucketInfo":
            bucket = self.buckets.get(params["globalAlias"])
            return httpx.Response(200, json=bucket) if bucket else httpx.Response(404, json={})
        if path == "/v2/CreateBucket":
            bucket = {"id": f"id-{body['globalAlias']}", "website": None, "keys": []}
            self.buckets[body["globalAlias"]] = bucket
            return httpx.Response(200, json=bucket)
        if path == "/v2/AllowBucketKey":
            bucket = next(b for b in self.buckets.values() if b["id"] == body["bucketId"])
            bucket["keys"].append(body)
            return httpx.Response(200, json={})
        if path == "/v2/UpdateBucket":
            bucket = next(b for b in self.buckets.values() if b["id"] == params["id"])
            bucket["website"] = body["websiteAccess"]["enabled"]
            return httpx.Response(200, json={})
        return httpx.Response(500, json={"path": path})


@pytest.fixture
def garage_settings(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(settings, "garage_admin_token", "token")
    monkeypatch.setattr(settings, "s3_access_key_id", "GK" + "0" * 24)
    monkeypatch.setattr(settings, "s3_secret_access_key", "1" * 64)
    monkeypatch.setattr(settings, "s3_endpoint_url", "http://garage:3900")
    monkeypatch.setattr(settings, "backup_s3_bucket", "backups")
    monkeypatch.setattr(settings, "backup_s3_endpoint_url", "http://garage:3900")


def _run(fake: FakeGarageAdmin) -> None:
    with httpx.Client(base_url="http://garage:3903", transport=httpx.MockTransport(fake)) as client:
        status = bootstrap._wait_ready(client)
        bootstrap.ensure_layout(client, status)
        bootstrap.ensure_key(client)
        for name, public in bootstrap._bucket_plan():
            bootstrap.ensure_bucket(client, name, public=public)


def test_bootstrap_configures_fresh_garage(garage_settings: None) -> None:
    fake = FakeGarageAdmin()
    _run(fake)

    assert fake.layout_version == 1
    assert fake.role["capacity"] == settings.garage_node_capacity_bytes
    assert settings.s3_access_key_id in fake.keys
    public = {settings.s3_bucket_avatars, settings.s3_bucket_content}
    assert set(fake.buckets) == public | {settings.s3_bucket_audit, "backups"}
    for name, bucket in fake.buckets.items():
        assert bucket["website"] is (name in public)
        assert bucket["keys"][0]["permissions"] == {"read": True, "write": True, "owner": True}


def test_bootstrap_is_idempotent(garage_settings: None) -> None:
    fake = FakeGarageAdmin()
    _run(fake)
    fake.calls.clear()
    _run(fake)

    assert fake.layout_version == 1
    assert ("POST", "/v2/UpdateClusterLayout") not in fake.calls
    assert ("POST", "/v2/ImportKey") not in fake.calls
    assert ("POST", "/v2/CreateBucket") not in fake.calls


def test_backup_bucket_skipped_when_destination_is_external(
    garage_settings: None, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(settings, "backup_s3_endpoint_url", "https://s3.external.example.com")
    assert "backups" not in [name for name, _ in bootstrap._bucket_plan()]
