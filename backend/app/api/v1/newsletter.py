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
from app.domain.platform_config import PlatformConfig, get_platform_config, touch
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


def _clean_scheduled_at(value: datetime | None) -> datetime | None:
    # confrontato con datetime.now(timezone.utc) più sotto: un valore
    # naive (senza offset, es. "2026-01-01T10:00:00") solleverebbe
    # TypeError a runtime invece di un 422 leggibile per il client.
    if value is not None and value.tzinfo is None:
        raise ValueError("scheduled_at deve includere il fuso orario (es. suffisso 'Z' o '+00:00').")
    return value


class CampaignCreateRequest(BaseModel):
    subject: str
    body_markdown: str
    scheduled_at: datetime | None = None

    _clean_scheduled_at = field_validator("scheduled_at")(_clean_scheduled_at)


class CampaignUpdateRequest(BaseModel):
    """Solo per campagne `manual` con `status=scheduled` (vedi
    _require_editable_campaign): una campagna già in invio/inviata non è più
    modificabile, una automatica (`post_notification`) non ha un
    subject/body propri da modificare (generati dal post al momento
    dell'invio). Tri-state solo su `scheduled_at`: omesso lascia invariato,
    `null` o nel passato converte la campagna in invio immediato (stessa
    logica di creazione, `_create_manual_campaign::is_immediate`)."""

    subject: str | None = None
    body_markdown: str | None = None
    scheduled_at: datetime | None = None

    _clean_scheduled_at = field_validator("scheduled_at")(_clean_scheduled_at)


MAX_NEWSLETTER_SENDER_NAME_LENGTH = 120
MAX_NEWSLETTER_BANNER_ALT_LENGTH = 300


def _clean_newsletter_sender_name(value: str | None) -> str | None:
    if value is None:
        return None
    text = value.strip()
    if len(text) > MAX_NEWSLETTER_SENDER_NAME_LENGTH:
        raise ValueError(f"Il nome del mittente può avere al massimo {MAX_NEWSLETTER_SENDER_NAME_LENGTH} caratteri.")
    return text or None


def _clean_newsletter_banner_url(value: str | None) -> str | None:
    """Come app/domain/notes.py::_clean_url: questo valore finisce in un
    `<img src>` dell'email HTML della campagna (app/workers/newsletter_consumer.py)
    — niente `javascript:`/schema arbitrario."""
    if value is None:
        return None
    text = value.strip()
    if not text:
        return None
    if not (text.startswith("http://") or text.startswith("https://")):
        raise ValueError("L'URL del banner deve iniziare con http:// o https://.")
    return text


def _clean_newsletter_banner_alt(value: str | None) -> str | None:
    if value is None:
        return None
    text = value.strip()
    if len(text) > MAX_NEWSLETTER_BANNER_ALT_LENGTH:
        raise ValueError(f"L'alt text del banner può avere al massimo {MAX_NEWSLETTER_BANNER_ALT_LENGTH} caratteri.")
    return text


class NewsletterSettingsRequest(BaseModel):
    """Tri-state (CLAUDE.md "insidie note"): un campo omesso lascia il valore
    attuale invariato, `null` lo azzera esplicitamente — usare
    `model_fields_set`, non basta il default `None`."""

    newsletter_auto_notify_enabled: bool | None = None
    newsletter_sender_name: str | None = None
    newsletter_banner_url: str | None = None
    newsletter_banner_alt_text: str | None = None

    _clean_sender_name = field_validator("newsletter_sender_name")(_clean_newsletter_sender_name)
    _clean_banner_url = field_validator("newsletter_banner_url")(_clean_newsletter_banner_url)
    _clean_banner_alt = field_validator("newsletter_banner_alt_text")(_clean_newsletter_banner_alt)


class NewsletterSettingsOut(BaseModel):
    newsletter_auto_notify_enabled: bool
    newsletter_sender_name: str | None
    newsletter_banner_url: str | None
    newsletter_banner_alt_text: str


class AdminNewsletterSettingsRequest(BaseModel):
    """Come NewsletterSettingsRequest, per il digest di piattaforma
    (blog_id=None): nessun newsletter_auto_notify_enabled, non ha senso per
    un digest non legato alla pubblicazione di un singolo blog."""

    newsletter_sender_name: str | None = None
    newsletter_banner_url: str | None = None
    newsletter_banner_alt_text: str | None = None

    _clean_sender_name = field_validator("newsletter_sender_name")(_clean_newsletter_sender_name)
    _clean_banner_url = field_validator("newsletter_banner_url")(_clean_newsletter_banner_url)
    _clean_banner_alt = field_validator("newsletter_banner_alt_text")(_clean_newsletter_banner_alt)


class AdminNewsletterSettingsOut(BaseModel):
    newsletter_sender_name: str | None
    newsletter_banner_url: str | None
    newsletter_banner_alt_text: str


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


async def _get_campaign_or_404(
    session: AsyncSession, *, blog_id: uuid.UUID | None, campaign_id: uuid.UUID
) -> NewsletterCampaign:
    campaign = await session.get(NewsletterCampaign, campaign_id)
    if campaign is None or campaign.blog_id != blog_id:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Campagna non trovata.")
    return campaign


def _require_editable_campaign(campaign: NewsletterCampaign) -> None:
    """Solo una campagna manuale ancora `scheduled` può essere modificata o
    annullata: una volta `sending` il worker può già averla presa in carico
    (nessuna finestra sicura per intercettarla), `sent`/`failed`/`canceled`
    sono stati finali. Le automatiche (`post_notification`) nascono già
    `sending` (vedi app/api/v1/posts.py::_maybe_queue_post_notification),
    quindi non sono mai in questo stato — di fatto mai editabili/annullabili
    da qui, solo dalla dashboard del post che le genera."""
    if campaign.status != NewsletterCampaignStatus.SCHEDULED:
        raise HTTPException(
            status.HTTP_409_CONFLICT,
            "Solo una campagna pianificata (non ancora in invio) può essere modificata o annullata.",
        )


async def _update_campaign(
    session: AsyncSession, campaign: NewsletterCampaign, payload: CampaignUpdateRequest
) -> CampaignOut:
    _require_editable_campaign(campaign)
    fields_set = payload.model_fields_set
    if "subject" in fields_set and payload.subject is not None:
        campaign.subject = payload.subject
    if "body_markdown" in fields_set and payload.body_markdown is not None:
        campaign.body_markdown = payload.body_markdown
    is_immediate = False
    if "scheduled_at" in fields_set:
        campaign.scheduled_at = payload.scheduled_at
        is_immediate = payload.scheduled_at is None or payload.scheduled_at <= datetime.now(timezone.utc)
        if is_immediate:
            campaign.status = NewsletterCampaignStatus.SENDING
    await session.commit()
    await session.refresh(campaign)
    if is_immediate:
        publish_newsletter_campaign(str(campaign.id))
    return CampaignOut.model_validate(campaign)


async def _cancel_campaign(session: AsyncSession, campaign: NewsletterCampaign) -> CampaignOut:
    _require_editable_campaign(campaign)
    campaign.status = NewsletterCampaignStatus.CANCELED
    await session.commit()
    await session.refresh(campaign)
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


@router.patch("/blogs/{blog_slug}/newsletter/campaigns/{campaign_id}", response_model=CampaignOut)
async def update_blog_newsletter_campaign(
    blog_slug: str,
    campaign_id: uuid.UUID,
    payload: CampaignUpdateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> CampaignOut:
    blog = await _get_blog_or_404(session, blog_slug)
    await _require_manage_access(session, current_user, blog)
    campaign = await _get_campaign_or_404(session, blog_id=blog.id, campaign_id=campaign_id)
    return await _update_campaign(session, campaign, payload)


@router.delete("/blogs/{blog_slug}/newsletter/campaigns/{campaign_id}", response_model=CampaignOut)
async def cancel_blog_newsletter_campaign(
    blog_slug: str,
    campaign_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> CampaignOut:
    blog = await _get_blog_or_404(session, blog_slug)
    await _require_manage_access(session, current_user, blog)
    campaign = await _get_campaign_or_404(session, blog_id=blog.id, campaign_id=campaign_id)
    return await _cancel_campaign(session, campaign)


def _newsletter_settings_out(blog: Blog) -> NewsletterSettingsOut:
    return NewsletterSettingsOut(
        newsletter_auto_notify_enabled=blog.newsletter_auto_notify_enabled,
        newsletter_sender_name=blog.newsletter_sender_name,
        newsletter_banner_url=blog.newsletter_banner_url,
        newsletter_banner_alt_text=blog.newsletter_banner_alt_text,
    )


@router.get("/blogs/{blog_slug}/newsletter/settings", response_model=NewsletterSettingsOut)
async def get_blog_newsletter_settings(
    blog_slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> NewsletterSettingsOut:
    blog = await _get_blog_or_404(session, blog_slug)
    await _require_manage_access(session, current_user, blog)
    return _newsletter_settings_out(blog)


@router.patch("/blogs/{blog_slug}/newsletter/settings", response_model=NewsletterSettingsOut)
async def update_blog_newsletter_settings(
    blog_slug: str,
    payload: NewsletterSettingsRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> NewsletterSettingsOut:
    blog = await _get_blog_or_404(session, blog_slug)
    await _require_manage_access(session, current_user, blog)
    fields_set = payload.model_fields_set
    if "newsletter_auto_notify_enabled" in fields_set and payload.newsletter_auto_notify_enabled is not None:
        blog.newsletter_auto_notify_enabled = payload.newsletter_auto_notify_enabled
    if "newsletter_sender_name" in fields_set:
        blog.newsletter_sender_name = payload.newsletter_sender_name
    if "newsletter_banner_url" in fields_set:
        blog.newsletter_banner_url = payload.newsletter_banner_url
    if "newsletter_banner_alt_text" in fields_set:
        blog.newsletter_banner_alt_text = payload.newsletter_banner_alt_text or ""
    await session.commit()
    await session.refresh(blog)
    return _newsletter_settings_out(blog)


# --- Endpoint amministrazione di piattaforma (digest, blog_id=None) ---------


@router.get("/admin/newsletter/stats", response_model=NewsletterStatsOut)
async def admin_newsletter_stats(
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> NewsletterStatsOut:
    return await _stats_for(session, None)


def _admin_newsletter_settings_out(config: PlatformConfig) -> AdminNewsletterSettingsOut:
    return AdminNewsletterSettingsOut(
        newsletter_sender_name=config.newsletter_sender_name,
        newsletter_banner_url=config.newsletter_banner_url,
        newsletter_banner_alt_text=config.newsletter_banner_alt_text,
    )


@router.get("/admin/newsletter/settings", response_model=AdminNewsletterSettingsOut)
async def get_admin_newsletter_settings(
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> AdminNewsletterSettingsOut:
    config = await get_platform_config(session)
    return _admin_newsletter_settings_out(config)


@router.patch("/admin/newsletter/settings", response_model=AdminNewsletterSettingsOut)
async def update_admin_newsletter_settings(
    payload: AdminNewsletterSettingsRequest,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> AdminNewsletterSettingsOut:
    config = await get_platform_config(session)
    fields_set = payload.model_fields_set
    if "newsletter_sender_name" in fields_set:
        config.newsletter_sender_name = payload.newsletter_sender_name
    if "newsletter_banner_url" in fields_set:
        config.newsletter_banner_url = payload.newsletter_banner_url
    if "newsletter_banner_alt_text" in fields_set:
        config.newsletter_banner_alt_text = payload.newsletter_banner_alt_text or ""
    if fields_set:
        touch(config, by_id=current_user.id)
        await session.commit()
        await session.refresh(config)
    return _admin_newsletter_settings_out(config)


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


@router.patch("/admin/newsletter/campaigns/{campaign_id}", response_model=CampaignOut)
async def update_admin_newsletter_campaign(
    campaign_id: uuid.UUID,
    payload: CampaignUpdateRequest,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> CampaignOut:
    campaign = await _get_campaign_or_404(session, blog_id=None, campaign_id=campaign_id)
    return await _update_campaign(session, campaign, payload)


@router.delete("/admin/newsletter/campaigns/{campaign_id}", response_model=CampaignOut)
async def cancel_admin_newsletter_campaign(
    campaign_id: uuid.UUID,
    current_user: User = Depends(require_platform_admin),
    session: AsyncSession = Depends(get_session),
) -> CampaignOut:
    campaign = await _get_campaign_or_404(session, blog_id=None, campaign_id=campaign_id)
    return await _cancel_campaign(session, campaign)
