"""GET /api/v1/users/me/{posts,media,publications,links,bibliography}: viste
aggregate su tutti i blog dell'utente (proprietà + membership), generalizzando
i corrispondenti endpoint per-blog. Ogni test copre un utente con due blog
(uno di proprietà, uno via membership) e verifica che il contenuto di
entrambi compaia, correttamente attribuito, e che il contenuto di un blog
altrui resti fuori."""

from collections.abc import Callable

from httpx import AsyncClient
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.blog import Blog, BlogMembership, BlogRole
from app.models.user import User
from tests.conftest import AuthedUser


async def _add_membership(db_session: AsyncSession, *, username: str, blog_slug: str, role: BlogRole) -> None:
    user = (await db_session.execute(select(User).where(User.username == username))).scalar_one()
    blog = (await db_session.execute(select(Blog).where(Blog.slug == blog_slug))).scalar_one()
    db_session.add(BlogMembership(user_id=user.id, blog_id=blog.id, role=role))
    await db_session.commit()


async def _create_blog(client: AsyncClient, owner: AuthedUser, slug: str) -> None:
    res = await client.post("/api/v1/blogs", json={"slug": slug, "title": slug}, headers=owner.headers)
    assert res.status_code == 201, res.text


async def _create_post(
    client: AsyncClient, author: AuthedUser, blog_slug: str, slug: str, *, content: str = "y" * 50
) -> dict:
    res = await client.post(
        f"/api/v1/blogs/{blog_slug}/posts",
        json={"slug": slug, "title": f"t-{slug}", "content": content},
        headers=author.headers,
    )
    assert res.status_code == 201, res.text
    return res.json()


async def _setup_two_blogs(client: AsyncClient, make_user: Callable, db_session: AsyncSession, tag: str):
    """Utente `user` proprietario di `own-{tag}` e collaboratore (via
    BlogMembership) su `member-{tag}`, di proprietà di un altro utente.
    Ritorna (user, own_slug, member_slug, other: AuthedUser proprietario di un
    terzo blog non correlato)."""
    user: AuthedUser = await make_user(f"agg-{tag}")
    member_owner: AuthedUser = await make_user(f"agg-memowner-{tag}")
    stranger: AuthedUser = await make_user(f"agg-stranger-{tag}")

    own_slug = f"own-{tag}"
    member_slug = f"member-{tag}"
    stranger_slug = f"stranger-{tag}"

    await _create_blog(client, user, own_slug)
    await _create_blog(client, member_owner, member_slug)
    await _create_blog(client, stranger, stranger_slug)
    await _add_membership(db_session, username=user.username, blog_slug=member_slug, role=BlogRole.CO_AUTORE)

    return user, member_owner, own_slug, member_slug, stranger, stranger_slug


async def test_my_posts_spans_owned_and_member_blogs_not_others(
    client: AsyncClient, make_user: Callable, db_session: AsyncSession
) -> None:
    user, member_owner, own_slug, member_slug, stranger, stranger_slug = await _setup_two_blogs(
        client, make_user, db_session, "posts"
    )
    await _create_post(client, user, own_slug, "own-post")
    await _create_post(client, user, member_slug, "member-post")
    await _create_post(client, stranger, stranger_slug, "stranger-post")

    res = await client.get("/api/v1/users/me/posts", headers=user.headers)
    assert res.status_code == 200, res.text
    items = res.json()
    slugs = {(i["blog_slug"], i["slug"]) for i in items}
    assert (own_slug, "own-post") in slugs
    assert (member_slug, "member-post") in slugs
    assert not any(blog_slug == stranger_slug for blog_slug, _ in slugs)


async def test_my_media_spans_owned_and_member_blogs_not_others(
    client: AsyncClient, make_user: Callable, db_session: AsyncSession
) -> None:
    from app.models.media_file import MediaFile

    user, member_owner, own_slug, member_slug, stranger, stranger_slug = await _setup_two_blogs(
        client, make_user, db_session, "media"
    )
    own_blog = (await db_session.execute(select(Blog).where(Blog.slug == own_slug))).scalar_one()
    member_blog = (await db_session.execute(select(Blog).where(Blog.slug == member_slug))).scalar_one()
    stranger_blog = (await db_session.execute(select(Blog).where(Blog.slug == stranger_slug))).scalar_one()

    db_session.add(MediaFile(blog_id=own_blog.id, url="https://example.com/own.png", alt_text=""))
    db_session.add(MediaFile(blog_id=member_blog.id, url="https://example.com/member.png", alt_text=""))
    db_session.add(MediaFile(blog_id=stranger_blog.id, url="https://example.com/stranger.png", alt_text=""))
    await db_session.commit()

    res = await client.get("/api/v1/users/me/media", headers=user.headers)
    assert res.status_code == 200, res.text
    items = res.json()
    urls = {(i["blog_slug"], i["url"]) for i in items}
    assert (own_slug, "https://example.com/own.png") in urls
    assert (member_slug, "https://example.com/member.png") in urls
    assert not any(blog_slug == stranger_slug for blog_slug, _ in urls)


async def test_my_publications_spans_owned_and_member_blogs_not_others(
    client: AsyncClient, make_user: Callable, db_session: AsyncSession
) -> None:
    user, member_owner, own_slug, member_slug, stranger, stranger_slug = await _setup_two_blogs(
        client, make_user, db_session, "pubs"
    )
    own_res = await client.post(
        f"/api/v1/blogs/{own_slug}/publications",
        json={"name": "own-pub", "title": "Own Pub"},
        headers=user.headers,
    )
    assert own_res.status_code == 201, own_res.text
    member_res = await client.post(
        f"/api/v1/blogs/{member_slug}/publications",
        json={"name": "member-pub", "title": "Member Pub"},
        headers=user.headers,
    )
    assert member_res.status_code == 201, member_res.text
    stranger_res = await client.post(
        f"/api/v1/blogs/{stranger_slug}/publications",
        json={"name": "stranger-pub", "title": "Stranger Pub"},
        headers=stranger.headers,
    )
    assert stranger_res.status_code == 201, stranger_res.text

    res = await client.get("/api/v1/users/me/publications", headers=user.headers)
    assert res.status_code == 200, res.text
    items = res.json()
    names = {(i["blog_slug"], i["name"]) for i in items}
    assert (own_slug, "own-pub") in names
    assert (member_slug, "member-pub") in names
    assert not any(blog_slug == stranger_slug for blog_slug, _ in names)


async def test_my_links_spans_owned_and_member_blogs_not_others(
    client: AsyncClient, make_user: Callable, db_session: AsyncSession
) -> None:
    user, member_owner, own_slug, member_slug, stranger, stranger_slug = await _setup_two_blogs(
        client, make_user, db_session, "links"
    )
    await _create_post(
        client, user, own_slug, "own-link-post", content="Testo [link proprio](https://example.com/own-link) qui."
    )
    await _create_post(
        client,
        user,
        member_slug,
        "member-link-post",
        content="Testo [link membership](https://example.com/member-link) qui.",
    )
    await _create_post(
        client,
        stranger,
        stranger_slug,
        "stranger-link-post",
        content="Testo [link estraneo](https://example.com/stranger-link) qui.",
    )

    res = await client.get("/api/v1/users/me/links", headers=user.headers)
    assert res.status_code == 200, res.text
    items = res.json()
    urls = {entry["url"] for entry in items}
    assert "https://example.com/own-link" in urls
    assert "https://example.com/member-link" in urls
    assert "https://example.com/stranger-link" not in urls

    own_entry = next(e for e in items if e["url"] == "https://example.com/own-link")
    assert own_entry["citations"][0]["blog_slug"] == own_slug
    member_entry = next(e for e in items if e["url"] == "https://example.com/member-link")
    assert member_entry["citations"][0]["blog_slug"] == member_slug


async def test_my_bibliography_spans_owned_and_member_blogs_not_others(
    client: AsyncClient, make_user: Callable, db_session: AsyncSession
) -> None:
    user, member_owner, own_slug, member_slug, stranger, stranger_slug = await _setup_two_blogs(
        client, make_user, db_session, "biblio"
    )

    own_res = await client.post(
        f"/api/v1/blogs/{own_slug}/posts",
        json={
            "slug": "own-note-post",
            "title": "t",
            "content": "Testo con nota [1](#nota-1).",
            "notes": [{"idx": 1, "content": "Nota del blog di proprietà."}],
        },
        headers=user.headers,
    )
    assert own_res.status_code == 201, own_res.text

    member_res = await client.post(
        f"/api/v1/blogs/{member_slug}/posts",
        json={
            "slug": "member-note-post",
            "title": "t",
            "content": "Testo con nota [1](#nota-1).",
            "notes": [{"idx": 1, "content": "Nota del blog in membership."}],
        },
        headers=user.headers,
    )
    assert member_res.status_code == 201, member_res.text

    stranger_res = await client.post(
        f"/api/v1/blogs/{stranger_slug}/posts",
        json={
            "slug": "stranger-note-post",
            "title": "t",
            "content": "Testo con nota [1](#nota-1).",
            "notes": [{"idx": 1, "content": "Nota di un blog estraneo."}],
        },
        headers=stranger.headers,
    )
    assert stranger_res.status_code == 201, stranger_res.text

    res = await client.get("/api/v1/users/me/bibliography", headers=user.headers)
    assert res.status_code == 200, res.text
    items = res.json()
    contents = {entry["content"] for entry in items}
    assert "Nota del blog di proprietà." in contents
    assert "Nota del blog in membership." in contents
    assert "Nota di un blog estraneo." not in contents

    own_entry = next(e for e in items if e["content"] == "Nota del blog di proprietà.")
    assert own_entry["citations"][0]["blog_slug"] == own_slug
