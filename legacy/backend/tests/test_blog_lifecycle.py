"""todo/UX_REDESIGN.md B3: pausa, lingue secondarie, trasferimento, export,
cancellazione con tolleranza e purge."""

import io
import zipfile
from collections.abc import Callable
from datetime import datetime, timedelta, timezone

from httpx import AsyncClient
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.domain.blog_lifecycle import purge_deleted_blogs
from app.models.blog import Blog
from tests.conftest import AuthedUser, FakeS3Client
from tests.test_overview import _blog, _post


async def _add_coauthor(client: AsyncClient, owner: AuthedUser, slug: str, member: AuthedUser) -> None:
    inv = await client.post(
        f"/api/v1/blogs/{slug}/invitations", json={"username": member.username, "role": "co_autore"}, headers=owner.headers
    )
    assert inv.status_code == 201, inv.text
    acc = await client.post(f"/api/v1/blogs/received-invitations/{inv.json()['id']}/accept", headers=member.headers)
    assert acc.status_code == 200, acc.text


async def test_pause_hides_content_from_readers_but_not_owner(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("lc-pause-owner")
    await _blog(client, owner, "lc-pause")
    post = await _post(client, owner, "lc-pause", "visibile")

    res = await client.patch("/api/v1/blogs/lc-pause", json={"is_paused": True}, headers=owner.headers)
    assert res.status_code == 200 and res.json()["is_paused"] is True

    public = await client.get("/api/v1/blogs/lc-pause")
    assert public.status_code == 200 and public.json()["is_paused"] is True
    assert (await client.get("/api/v1/blogs/lc-pause/posts")).status_code == 404
    assert (await client.get(f"/api/v1/posts/{post['id']}/read")).status_code in (404, 405)
    feed_ids = {p["id"] for p in (await client.get("/api/v1/feed/posts")).json()}
    assert post["id"] not in feed_ids
    assert "lc-pause" not in {b["slug"] for b in (await client.get("/api/v1/blogs")).json()}

    mine = await client.get("/api/v1/blogs/lc-pause/posts", headers=owner.headers)
    assert mine.status_code == 200 and len(mine.json()) == 1

    res = await client.patch("/api/v1/blogs/lc-pause", json={"is_paused": False}, headers=owner.headers)
    assert res.json()["is_paused"] is False
    assert (await client.get("/api/v1/blogs/lc-pause/posts")).status_code == 200


async def test_extra_locales_validated_and_deduplicated(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("lc-lang-owner")
    await _blog(client, owner, "lc-lang")
    res = await client.patch("/api/v1/blogs/lc-lang", json={"extra_locales": ["en", "it", "en", "de"]}, headers=owner.headers)
    assert res.status_code == 200, res.text
    assert res.json()["extra_locales"] == ["en", "de"]
    bad = await client.patch("/api/v1/blogs/lc-lang", json={"extra_locales": ["english"]}, headers=owner.headers)
    assert bad.status_code == 400


async def test_transfer_ownership_to_coauthor(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("lc-tr-owner")
    coauthor: AuthedUser = await make_user("lc-tr-co")
    stranger: AuthedUser = await make_user("lc-tr-stranger")
    await _blog(client, owner, "lc-transfer")
    await _add_coauthor(client, owner, "lc-transfer", coauthor)

    bad = await client.post("/api/v1/blogs/lc-transfer/transfer", json={"username": stranger.username}, headers=owner.headers)
    assert bad.status_code == 400

    res = await client.post("/api/v1/blogs/lc-transfer/transfer", json={"username": coauthor.username}, headers=owner.headers)
    assert res.status_code == 200, res.text
    assert res.json()["owner_id"] is None  # il vecchio proprietario non è più tale

    as_new_owner = await client.get("/api/v1/blogs/lc-transfer", headers=coauthor.headers)
    assert as_new_owner.json()["owner_id"] is not None
    members = await client.get("/api/v1/blogs/lc-transfer/members", headers=coauthor.headers)
    assert members.status_code == 200
    roles = {m["username"]: m["role"] for m in members.json()}
    assert roles.get(owner.username) == "co_autore"
    assert coauthor.username not in roles

    again = await client.post("/api/v1/blogs/lc-transfer/transfer", json={"username": coauthor.username}, headers=owner.headers)
    assert again.status_code == 403


async def test_export_zip_contains_posts_and_metadata(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("lc-ex-owner")
    other: AuthedUser = await make_user("lc-ex-other")
    await _blog(client, owner, "lc-export")
    res = await client.post(
        "/api/v1/blogs/lc-export/posts",
        json={"slug": "con-note", "title": "Con note", "content": "Testo [1](#nota-1)", "notes": [{"idx": 1, "content": "Nota uno"}]},
        headers=owner.headers,
    )
    assert res.status_code == 201, res.text

    assert (await client.get("/api/v1/blogs/lc-export/export", headers=other.headers)).status_code == 403
    res = await client.get("/api/v1/blogs/lc-export/export", headers=owner.headers)
    assert res.status_code == 200
    assert res.headers["content-type"].startswith("application/zip")
    with zipfile.ZipFile(io.BytesIO(res.content)) as zf:
        names = set(zf.namelist())
        assert {"blog.json", "comments.json", "categories.json", "media.json", "links.json", "posts/it-con-note.md"} <= names
        md = zf.read("posts/it-con-note.md").decode()
        assert "title: " in md and "[^1]: Nota uno" in md


async def test_soft_delete_restore_and_purge(
    client: AsyncClient, make_user: Callable, db_session: AsyncSession, fake_s3: FakeS3Client
) -> None:
    owner: AuthedUser = await make_user("lc-del-owner")
    reader: AuthedUser = await make_user("lc-del-reader")
    await _blog(client, owner, "lc-delete")
    post = await _post(client, owner, "lc-delete", "sparira")
    await client.post(f"/api/v1/posts/{post['id']}/comments", json={"content": "ciao"}, headers=reader.headers)

    wrong = await client.request("DELETE", "/api/v1/blogs/lc-delete", json={"confirm_slug": "altro"}, headers=owner.headers)
    assert wrong.status_code == 400
    res = await client.request("DELETE", "/api/v1/blogs/lc-delete", json={"confirm_slug": "lc-delete"}, headers=owner.headers)
    assert res.status_code == 200, res.text
    assert res.json()["deleted_at"] is not None

    assert (await client.get("/api/v1/blogs/lc-delete")).status_code == 404
    assert (await client.get("/api/v1/blogs/lc-delete", headers=owner.headers)).status_code == 200
    assert (await client.get("/api/v1/blogs/lc-delete", headers=reader.headers)).status_code == 404
    assert "lc-delete" in {b["slug"] for b in (await client.get("/api/v1/blogs/mine", headers=owner.headers)).json()}

    restored = await client.post("/api/v1/blogs/lc-delete/restore", headers=owner.headers)
    assert restored.status_code == 200 and restored.json()["deleted_at"] is None
    assert (await client.get("/api/v1/blogs/lc-delete")).status_code == 200

    await client.request("DELETE", "/api/v1/blogs/lc-delete", json={"confirm_slug": "lc-delete"}, headers=owner.headers)
    blog = (await db_session.execute(select(Blog).where(Blog.slug == "lc-delete"))).scalar_one()
    blog.deleted_at = datetime.now(timezone.utc) - timedelta(days=31)
    await db_session.commit()

    assert await purge_deleted_blogs(db_session) == 1
    assert (await db_session.execute(select(Blog).where(Blog.slug == "lc-delete"))).scalar_one_or_none() is None
    assert (await client.get("/api/v1/blogs/lc-delete", headers=owner.headers)).status_code == 404
