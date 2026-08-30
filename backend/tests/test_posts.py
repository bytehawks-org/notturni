from collections.abc import Callable
from datetime import datetime, timedelta, timezone

from httpx import AsyncClient
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.blog import Blog, BlogMembership, BlogRole
from app.models.user import User
from tests.conftest import AuthedUser


async def _add_membership(
    db_session: AsyncSession, *, username: str, blog_slug: str, role: BlogRole
) -> None:
    user = (
        await db_session.execute(select(User).where(User.username == username))
    ).scalar_one()
    blog = (
        await db_session.execute(select(Blog).where(Blog.slug == blog_slug))
    ).scalar_one()
    db_session.add(BlogMembership(user_id=user.id, blog_id=blog.id, role=role))
    await db_session.commit()


async def _create_blog(client: AsyncClient, owner: AuthedUser, slug: str = "blog-post-test") -> str:
    res = await client.post("/api/v1/blogs", json={"slug": slug, "title": "x"}, headers=owner.headers)
    assert res.status_code == 201
    return slug


async def test_owner_sees_drafts_in_list_stranger_does_not(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user("owner-list-draft")
    stranger: AuthedUser = await make_user("stranger-list-draft")
    slug = await _create_blog(client, owner, "blog-list-draft-test")
    await client.post(
        f"/api/v1/blogs/{slug}/posts",
        json={"slug": "bozza", "title": "x", "content": "y"},
        headers=owner.headers,
    )

    anon_res = await client.get(f"/api/v1/blogs/{slug}/posts")
    assert anon_res.json() == []

    stranger_res = await client.get(f"/api/v1/blogs/{slug}/posts", headers=stranger.headers)
    assert stranger_res.json() == []

    owner_res = await client.get(f"/api/v1/blogs/{slug}/posts", headers=owner.headers)
    assert len(owner_res.json()) == 1
    assert owner_res.json()[0]["status"] == "draft"


async def test_create_post_requires_write_access(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner3")
    stranger: AuthedUser = await make_user("stranger3")
    slug = await _create_blog(client, owner)

    res = await client.post(
        f"/api/v1/blogs/{slug}/posts",
        json={"slug": "intruso", "title": "x", "content": "y"},
        headers=stranger.headers,
    )
    assert res.status_code == 403


async def test_post_draft_publish_flow(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner4")
    slug = await _create_blog(client, owner, "blog-draft-test")

    create_res = await client.post(
        f"/api/v1/blogs/{slug}/posts",
        json={"slug": "primo-post", "title": "Titolo", "content": "Corpo"},
        headers=owner.headers,
    )
    assert create_res.status_code == 201
    post = create_res.json()
    assert post["status"] == "draft"
    assert post["locale"] == "it"
    post_id = post["id"]

    # non ancora pubblicato: non compare nella lista pubblica
    list_res = await client.get(f"/api/v1/blogs/{slug}/posts")
    assert list_res.json() == []

    # una bozza è visibile via detail solo a chi ha accesso in scrittura
    anon_detail = await client.get(f"/api/v1/posts/{post_id}")
    assert anon_detail.status_code == 404
    owner_detail = await client.get(f"/api/v1/posts/{post_id}", headers=owner.headers)
    assert owner_detail.status_code == 200

    publish_res = await client.post(f"/api/v1/posts/{post_id}/publish", headers=owner.headers)
    assert publish_res.status_code == 200
    assert publish_res.json()["status"] == "published"
    published_at = publish_res.json()["published_at"]

    # ripubblicare non deve cambiare published_at
    republish_res = await client.post(f"/api/v1/posts/{post_id}/publish", headers=owner.headers)
    assert republish_res.json()["published_at"] == published_at

    list_res2 = await client.get(f"/api/v1/blogs/{slug}/posts")
    assert len(list_res2.json()) == 1

    anon_detail2 = await client.get(f"/api/v1/posts/{post_id}")
    assert anon_detail2.status_code == 200


async def test_update_post(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner5")
    slug = await _create_blog(client, owner, "blog-update-test")
    create_res = await client.post(
        f"/api/v1/blogs/{slug}/posts",
        json={"slug": "modifica-post", "title": "A", "content": "B"},
        headers=owner.headers,
    )
    post_id = create_res.json()["id"]

    update_res = await client.patch(
        f"/api/v1/posts/{post_id}", json={"title": "Nuovo titolo"}, headers=owner.headers
    )
    assert update_res.status_code == 200
    assert update_res.json()["title"] == "Nuovo titolo"
    assert update_res.json()["content"] == "B"


async def test_post_cover_image_url(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner-cover")
    slug = await _create_blog(client, owner, "blog-cover-test")

    create_res = await client.post(
        f"/api/v1/blogs/{slug}/posts",
        json={"slug": "post-cover", "title": "A", "content": "B", "cover_image_url": "https://x/1.png"},
        headers=owner.headers,
    )
    assert create_res.status_code == 201
    post_id = create_res.json()["id"]
    assert create_res.json()["cover_image_url"] == "https://x/1.png"

    # omesso: la cover resta invariata
    no_touch_res = await client.patch(
        f"/api/v1/posts/{post_id}", json={"title": "C"}, headers=owner.headers
    )
    assert no_touch_res.json()["cover_image_url"] == "https://x/1.png"

    # nuovo URL: la sostituisce
    replace_res = await client.patch(
        f"/api/v1/posts/{post_id}", json={"cover_image_url": "https://x/2.png"}, headers=owner.headers
    )
    assert replace_res.json()["cover_image_url"] == "https://x/2.png"

    # stringa vuota: la rimuove
    remove_res = await client.patch(
        f"/api/v1/posts/{post_id}", json={"cover_image_url": ""}, headers=owner.headers
    )
    assert remove_res.json()["cover_image_url"] is None

    # creazione senza cover_image_url: resta null
    no_cover_res = await client.post(
        f"/api/v1/blogs/{slug}/posts",
        json={"slug": "post-senza-cover", "title": "A", "content": "B"},
        headers=owner.headers,
    )
    assert no_cover_res.json()["cover_image_url"] is None


async def test_post_translations(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner6")
    slug = await _create_blog(client, owner, "blog-i18n-test")

    it_res = await client.post(
        f"/api/v1/blogs/{slug}/posts",
        json={"slug": "il-mio-post", "title": "Il mio post", "content": "Contenuto"},
        headers=owner.headers,
    )
    post_id = it_res.json()["id"]

    en_res = await client.post(
        f"/api/v1/posts/{post_id}/translations",
        json={"slug": "my-post", "locale": "en", "title": "My post", "content": "Content"},
        headers=owner.headers,
    )
    assert en_res.status_code == 201
    en_post_id = en_res.json()["id"]
    assert en_res.json()["translation_group_id"] == it_res.json()["translation_group_id"]

    # stessa lingua due volte -> conflitto
    dup_res = await client.post(
        f"/api/v1/posts/{post_id}/translations",
        json={"slug": "my-post-2", "locale": "en", "title": "x", "content": "y"},
        headers=owner.headers,
    )
    assert dup_res.status_code == 409

    await client.post(f"/api/v1/posts/{post_id}/publish", headers=owner.headers)
    await client.post(f"/api/v1/posts/{en_post_id}/publish", headers=owner.headers)

    translations_res = await client.get(f"/api/v1/posts/{post_id}/translations")
    locales = {t["locale"] for t in translations_res.json()}
    assert locales == {"it", "en"}

    it_list = await client.get(f"/api/v1/blogs/{slug}/posts", params={"locale": "it"})
    assert [p["slug"] for p in it_list.json()] == ["il-mio-post"]

    en_list = await client.get(f"/api/v1/blogs/{slug}/posts", params={"locale": "en"})
    assert [p["slug"] for p in en_list.json()] == ["my-post"]

    all_list = await client.get(f"/api/v1/blogs/{slug}/posts")
    assert len(all_list.json()) == 2


async def test_post_default_locale_from_blog(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner7")
    res = await client.post(
        "/api/v1/blogs",
        json={"slug": "blog-locale-test", "title": "x", "default_locale": "de"},
        headers=owner.headers,
    )
    assert res.status_code == 201

    post_res = await client.post(
        "/api/v1/blogs/blog-locale-test/posts",
        json={"slug": "post-tedesco", "title": "x", "content": "y"},
        headers=owner.headers,
    )
    assert post_res.json()["locale"] == "de"


async def test_review_workflow(
    client: AsyncClient, make_user: Callable, db_session: AsyncSession
) -> None:
    owner: AuthedUser = await make_user("owner-review")
    reviewer: AuthedUser = await make_user("reviewer1")
    stranger: AuthedUser = await make_user("stranger-review")
    slug = await _create_blog(client, owner, "blog-review-test")

    post_res = await client.post(
        f"/api/v1/blogs/{slug}/posts",
        json={"slug": "post-review", "title": "x", "content": "y"},
        headers=owner.headers,
    )
    post_id = post_res.json()["id"]

    submit_res = await client.post(f"/api/v1/posts/{post_id}/submit-for-review", headers=owner.headers)
    assert submit_res.status_code == 200
    assert submit_res.json()["status"] == "pending_review"

    # non si può rimandare in revisione due volte
    resubmit_res = await client.post(f"/api/v1/posts/{post_id}/submit-for-review", headers=owner.headers)
    assert resubmit_res.status_code == 400

    # il revisore non è ancora membro del blog: non può approvare né rifiutare
    forbidden_publish = await client.post(f"/api/v1/posts/{post_id}/publish", headers=reviewer.headers)
    assert forbidden_publish.status_code == 403
    forbidden_return = await client.post(f"/api/v1/posts/{post_id}/return-to-draft", headers=reviewer.headers)
    assert forbidden_return.status_code == 403

    # uno sconosciuto non può nulla neanche dopo
    stranger_res = await client.post(f"/api/v1/posts/{post_id}/publish", headers=stranger.headers)
    assert stranger_res.status_code == 403

    await _add_membership(db_session, username="reviewer1", blog_slug=slug, role=BlogRole.REVISORE)

    return_res = await client.post(f"/api/v1/posts/{post_id}/return-to-draft", headers=reviewer.headers)
    assert return_res.status_code == 200
    assert return_res.json()["status"] == "draft"

    # non si può rimandare in bozza un post che non è in revisione
    return_again_res = await client.post(f"/api/v1/posts/{post_id}/return-to-draft", headers=reviewer.headers)
    assert return_again_res.status_code == 400

    await client.post(f"/api/v1/posts/{post_id}/submit-for-review", headers=owner.headers)
    approve_res = await client.post(f"/api/v1/posts/{post_id}/publish", headers=reviewer.headers)
    assert approve_res.status_code == 200
    assert approve_res.json()["status"] == "published"


async def test_scheduled_publish_not_yet_publicly_visible(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner-sched")
    slug = await _create_blog(client, owner, "blog-sched-test")

    post_res = await client.post(
        f"/api/v1/blogs/{slug}/posts",
        json={"slug": "post-sched", "title": "x", "content": "y"},
        headers=owner.headers,
    )
    post_id = post_res.json()["id"]

    future = (datetime.now(timezone.utc) + timedelta(days=1)).isoformat()
    publish_res = await client.post(
        f"/api/v1/posts/{post_id}/publish", json={"published_at": future}, headers=owner.headers
    )
    assert publish_res.status_code == 200
    assert publish_res.json()["status"] == "published"

    # "published" ma non ancora pubblicamente visibile: la data è futura
    public_list = await client.get(f"/api/v1/blogs/{slug}/posts")
    assert public_list.json() == []
    public_detail = await client.get(f"/api/v1/posts/{post_id}")
    assert public_detail.status_code == 404

    # l'owner lo vede comunque
    owner_list = await client.get(f"/api/v1/blogs/{slug}/posts", headers=owner.headers)
    assert len(owner_list.json()) == 1
    owner_detail = await client.get(f"/api/v1/posts/{post_id}", headers=owner.headers)
    assert owner_detail.status_code == 200

    # pubblicare di nuovo senza published_at esplicito non cambia la pianificazione
    republish_res = await client.post(f"/api/v1/posts/{post_id}/publish", headers=owner.headers)
    assert republish_res.json()["published_at"] == publish_res.json()["published_at"]

    # una nuova data (anche nel passato) sovrascrive la pianificazione
    past = (datetime.now(timezone.utc) - timedelta(minutes=1)).isoformat()
    republish2_res = await client.post(
        f"/api/v1/posts/{post_id}/publish", json={"published_at": past}, headers=owner.headers
    )
    assert republish2_res.status_code == 200
    public_list2 = await client.get(f"/api/v1/blogs/{slug}/posts")
    assert len(public_list2.json()) == 1


async def test_post_save_triggers_s3_backup(
    client: AsyncClient, make_user: Callable, captured_post_backups: list[dict]
) -> None:
    owner: AuthedUser = await make_user("owner-backup")
    slug = await _create_blog(client, owner, "blog-backup-test")

    post_res = await client.post(
        f"/api/v1/blogs/{slug}/posts",
        json={"slug": "post-backup", "title": "x", "content": "# Markdown"},
        headers=owner.headers,
    )
    post_id = post_res.json()["id"]
    assert len(captured_post_backups) == 1
    assert captured_post_backups[0]["post_id"] == post_id
    assert captured_post_backups[0]["content"] == "# Markdown"

    await client.patch(
        f"/api/v1/posts/{post_id}", json={"content": "# Modificato"}, headers=owner.headers
    )
    assert len(captured_post_backups) == 2
    assert captured_post_backups[1]["content"] == "# Modificato"
