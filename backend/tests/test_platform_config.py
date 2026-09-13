"""todo/UX_REDESIGN.md B6: impostazioni di piattaforma, coda GDPR, lingua interfaccia."""

from collections.abc import Callable

from httpx import AsyncClient
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.user import PlatformRole, User
from tests.conftest import AuthedUser
from tests.test_overview import _blog


async def _super(make_admin: Callable, db_session: AsyncSession, name: str) -> AuthedUser:
    authed = await make_admin(name)
    user = (await db_session.execute(select(User).where(User.username == name))).scalar_one()
    user.platform_role = PlatformRole.SUPER_ADMIN
    await db_session.commit()
    return authed


async def test_public_config_exposes_defaults_and_super_admin_updates(
    client: AsyncClient, make_admin: Callable, make_user: Callable, db_session: AsyncSession
) -> None:
    pub = await client.get("/api/v1/config")
    assert pub.status_code == 200
    assert pub.json()["default_locale"] == "it" and pub.json()["registration_mode"] == "open"

    admin: AuthedUser = await make_admin("pc-admin")
    assert (await client.get("/api/v1/admin/config", headers=admin.headers)).status_code == 403
    root = await _super(make_admin, db_session, "pc-root")
    cfg = await client.get("/api/v1/admin/config", headers=root.headers)
    assert cfg.status_code == 200 and cfg.json()["max_blogs_per_user"] == 5
    assert "blog" in cfg.json()["reserved_builtin"]

    bad = await client.patch("/api/v1/admin/config", json={"default_locale": "de"}, headers=root.headers)
    assert bad.status_code == 400
    res = await client.patch(
        "/api/v1/admin/config",
        json={"default_locale": "en", "max_blogs_per_user": 1, "reserved_blog_names": ["Notturni", "help"], "anonymous_comments_allowed": False},
        headers=root.headers,
    )
    assert res.status_code == 200, res.text
    assert res.json()["reserved_blog_names"] == ["help", "notturni"]
    assert (await client.get("/api/v1/config")).json()["default_locale"] == "en"

    owner: AuthedUser = await make_user("pc-owner")
    assert (await client.post("/api/v1/blogs", json={"slug": "help", "title": "x"}, headers=owner.headers)).status_code == 400
    await _blog(client, owner, "pc-first")
    second = await client.post("/api/v1/blogs", json={"slug": "pc-second", "title": "x"}, headers=owner.headers)
    assert second.status_code == 400
    anon = await client.patch("/api/v1/blogs/pc-first", json={"comments_mode": "everyone"}, headers=owner.headers)
    assert anon.status_code == 400

    closed = await client.patch("/api/v1/admin/config", json={"registration_mode": "closed"}, headers=root.headers)
    assert closed.status_code == 200
    reg = await client.post("/api/v1/auth/register", json={"username": "pc-late", "email": "pc-late@example.com", "password": "Password123!"})
    assert reg.status_code == 403
    await client.patch("/api/v1/admin/config", json={"registration_mode": "open", "max_blogs_per_user": 5, "anonymous_comments_allowed": True}, headers=root.headers)

    audit = await client.get("/api/v1/admin/audit-log", params={"action": "platform.config_updated"}, headers=root.headers)
    assert audit.status_code == 200


async def test_mfa_required_for_admins_blocks_admin_area(client: AsyncClient, make_admin: Callable, db_session: AsyncSession) -> None:
    root = await _super(make_admin, db_session, "mfa-root")
    other: AuthedUser = await make_admin("mfa-other")
    assert (await client.get("/api/v1/admin/users", headers=other.headers)).status_code == 200
    res = await client.patch("/api/v1/admin/config", json={"mfa_required_for_admins": True}, headers=root.headers)
    assert res.status_code == 200
    blocked = await client.get("/api/v1/admin/users", headers=other.headers)
    assert blocked.status_code == 403
    await client.patch("/api/v1/admin/config", json={"mfa_required_for_admins": False}, headers=root.headers)


async def test_ui_locale_saved_on_profile(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user("ui-user")
    assert (await client.patch("/api/v1/users/me", json={"ui_locale": "de"}, headers=user.headers)).status_code == 400
    res = await client.patch("/api/v1/users/me", json={"ui_locale": "en"}, headers=user.headers)
    assert res.status_code == 200
    me = await client.get("/api/v1/auth/me", headers=user.headers)
    assert me.json()["ui_locale"] == "en"
    res = await client.patch("/api/v1/users/me", json={"ui_locale": ""}, headers=user.headers)
    assert (await client.get("/api/v1/auth/me", headers=user.headers)).json()["ui_locale"] is None


async def test_gdpr_queue_second_admin_approval(client: AsyncClient, make_admin: Callable, make_user: Callable) -> None:
    a1: AuthedUser = await make_admin("gq-a1")
    a2: AuthedUser = await make_admin("gq-a2")
    target: AuthedUser = await make_user("gq-target")
    await _blog(client, target, "gq-blog")

    # self-service export lascia una riga completata
    assert (await client.get("/api/v1/users/me/export-data", headers=target.headers)).status_code == 200
    rows = (await client.get("/api/v1/admin/gdpr", headers=a1.headers)).json()
    assert any(r["username"] == "gq-target" and r["type"] == "export" and r["status"] == "completed" for r in rows)

    no_note = await client.post("/api/v1/admin/gdpr", json={"username": "gq-target", "type": "deletion", "note": ""}, headers=a1.headers)
    assert no_note.status_code == 400
    created = await client.post("/api/v1/admin/gdpr", json={"username": "gq-target", "type": "deletion", "note": "richiesta via PEC"}, headers=a1.headers)
    assert created.status_code == 201, created.text
    rid = created.json()["id"]
    assert created.json()["deadline_at"] is not None

    assert (await client.post(f"/api/v1/admin/gdpr/{rid}/execute", headers=a1.headers)).status_code == 400
    same = await client.post(f"/api/v1/admin/gdpr/{rid}/approve", headers=a1.headers)
    assert same.status_code == 403
    ok = await client.post(f"/api/v1/admin/gdpr/{rid}/approve", headers=a2.headers)
    assert ok.status_code == 200 and ok.json()["approved_by_username"] == "gq-a2"

    done = await client.post(f"/api/v1/admin/gdpr/{rid}/execute", headers=a1.headers)
    assert done.status_code == 200, done.text
    assert (await client.get("/api/v1/users/gq-target")).status_code == 404
    rows = (await client.get("/api/v1/admin/gdpr", params={"status": "completed"}, headers=a1.headers)).json()
    assert any(r["id"] == rid for r in rows)

    exp = await client.post("/api/v1/admin/gdpr", json={"username": "gq-a2", "type": "export", "note": "test"}, headers=a1.headers)
    data = await client.post(f"/api/v1/admin/gdpr/{exp.json()['id']}/execute", headers=a1.headers)
    assert data.status_code == 200 and data.json()["account"]["username"] == "gq-a2"
