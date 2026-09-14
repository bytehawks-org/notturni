"""todo/UX_REDESIGN.md B4: bloccati per blog, segnalazione alla piattaforma,
chiusura automatica dei commenti."""

from collections.abc import Callable
from datetime import datetime, timedelta, timezone

from httpx import AsyncClient
from sqlalchemy import select, update
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.post import Post
from tests.conftest import AuthedUser
from tests.test_overview import _blog, _post


async def test_block_author_hides_comment_and_prevents_new_ones(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("bl-owner")
    troll: AuthedUser = await make_user("bl-troll")
    await _blog(client, owner, "bl-blog")
    post = await _post(client, owner, "bl-blog", "aperto")
    c = await client.post(f"/api/v1/posts/{post['id']}/comments", json={"content": "spam"}, headers=troll.headers)
    assert c.status_code == 201

    forbidden = await client.post(f"/api/v1/comments/{c.json()['id']}/block-author", json={}, headers=troll.headers)
    assert forbidden.status_code == 403
    res = await client.post(f"/api/v1/comments/{c.json()['id']}/block-author", json={"note": "spam"}, headers=owner.headers)
    assert res.status_code == 200, res.text
    assert res.json()["label"] == "@bl-troll" and res.json()["is_anonymous"] is False

    again = await client.post(f"/api/v1/posts/{post['id']}/comments", json={"content": "ancora"}, headers=troll.headers)
    assert again.status_code == 403
    hidden = await client.get("/api/v1/blogs/bl-blog/comments", params={"status": "rejected"}, headers=owner.headers)
    assert [x["id"] for x in hidden.json()] == [c.json()["id"]]

    blocked = await client.get("/api/v1/blogs/bl-blog/blocked", headers=owner.headers)
    assert blocked.status_code == 200 and len(blocked.json()) == 1
    assert (await client.delete(f"/api/v1/blogs/bl-blog/blocked/{blocked.json()[0]['id']}", headers=owner.headers)).status_code == 204
    assert (await client.post(f"/api/v1/posts/{post['id']}/comments", json={"content": "sbloccato"}, headers=troll.headers)).status_code == 201


async def test_report_to_platform_requires_note_and_shows_in_admin_queue(
    client: AsyncClient, make_user: Callable, make_moderator: Callable
) -> None:
    owner: AuthedUser = await make_user("rp-owner")
    reader: AuthedUser = await make_user("rp-reader")
    moderator: AuthedUser = await make_moderator("rp-mod")
    await _blog(client, owner, "rp-blog")
    post = await _post(client, owner, "rp-blog", "post")
    c = (await client.post(f"/api/v1/posts/{post['id']}/comments", json={"content": "dubbio"}, headers=reader.headers)).json()

    assert (await client.post(f"/api/v1/comments/{c['id']}/report", json={"note": "  "}, headers=owner.headers)).status_code == 400
    res = await client.post(f"/api/v1/comments/{c['id']}/report", json={"note": "contenuto illecito"}, headers=owner.headers)
    assert res.status_code == 200 and res.json()["reported_to_platform"] is True

    mine = await client.get("/api/v1/blogs/rp-blog/comments", params={"reported": "true"}, headers=owner.headers)
    assert [x["id"] for x in mine.json()] == [c["id"]]
    queue = await client.get("/api/v1/admin/comments", params={"reported": "true"}, headers=moderator.headers)
    assert queue.status_code == 200
    row = next(x for x in queue.json() if x["id"] == c["id"])
    assert row["report_note"] == "contenuto illecito" and row["reported_at"] is not None


async def test_comments_auto_close_after_days(client: AsyncClient, make_user: Callable, db_session: AsyncSession) -> None:
    owner: AuthedUser = await make_user("ac-owner")
    reader: AuthedUser = await make_user("ac-reader")
    await _blog(client, owner, "ac-blog")
    post = await _post(client, owner, "ac-blog", "vecchio")
    res = await client.patch("/api/v1/blogs/ac-blog", json={"comments_auto_close_days": 90}, headers=owner.headers)
    assert res.status_code == 200 and res.json()["comments_auto_close_days"] == 90
    assert (await client.patch("/api/v1/blogs/ac-blog", json={"comments_auto_close_days": -1}, headers=owner.headers)).status_code == 400

    assert (await client.post(f"/api/v1/posts/{post['id']}/comments", json={"content": "ok"}, headers=reader.headers)).status_code == 201
    await db_session.execute(update(Post).where(Post.id == post["id"]).values(published_at=datetime.now(timezone.utc) - timedelta(days=91)))
    await db_session.commit()
    assert (await client.post(f"/api/v1/posts/{post['id']}/comments", json={"content": "tardi"}, headers=reader.headers)).status_code == 403
    detail = await client.get(f"/api/v1/blogs/ac-blog/posts/vecchio")
    assert detail.json()["effective_comments_mode"] == "closed"
    assert (await db_session.execute(select(Post.id).where(Post.id == post["id"]))).scalar_one()
