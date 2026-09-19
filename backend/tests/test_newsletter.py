import uuid
from collections.abc import Callable

from httpx import AsyncClient
from sqlalchemy import select

from app.domain.newsletter import sign_unsubscribe_token
from app.models.blog import Blog, BlogMembership, BlogRole
from app.models.newsletter import NewsletterCampaign, NewsletterSubscriber, NewsletterSubscriberStatus
from app.models.user import User
from tests.conftest import AuthedUser


async def _create_blog(client: AsyncClient, owner: AuthedUser, slug: str) -> str:
    res = await client.post("/api/v1/blogs", json={"slug": slug, "title": "Blog di prova"}, headers=owner.headers)
    assert res.status_code == 201, res.text
    return slug


async def _invite_and_accept(
    client: AsyncClient, owner: AuthedUser, guest: AuthedUser, slug: str, role: str
) -> None:
    invite = await client.post(
        f"/api/v1/blogs/{slug}/invitations",
        json={"username": guest.username, "role": role},
        headers=owner.headers,
    )
    assert invite.status_code == 201, invite.text
    accept = await client.post(
        f"/api/v1/blogs/received-invitations/{invite.json()['id']}/accept", headers=guest.headers
    )
    assert accept.status_code == 200, accept.text


async def test_subscribe_creates_pending_and_queues_confirmation(
    client: AsyncClient, make_user: Callable, db_session, captured_newsletter_confirmations: list[dict]
) -> None:
    owner: AuthedUser = await make_user("newsletter-owner-1")
    slug = await _create_blog(client, owner, "blog-newsletter-1")

    res = await client.post(
        "/api/v1/newsletter/subscribe",
        json={"email": "lettore@example.com", "blog_slug": slug},
    )
    assert res.status_code == 202, res.text
    assert res.json() == {"status": "ok"}

    assert len(captured_newsletter_confirmations) == 1
    assert captured_newsletter_confirmations[0]["email"] == "lettore@example.com"
    assert captured_newsletter_confirmations[0]["token"]

    result = await db_session.execute(
        select(NewsletterSubscriber).where(NewsletterSubscriber.email == "lettore@example.com")
    )
    subscriber = result.scalar_one()
    assert subscriber.status == NewsletterSubscriberStatus.PENDING
    assert subscriber.confirm_token_hash is not None
    assert subscriber.confirm_token_expires_at is not None
    assert subscriber.blog_id is not None


async def test_subscribe_platform_digest_no_blog_slug(
    client: AsyncClient, db_session, captured_newsletter_confirmations: list[dict]
) -> None:
    res = await client.post("/api/v1/newsletter/subscribe", json={"email": "digest@example.com"})
    assert res.status_code == 202

    result = await db_session.execute(
        select(NewsletterSubscriber).where(NewsletterSubscriber.email == "digest@example.com")
    )
    subscriber = result.scalar_one()
    assert subscriber.blog_id is None
    assert captured_newsletter_confirmations[0]["list_label"] == "Notturni"


async def test_confirm_is_idempotent(
    client: AsyncClient, make_user: Callable, db_session, captured_newsletter_confirmations: list[dict]
) -> None:
    owner: AuthedUser = await make_user("newsletter-owner-2")
    slug = await _create_blog(client, owner, "blog-newsletter-2")

    await client.post(
        "/api/v1/newsletter/subscribe", json={"email": "confirm@example.com", "blog_slug": slug}
    )
    token = captured_newsletter_confirmations[0]["token"]

    first = await client.get("/api/v1/newsletter/confirm", params={"token": token})
    assert first.status_code == 200
    assert first.json() == {"status": "confirmed"}

    second = await client.get("/api/v1/newsletter/confirm", params={"token": token})
    assert second.status_code == 200
    assert second.json() == {"status": "already_confirmed"}

    invalid = await client.get("/api/v1/newsletter/confirm", params={"token": "token-inesistente"})
    assert invalid.status_code == 200
    assert invalid.json() == {"status": "invalid"}

    result = await db_session.execute(
        select(NewsletterSubscriber).where(NewsletterSubscriber.email == "confirm@example.com")
    )
    subscriber = result.scalar_one()
    assert subscriber.status == NewsletterSubscriberStatus.CONFIRMED
    assert subscriber.confirmed_at is not None


async def test_unsubscribe_via_token(
    client: AsyncClient, make_user: Callable, db_session, captured_newsletter_confirmations: list[dict]
) -> None:
    owner: AuthedUser = await make_user("newsletter-owner-3")
    slug = await _create_blog(client, owner, "blog-newsletter-3")
    await client.post(
        "/api/v1/newsletter/subscribe", json={"email": "unsub@example.com", "blog_slug": slug}
    )
    token = captured_newsletter_confirmations[0]["token"]
    await client.get("/api/v1/newsletter/confirm", params={"token": token})

    result = await db_session.execute(
        select(NewsletterSubscriber).where(NewsletterSubscriber.email == "unsub@example.com")
    )
    subscriber = result.scalar_one()
    unsubscribe_token = sign_unsubscribe_token(subscriber.id)

    res = await client.post(
        "/api/v1/newsletter/unsubscribe", json={"token": unsubscribe_token, "reason": "troppe email"}
    )
    assert res.status_code == 200
    assert res.json() == {"status": "ok"}

    await db_session.refresh(subscriber)
    assert subscriber.status == NewsletterSubscriberStatus.UNSUBSCRIBED
    assert subscriber.unsubscribed_at is not None
    assert subscriber.unsubscribe_reason == "troppe email"

    invalid = await client.post("/api/v1/newsletter/unsubscribe", json={"token": "garbage"})
    assert invalid.status_code == 400


async def test_unsubscribe_delete_removes_subscriber(
    client: AsyncClient, make_user: Callable, db_session, captured_newsletter_confirmations: list[dict]
) -> None:
    owner: AuthedUser = await make_user("newsletter-owner-4")
    slug = await _create_blog(client, owner, "blog-newsletter-4")
    await client.post(
        "/api/v1/newsletter/subscribe", json={"email": "delete-me@example.com", "blog_slug": slug}
    )
    result = await db_session.execute(
        select(NewsletterSubscriber).where(NewsletterSubscriber.email == "delete-me@example.com")
    )
    subscriber = result.scalar_one()
    token = sign_unsubscribe_token(subscriber.id)

    res = await client.post("/api/v1/newsletter/unsubscribe/delete", json={"token": token})
    assert res.status_code == 200

    result_after = await db_session.execute(
        select(NewsletterSubscriber).where(NewsletterSubscriber.email == "delete-me@example.com")
    )
    assert result_after.scalar_one_or_none() is None

    # idempotente: un secondo tentativo con lo stesso token non fallisce
    again = await client.post("/api/v1/newsletter/unsubscribe/delete", json={"token": token})
    assert again.status_code == 200


async def test_subscribe_rate_limited_per_email(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("newsletter-owner-5")
    slug = await _create_blog(client, owner, "blog-newsletter-5")
    email = f"ratelimited-{uuid.uuid4()}@example.com"

    # SUBSCRIBE_EMAIL_RATE_LIMIT=3 (app/api/v1/newsletter.py)
    for _ in range(3):
        res = await client.post(
            "/api/v1/newsletter/subscribe", json={"email": email, "blog_slug": slug}
        )
        assert res.status_code == 202

    blocked = await client.post(
        "/api/v1/newsletter/subscribe", json={"email": email, "blog_slug": slug}
    )
    assert blocked.status_code == 429


async def test_can_manage_newsletter_permission_boundary(
    client: AsyncClient, make_user: Callable, db_session
) -> None:
    owner: AuthedUser = await make_user("newsletter-owner-6")
    co_autore: AuthedUser = await make_user("newsletter-co-autore")
    revisore: AuthedUser = await make_user("newsletter-revisore")
    mediatore: AuthedUser = await make_user("newsletter-mediatore")
    stranger: AuthedUser = await make_user("newsletter-stranger")
    slug = await _create_blog(client, owner, "blog-newsletter-6")

    await _invite_and_accept(client, owner, co_autore, slug, "co_autore")
    await _invite_and_accept(client, owner, mediatore, slug, "mediatore")
    # l'endpoint di invito ammette solo co_autore/mediatore (app/api/v1/blogs/
    # invitations.py): il ruolo revisore per questo test va assegnato
    # direttamente, come fa la fixture make_admin per il ruolo di piattaforma.
    blog = (await db_session.execute(select(Blog).where(Blog.slug == slug))).scalar_one()
    revisore_user = (
        await db_session.execute(select(User).where(User.username == revisore.username))
    ).scalar_one()
    db_session.add(BlogMembership(user_id=revisore_user.id, blog_id=blog.id, role=BlogRole.REVISORE))
    await db_session.commit()

    owner_res = await client.get(f"/api/v1/blogs/{slug}/newsletter/stats", headers=owner.headers)
    assert owner_res.status_code == 200

    co_autore_res = await client.get(f"/api/v1/blogs/{slug}/newsletter/stats", headers=co_autore.headers)
    assert co_autore_res.status_code == 200

    revisore_res = await client.get(f"/api/v1/blogs/{slug}/newsletter/stats", headers=revisore.headers)
    assert revisore_res.status_code == 403

    mediatore_res = await client.get(f"/api/v1/blogs/{slug}/newsletter/stats", headers=mediatore.headers)
    assert mediatore_res.status_code == 403

    stranger_res = await client.get(f"/api/v1/blogs/{slug}/newsletter/stats", headers=stranger.headers)
    assert stranger_res.status_code == 403


async def test_post_publish_hook_creates_one_campaign_even_if_called_twice(
    client: AsyncClient, make_user: Callable, db_session, captured_newsletter_campaigns: list[str]
) -> None:
    owner: AuthedUser = await make_user("newsletter-owner-7")
    slug = await _create_blog(client, owner, "blog-newsletter-7")

    post_res = await client.post(
        f"/api/v1/blogs/{slug}/posts",
        json={"slug": "post-newsletter", "title": "Un nuovo post", "content": "Contenuto del post."},
        headers=owner.headers,
    )
    assert post_res.status_code == 201, post_res.text
    post_id = post_res.json()["id"]

    first_publish = await client.post(f"/api/v1/posts/{post_id}/publish", headers=owner.headers)
    assert first_publish.status_code == 200
    assert len(captured_newsletter_campaigns) == 1

    # ripubblicare con un published_at esplicito bypassa il no-op idempotente
    # di publish_post e richiama di nuovo l'hook di accodamento della newsletter
    second_publish = await client.post(
        f"/api/v1/posts/{post_id}/publish",
        json={"published_at": "2026-01-01T00:00:00Z"},
        headers=owner.headers,
    )
    assert second_publish.status_code == 200

    result = await db_session.execute(
        select(NewsletterCampaign).where(NewsletterCampaign.post_id == uuid.UUID(post_id))
    )
    campaigns = result.scalars().all()
    assert len(campaigns) == 1
    # solo il primo giro ha davvero accodato una campagna sul broker
    assert len(captured_newsletter_campaigns) == 1


async def test_blog_newsletter_settings_toggle(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("newsletter-owner-8")
    slug = await _create_blog(client, owner, "blog-newsletter-8")

    res = await client.patch(
        f"/api/v1/blogs/{slug}/newsletter/settings",
        json={"newsletter_auto_notify_enabled": False},
        headers=owner.headers,
    )
    assert res.status_code == 200
    assert res.json() == {"newsletter_auto_notify_enabled": False}
