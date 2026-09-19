"""Newsletter/mailing-list (app/models/newsletter.py): iscrizione pubblica
(double opt-in), disiscrizione/cancellazione self-service senza login, e
gestione delle campagne per chi ha diritti di scrittura su un blog o è
amministratore di piattaforma (digest, `blog_id=None`)."""

import uuid
from datetime import datetime, timezone

from fastapi import APIRouter, Depends, HTTPException, Request, status
from pydantic import BaseModel, EmailStr, field_validator
from sqlalchemy import func, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user, get_optional_current_user, require_platform_admin
from app.core.broker import publish_newsletter_campaign, publish_newsletter_confirmation
from app.core.database import get_session
from app.core.http import client_ip
from app.core.security import sha256_hex
from app.domain.authorization import can_manage_newsletter, can_view_blog
from app.domain.newsletter import (
    confirm_token_expiry,
    generate_confirm_token,
    verify_unsubscribe_token,
)
from app.domain.rate_limit import enforce_rate_limit
from app.models.blog import Blog
from app.models.newsletter import (
    NewsletterCampaign,
    NewsletterCampaignKind,
    NewsletterCampaignStatus,
    NewsletterSubscriber,
    NewsletterSubscriberStatus,
)
from app.models.user import User

router = APIRouter()

# app/domain/rate_limit.py: protezione aggiuntiva contro l'abuso
# dell'endpoint pubblico di iscrizione (spam su email altrui, enumerazione).
SUBSCRIBE_IP_RATE_LIMIT = 5
SUBSCRIBE_EMAIL_RATE_LIMIT = 3
SUBSCRIBE_RATE_LIMIT_WINDOW_SECONDS = 3600


class SubscribeRequest(BaseModel):
    email: EmailStr
    blog_slug: str | None = None
    locale: str | None = None


class GenericStatusResponse(BaseModel):
    status: str = "ok"


class ConfirmResponse(BaseModel):
    status: str  # "confirmed" | "already_confirmed" | "invalid"


class UnsubscribeRequest(BaseModel):
    token: str
    reason: str | None = None


class UnsubscribeDeleteRequest(BaseModel):
    token: str


class NewsletterStatsOut(BaseModel):
    pending: int
    confirmed: int
    unsubscribed: int


class CampaignOut(BaseModel):
    id: uuid.UUID
    blog_id: uuid.UUID | None
    kind: NewsletterCampaignKind
    post_id: uuid.UUID | None
    subject: str
    body_markdown: str | None
    status: NewsletterCampaignStatus
    scheduled_at: datetime | None
    sent_at: datetime | None
    recipient_count: int
    failed_count: int
    created_at: datetime

    model_config = {"from_attributes": True}


class CampaignCreateRequest(BaseModel):
    subject: str
    body_markdown: str
    scheduled_at: datetime | None = None

    @field_validator("scheduled_at")
    @classmethod
    def _scheduled_at_must_be_aware(cls, value: datetime | None) -> datetime | None:
        # confrontato con datetime.now(timezone.utc) più sotto: un valore
        # naive (senza offset, es. "2026-01-01T10:00:00") solleverebbe
        # TypeError a runtime invece di un 422 leggibile per il client.
        if value is not None and value.tzinfo is None:
            raise ValueError("scheduled_at deve includere il fuso orario (es. suffisso 'Z' o '+00:00').")
        return value


class NewsletterSettingsRequest(BaseModel):
    newsletter_auto_notify_enabled: bool


class NewsletterSettingsOut(BaseModel):
    newsletter_auto_notify_enabled: bool


async def _get_blog_or_404(session: AsyncSession, blog_slug: str) -> Blog:
    result = await session.execute(select(Blog).where(Blog.slug == blog_slug))
    blog = result.scalar_one_or_none()
    if blog is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")
    return blog


async def _require_manage_access(session: AsyncSession, user: User, blog: Blog) -> None:
    if not await can_manage_newsletter(session, user_id=user.id, blog=blog):
        raise HTTPException(
            status.HTTP_403_FORBIDDEN,
            "Serve essere proprietario del blog o avere ruolo autore/co-autore.",
        )


async def _find_subscriber(
    session: AsyncSession, *, blog_id: uuid.UUID | None, email: str
) -> NewsletterSubscriber | None:
    stmt = select(NewsletterSubscriber).where(func.lower(NewsletterSubscriber.email) == email.lower())
    stmt = stmt.where(
        NewsletterSubscriber.blog_id.is_(None) if blog_id is None else NewsletterSubscriber.blog_id == blog_id
    )
    return (await session.execute(stmt)).scalar_one_or_none()


async def _stats_for(session: AsyncSession, blog_id: uuid.UUID | None) -> NewsletterStatsOut:
    stmt = select(NewsletterSubscriber.status, func.count()).group_by(NewsletterSubscriber.status)
    stmt = stmt.where(
        NewsletterSubscriber.blog_id.is_(None) if blog_id is None else NewsletterSubscriber.blog_id == blog_id
    )
    counts = {row[0]: row[1] for row in (await session.execute(stmt)).all()}
    return NewsletterStatsOut(
        pending=counts.get(NewsletterSubscriberStatus.PENDING, 0),
        confirmed=counts.get(NewsletterSubscriberStatus.CONFIRMED, 0),
        unsubscribed=counts.get(NewsletterSubscriberStatus.UNSUBSCRIBED, 0),
    )


async def _campaigns_for(session: AsyncSession, blog_id: uuid.UUID | None) -> list[CampaignOut]:
    stmt = select(NewsletterCampaign).order_by(NewsletterCampaign.created_at.desc())
    stmt = stmt.where(
        NewsletterCampaign.blog_id.is_(None) if blog_id is None else NewsletterCampaign.blog_id == blog_id
    )
    campaigns = (await session.execute(stmt)).scalars().all()
    return [CampaignOut.model_validate(c) for c in campaigns]


async def _create_manual_campaign(
    session: AsyncSession, *, blog_id: uuid.UUID | None, created_by: User, payload: CampaignCreateRequest
) -> CampaignOut:
    is_immediate = payload.scheduled_at is None or payload.scheduled_at <= datetime.now(timezone.utc)
    campaign = NewsletterCampaign(
        blog_id=blog_id,
        kind=NewsletterCampaignKind.MANUAL,
        created_by_id=created_by.id,
        subject=payload.subject,
        body_markdown=payload.body_markdown,
        status=NewsletterCampaignStatus.SENDING if is_immediate else NewsletterCampaignStatus.SCHEDULED,
        scheduled_at=payload.scheduled_at,
    )
    session.add(campaign)
    await session.commit()
    await session.refresh(campaign)
    if is_immediate:
        publish_newsletter_campaign(str(campaign.id))
    return CampaignOut.model_validate(campaign)


# --- Endpoint pubblici (nessuna autenticazione) ------------------------------


@router.post("/newsletter/subscribe", response_model=GenericStatusResponse, status_code=status.HTTP_202_ACCEPTED)
async def subscribe(
    payload: SubscribeRequest,
    request: Request,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> GenericStatusResponse:
    ip = client_ip(request)
    if ip is not None:
        await enforce_rate_limit(
            f"ratelimit:newsletter:subscribe:ip:{ip}",
            limit=SUBSCRIBE_IP_RATE_LIMIT,
            window_seconds=SUBSCRIBE_RATE_LIMIT_WINDOW_SECONDS,
            message="Troppe richieste di iscrizione da questo indirizzo. Riprova tra qualche minuto.",
        )
    await enforce_rate_limit(
        f"ratelimit:newsletter:subscribe:email:{payload.email.lower()}",
        limit=SUBSCRIBE_EMAIL_RATE_LIMIT,
        window_seconds=SUBSCRIBE_RATE_LIMIT_WINDOW_SECONDS,
        message="Troppe richieste di iscrizione per questo indirizzo email. Riprova tra qualche minuto.",
    )

    blog: Blog | None = None
    list_label = "Notturni"
    if payload.blog_slug is not None:
        blog = await _get_blog_or_404(session, payload.blog_slug)
        if not await can_view_blog(session, user_id=current_user.id if current_user else None, blog=blog):
            raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")
        list_label = blog.title

    blog_id = blog.id if blog is not None else None
    subscriber = await _find_subscriber(session, blog_id=blog_id, email=payload.email)

    # Nessuna enumerazione: stessa risposta generica in ogni caso, incluso
    # "già iscritto e confermato" — chi tenta di scoprire indirizzi email
    # iscritti non riceve segnali diversi.
    if subscriber is not None and subscriber.status == NewsletterSubscriberStatus.CONFIRMED:
        return GenericStatusResponse()

    plaintext_token, token_hash = generate_confirm_token()
    if subscriber is None:
        subscriber = NewsletterSubscriber(
            blog_id=blog_id,
            email=payload.email,
            locale=payload.locale,
            user_id=current_user.id if current_user else None,
        )
        session.add(subscriber)
    else:
        subscriber.status = NewsletterSubscriberStatus.PENDING
        subscriber.locale = payload.locale or subscriber.locale
        if current_user is not None:
            subscriber.user_id = current_user.id

    subscriber.confirm_token_hash = token_hash
    subscriber.confirm_token_expires_at = confirm_token_expiry()
    subscriber.consent_ip = ip
    subscriber.consent_user_agent = request.headers.get("user-agent")
    await session.commit()

    publish_newsletter_confirmation(payload.email, plaintext_token, list_label, payload.locale)
    return GenericStatusResponse()


@router.get("/newsletter/confirm", response_model=ConfirmResponse)
async def confirm_subscription(token: str, session: AsyncSession = Depends(get_session)) -> ConfirmResponse:
    result = await session.execute(
        select(NewsletterSubscriber).where(NewsletterSubscriber.confirm_token_hash == sha256_hex(token))
    )
    subscriber = result.scalar_one_or_none()
    if subscriber is None:
        return ConfirmResponse(status="invalid")
    if subscriber.status == NewsletterSubscriberStatus.CONFIRMED:
        # Idempotente: non azzeriamo confirm_token_hash alla conferma (sotto)
        # proprio per poter distinguere qui "stesso link ricliccato" da "token
        # mai esistito" — un secondo click da parte del *legittimo* iscritto è
        # il caso normale (doppio click, client email che pre-carica i link),
        # non un tentativo di indovinare un token altrui.
        return ConfirmResponse(status="already_confirmed")
    if (
        subscriber.confirm_token_expires_at is not None
        and subscriber.confirm_token_expires_at < datetime.now(timezone.utc)
    ):
        return ConfirmResponse(status="invalid")

    subscriber.status = NewsletterSubscriberStatus.CONFIRMED
    subscriber.confirmed_at = datetime.now(timezone.utc)
    await session.commit()
    return ConfirmResponse(status="confirmed")


@router.post("/newsletter/unsubscribe", response_model=GenericStatusResponse)
async def unsubscribe(
    payload: UnsubscribeRequest, session: AsyncSession = Depends(get_session)
) -> GenericStatusResponse:
    subscriber_id = verify_unsubscribe_token(payload.token)
    if subscriber_id is None:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Link di disiscrizione non valido.")
    subscriber = await session.get(NewsletterSubscriber, subscriber_id)
    if subscriber is None:
        # link valido ma iscrizione già cancellata: idempotente, stessa
        # risposta generica di successo.
        return GenericStatusResponse()

    subscriber.status = NewsletterSubscriberStatus.UNSUBSCRIBED
    subscriber.unsubscribed_at = datetime.now(timezone.utc)
    subscriber.unsubscribe_reason = payload.reason
    await session.commit()
    return GenericStatusResponse()


@router.post("/newsletter/unsubscribe/delete", response_model=GenericStatusResponse)
async def unsubscribe_and_delete(
    payload: UnsubscribeDeleteRequest, session: AsyncSession = Depends(get_session)
) -> GenericStatusResponse:
    """Cancellazione GDPR self-service (Art. 17), senza bisogno di login: chi
    riceve l'email è già identificato dal token firmato del link."""
    subscriber_id = verify_unsubscribe_token(payload.token)
    if subscriber_id is None:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Link non valido.")
    subscriber = await session.get(NewsletterSubscriber, subscriber_id)
    if subscriber is not None:
        await session.delete(subscriber)
        await session.commit()
    return GenericStatusResponse()


# --- Endpoint per blog (proprietario/autore/co-autore) -----------------------


@router.get("/blogs/{blog_slug}/newsletter/stats", response_model=NewsletterStatsOut)
async def blog_newsletter_stats(
    blog_slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> NewsletterStatsOut:
    blog = await _get_blog_or_404(session, blog_slug)
    await _require_manage_access(session, current_user, blog)
    return await _stats_for(session, blog.id)


@router.get("/blogs/{blog_slug}/newsletter/campaigns", response_model=list[CampaignOut])
async def blog_newsletter_campaigns(
    blog_slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[CampaignOut]:
    blog = await _get_blog_or_404(session, blog_slug)
    await _require_manage_access(session, current_user, blog)
    return await _campaigns_for(session, blog.id)


@router.post(
    "/blogs/{blog_slug}/newsletter/campaigns",
    response_model=CampaignOut,
    status_code=status.HTTP_201_CREATED,
)
async def create_blog_newsletter_campaign(
    blog_slug: str,
    payload: CampaignCreateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> CampaignOut:
    blog = await _get_blog_or_404(session, blog_slug)
    await _require_manage_access(session, current_user, blog)
    return await _create_manual_campaign(session, blog_id=blog.id, created_by=current_user, payload=payload)


@router.patch("/blogs/{blog_slug}/newsletter/settings", response_model=NewsletterSettingsOut)
async def update_blog_newsletter_settings(
    blog_slug: str,
    payload: NewsletterSettingsRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> NewsletterSettingsOut:
    blog = await _get_blog_or_404(session, blog_slug)
    await _require_manage_access(session, current_user, blog)
    blog.newsletter_auto_notify_enabled = payload.newsletter_auto_notify_enabled
    await session.commit()
    return NewsletterSettingsOut(newsletter_auto_notify_enabled=blog.newsletter_auto_notify_enabled)


# --- Endpoint amministrazione di piattaforma (digest, blog_id=None) ---------


@router.get("/admin/newsletter/stats", response_model=NewsletterStatsOut)
async def admin_newsletter_stats(
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> NewsletterStatsOut:
    return await _stats_for(session, None)


@router.get("/admin/newsletter/campaigns", response_model=list[CampaignOut])
async def admin_newsletter_campaigns(
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> list[CampaignOut]:
    return await _campaigns_for(session, None)


@router.post(
    "/admin/newsletter/campaigns", response_model=CampaignOut, status_code=status.HTTP_201_CREATED
)
async def create_admin_newsletter_campaign(
    payload: CampaignCreateRequest,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> CampaignOut:
    return await _create_manual_campaign(session, blog_id=None, created_by=current_user, payload=payload)
