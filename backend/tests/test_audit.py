from collections.abc import Callable

from httpx import AsyncClient
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.domain import audit
from app.models.audit_log import AuditActorType, AuditLog
from tests.conftest import AuthedUser


async def _events(db_session: AsyncSession, action: str | None = None) -> list[AuditLog]:
    stmt = select(AuditLog).order_by(AuditLog.occurred_at)
    if action is not None:
        stmt = stmt.where(AuditLog.action == action)
    result = await db_session.execute(stmt)
    return list(result.scalars().all())


async def test_record_defaults_to_system_actor(db_session: AsyncSession) -> None:
    await audit.record(db_session, action="test.event")
    await db_session.commit()

    events = await _events(db_session, "test.event")
    assert len(events) == 1
    assert events[0].actor_type == AuditActorType.SYSTEM
    assert events[0].actor_id is None
    assert events[0].payload == {}


async def test_successful_login_is_recorded(
    client: AsyncClient, make_user: Callable, db_session: AsyncSession
) -> None:
    user: AuthedUser = await make_user("audit-login")

    events = await _events(db_session, "auth.login")
    assert len(events) == 1
    assert events[0].actor_type == AuditActorType.USER
    assert events[0].actor_label == f"{user.username} <{user.email}>"
    assert events[0].payload == {"method": "password"}


async def test_failed_login_is_recorded(
    client: AsyncClient, make_user: Callable, db_session: AsyncSession
) -> None:
    user: AuthedUser = await make_user("audit-badpass")

    res = await client.post(
        "/api/v1/auth/login", json={"email": user.email, "password": "sbagliata"}
    )
    assert res.status_code == 401

    events = await _events(db_session, "auth.login_failed")
    assert len(events) == 1
    assert events[0].actor_type == AuditActorType.ANONYMOUS
    assert events[0].actor_label == user.email
    assert events[0].payload["email"] == user.email


async def test_admin_deactivation_is_recorded(
    client: AsyncClient, make_admin: Callable, make_user: Callable, db_session: AsyncSession
) -> None:
    admin: AuthedUser = await make_admin("audit-admin")
    target: AuthedUser = await make_user("audit-target")
    users_res = await client.get("/api/v1/admin/users", headers=admin.headers)
    target_id = next(u["id"] for u in users_res.json() if u["username"] == target.username)

    res = await client.patch(
        f"/api/v1/admin/users/{target_id}", json={"is_active": False}, headers=admin.headers
    )
    assert res.status_code == 200

    events = await _events(db_session, "user.deactivated")
    assert len(events) == 1
    assert events[0].actor_label == f"{admin.username} <{admin.email}>"
    assert events[0].target_type == "user"
    assert str(events[0].target_id) == target_id


async def test_role_change_records_old_and_new_value(
    client: AsyncClient, make_admin: Callable, make_user: Callable, db_session: AsyncSession
) -> None:
    admin: AuthedUser = await make_admin("audit-admin2")
    target: AuthedUser = await make_user("audit-role")
    users_res = await client.get("/api/v1/admin/users", headers=admin.headers)
    target_id = next(u["id"] for u in users_res.json() if u["username"] == target.username)

    res = await client.patch(
        f"/api/v1/admin/users/{target_id}",
        json={"platform_role": "moderatore"},
        headers=admin.headers,
    )
    assert res.status_code == 200

    events = await _events(db_session, "user.role_change")
    assert len(events) == 1
    assert events[0].payload == {"from": "utente", "to": "moderatore"}


async def test_no_event_when_value_is_unchanged(
    client: AsyncClient, make_admin: Callable, make_user: Callable, db_session: AsyncSession
) -> None:
    admin: AuthedUser = await make_admin("audit-admin3")
    target: AuthedUser = await make_user("audit-noop")
    users_res = await client.get("/api/v1/admin/users", headers=admin.headers)
    target_id = next(u["id"] for u in users_res.json() if u["username"] == target.username)

    # l'utente è già attivo: reinviare is_active=True non deve generare eventi
    res = await client.patch(
        f"/api/v1/admin/users/{target_id}", json={"is_active": True}, headers=admin.headers
    )
    assert res.status_code == 200
    assert await _events(db_session, "user.activated") == []
    assert await _events(db_session, "user.deactivated") == []


async def test_blog_suspension_is_recorded(
    client: AsyncClient, make_admin: Callable, make_user: Callable, db_session: AsyncSession
) -> None:
    admin: AuthedUser = await make_admin("audit-blogadmin")
    owner: AuthedUser = await make_user("audit-blogowner")
    create_res = await client.post(
        "/api/v1/blogs", json={"slug": "audit-blog", "title": "x"}, headers=owner.headers
    )
    blog_id = create_res.json()["id"]

    res = await client.patch(
        f"/api/v1/admin/blogs/{blog_id}", json={"is_suspended": True}, headers=admin.headers
    )
    assert res.status_code == 200

    events = await _events(db_session, "blog.suspended")
    assert len(events) == 1
    assert str(events[0].blog_id) == blog_id
    assert events[0].payload == {"slug": "audit-blog", "blog_alias": None}


async def test_api_token_creation_is_recorded(
    client: AsyncClient, core_api_token: str, db_session: AsyncSession
) -> None:
    res = await client.post(
        "/api/v1/tokens",
        json={"name": "figlio"},
        headers={"Authorization": f"Bearer {core_api_token}"},
    )
    assert res.status_code == 201

    events = await _events(db_session, "api_token.created")
    assert len(events) == 1
    assert events[0].actor_type == AuditActorType.CORE_TOKEN
    assert events[0].actor_label == "test-core-token"
    assert events[0].payload == {"name": "figlio", "owner_type": "core"}


async def test_blog_alias_included_when_blog_has_one(
    client: AsyncClient, make_admin: Callable, make_user: Callable, db_session: AsyncSession
) -> None:
    admin: AuthedUser = await make_admin("audit-aliasadmin")
    owner: AuthedUser = await make_user("audit-aliasowner")
    create_res = await client.post(
        "/api/v1/blogs",
        json={"slug": "audit-blog-alias", "title": "x", "default_author_display_name": "Ismaele"},
        headers=owner.headers,
    )
    blog_id = create_res.json()["id"]

    res = await client.patch(
        f"/api/v1/admin/blogs/{blog_id}", json={"is_suspended": True}, headers=admin.headers
    )
    assert res.status_code == 200

    events = await _events(db_session, "blog.suspended")
    assert events[0].payload["blog_alias"] == "Ismaele"


async def test_comment_moderation_is_recorded(
    client: AsyncClient, make_user: Callable, db_session: AsyncSession
) -> None:
    owner: AuthedUser = await make_user("audit-commentowner")
    await client.post(
        "/api/v1/blogs",
        json={"slug": "audit-comment-blog", "title": "x", "default_author_display_name": "Ismaele"},
        headers=owner.headers,
    )
    post_res = await client.post(
        "/api/v1/blogs/audit-comment-blog/posts",
        json={"slug": "audit-comment-post", "title": "x", "content": "y"},
        headers=owner.headers,
    )
    post_id = post_res.json()["id"]
    await client.post(f"/api/v1/posts/{post_id}/publish", headers=owner.headers)
    comment_res = await client.post(
        f"/api/v1/posts/{post_id}/comments", json={"content": "z"}, headers=owner.headers
    )
    comment_id = comment_res.json()["id"]

    res = await client.post(f"/api/v1/comments/{comment_id}/approve", headers=owner.headers)
    assert res.status_code == 200

    events = await _events(db_session, "comment.approved")
    assert len(events) == 1
    assert str(events[0].target_id) == comment_id
    assert events[0].payload["blog_slug"] == "audit-comment-blog"
    assert events[0].payload["blog_alias"] == "Ismaele"


async def test_audit_log_channel_field_and_filter(
    client: AsyncClient, make_admin: Callable, make_user: Callable, core_api_token: str
) -> None:
    admin: AuthedUser = await make_admin("audit-channeladmin")
    target: AuthedUser = await make_user("audit-channeltarget")
    # evento "web": cambio ruolo via sessione JWT
    users_res = await client.get("/api/v1/admin/users", headers=admin.headers)
    target_id = next(u["id"] for u in users_res.json() if u["username"] == target.username)
    await client.patch(
        f"/api/v1/admin/users/{target_id}", json={"platform_role": "moderatore"}, headers=admin.headers
    )
    # evento "api": creazione di un token via ApiToken opaco
    await client.post(
        "/api/v1/tokens", json={"name": "canale-api"}, headers={"Authorization": f"Bearer {core_api_token}"}
    )

    web_res = await client.get(
        "/api/v1/admin/audit-log", params={"channel": "web"}, headers=admin.headers
    )
    assert web_res.status_code == 200
    assert all(e["channel"] == "web" for e in web_res.json())
    assert any(e["action"] == "user.role_change" for e in web_res.json())

    api_res = await client.get(
        "/api/v1/admin/audit-log", params={"channel": "api"}, headers=admin.headers
    )
    assert api_res.status_code == 200
    assert all(e["channel"] == "api" for e in api_res.json())
    assert any(e["action"] == "api_token.created" for e in api_res.json())
