"""todo/UX_REDESIGN.md B5: segnalazioni dai lettori, pannello admin, azioni
con nota obbligatoria."""

from collections.abc import Callable

from httpx import AsyncClient

from tests.conftest import AuthedUser
from tests.test_overview import _blog, _post


async def test_readers_report_blog_and_post_once(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("rep-owner")
    reader: AuthedUser = await make_user("rep-reader")
    await _blog(client, owner, "rep-blog")
    post = await _post(client, owner, "rep-blog", "post")

    assert (await client.post("/api/v1/blogs/rep-blog/report", json={"reason": "spam"})).status_code == 401
    own = await client.post("/api/v1/blogs/rep-blog/report", json={"reason": "spam"}, headers=owner.headers)
    assert own.status_code == 400

    first = await client.post("/api/v1/blogs/rep-blog/report", json={"reason": "spam", "note": "link affiliati"}, headers=reader.headers)
    assert first.status_code == 201, first.text
    again = await client.post("/api/v1/blogs/rep-blog/report", json={"reason": "abuse"}, headers=reader.headers)
    assert again.status_code == 201 and again.json()["id"] == first.json()["id"]
    assert again.json()["reason"] == "spam"  # idempotente: resta la prima

    p = await client.post(f"/api/v1/posts/{post['id']}/report", json={"reason": "illegal"}, headers=reader.headers)
    assert p.status_code == 201 and p.json()["target_type"] == "post"


async def test_cannot_report_private_blog_or_its_posts(client: AsyncClient, make_user: Callable) -> None:
    """is_blog_publicly_readable copre solo sospensione/pausa/cancellazione,
    non la visibilità (todo/BLOG.md #2) — un utente autenticato ma senza
    membership su un blog privato non deve poter segnalare un blog/post che
    non può nemmeno vedere (bug corretto: usava quella invece di
    can_view_blog)."""
    owner: AuthedUser = await make_user("priv-owner")
    outsider: AuthedUser = await make_user("priv-outsider")
    await _blog(client, owner, "priv-blog", visibility="private")
    post = await _post(client, owner, "priv-blog", "segreto")

    blog_report = await client.post(
        "/api/v1/blogs/priv-blog/report", json={"reason": "spam"}, headers=outsider.headers
    )
    assert blog_report.status_code == 404

    post_report = await client.post(
        f"/api/v1/posts/{post['id']}/report", json={"reason": "spam"}, headers=outsider.headers
    )
    assert post_report.status_code == 404


async def test_admin_sees_reports_and_acts_with_mandatory_note(
    client: AsyncClient, make_user: Callable, make_admin: Callable
) -> None:
    owner: AuthedUser = await make_user("act-owner")
    reader: AuthedUser = await make_user("act-reader")
    admin: AuthedUser = await make_admin("act-admin")
    blog = await _blog(client, owner, "act-blog")
    post = await _post(client, owner, "act-blog", "segnalato")
    await client.post("/api/v1/blogs/act-blog/report", json={"reason": "spam", "note": "spam"}, headers=reader.headers)
    await client.post(f"/api/v1/posts/{post['id']}/report", json={"reason": "abuse"}, headers=reader.headers)

    listed = await client.get("/api/v1/admin/blogs", params={"state": "reported"}, headers=admin.headers)
    assert listed.status_code == 200
    row = next(b for b in listed.json() if b["slug"] == "act-blog")
    assert row["reports_open"] == 2 and row["posts_count"] == 1

    panel = await client.get(f"/api/v1/admin/blogs/{blog['id']}/reports", headers=admin.headers)
    assert panel.status_code == 200, panel.text
    data = panel.json()
    assert len(data["reports"]) == 2
    assert {r["reason"] for r in data["reports"]} == {"spam", "abuse"}
    assert any(r["post_slug"] == "segnalato" for r in data["reports"])
    assert data["owner_email_domain"] == "example.com"

    no_note = await client.post(f"/api/v1/admin/blogs/{blog['id']}/action", json={"action": "suspend", "note": " "}, headers=admin.headers)
    assert no_note.status_code == 400
    hidden = await client.post(
        f"/api/v1/admin/blogs/{blog['id']}/action", json={"action": "hide_reported_posts", "note": "post con abusi"}, headers=admin.headers
    )
    assert hidden.status_code == 200, hidden.text
    assert hidden.json()["reports_open"] == 0
    posts = await client.get("/api/v1/admin/posts", params={"q": "segnalato"}, headers=admin.headers)
    assert next(p for p in posts.json() if p["id"] == post["id"])["is_hidden"] is True

    suspend = await client.post(f"/api/v1/admin/blogs/{blog['id']}/action", json={"action": "suspend", "note": "recidivo"}, headers=admin.headers)
    assert suspend.status_code == 200 and suspend.json()["is_suspended"] is True

    audit = await client.get("/api/v1/admin/audit-log", params={"action": "blog.suspended"}, headers=admin.headers)
    assert audit.status_code == 200
    entries = audit.json()["items"] if isinstance(audit.json(), dict) else audit.json()
    assert any(e.get("payload", {}).get("note") == "recidivo" for e in entries)


async def test_admin_patch_requires_note_when_state_changes(client: AsyncClient, make_user: Callable, make_admin: Callable) -> None:
    owner: AuthedUser = await make_user("nt-owner")
    admin: AuthedUser = await make_admin("nt-admin")
    blog = await _blog(client, owner, "nt-blog")
    res = await client.patch(f"/api/v1/admin/blogs/{blog['id']}", json={"is_suspended": True}, headers=admin.headers)
    assert res.status_code == 400
    res = await client.patch(f"/api/v1/admin/blogs/{blog['id']}", json={"is_suspended": True, "note": "spam"}, headers=admin.headers)
    assert res.status_code == 200 and res.json()["is_suspended"] is True
    # nessun cambiamento: nessuna nota richiesta
    res = await client.patch(f"/api/v1/admin/blogs/{blog['id']}", json={"is_suspended": True}, headers=admin.headers)
    assert res.status_code == 200
