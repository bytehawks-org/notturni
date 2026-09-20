"""Newsletter/mailing-list (ROADMAP.md, blocco newsletter): iscrizione a un
singolo blog o al digest di piattaforma (`blog_id is None`), e invio di
campagne (notifica automatica di un nuovo post, o manuale)."""

import enum
import uuid
from datetime import datetime

from sqlalchemy import (
    ARRAY,
    DateTime,
    Enum,
    ForeignKey,
    Index,
    Integer,
    String,
    Text,
    func,
)
from sqlalchemy.dialects.postgresql import UUID
from sqlalchemy.orm import Mapped, mapped_column

from app.models.base import Base, TimestampMixin, UUIDPKMixin


class NewsletterSubscriberStatus(str, enum.Enum):
    PENDING = "pending"
    CONFIRMED = "confirmed"
    UNSUBSCRIBED = "unsubscribed"


class NewsletterCampaignKind(str, enum.Enum):
    POST_NOTIFICATION = "post_notification"
    MANUAL = "manual"


class NewsletterCampaignStatus(str, enum.Enum):
    DRAFT = "draft"
    SCHEDULED = "scheduled"
    SENDING = "sending"
    SENT = "sent"
    CANCELED = "canceled"
    FAILED = "failed"


class NewsletterSubscriber(Base, UUIDPKMixin, TimestampMixin):
    __tablename__ = "newsletter_subscribers"

    # None: iscrizione al digest di piattaforma (notturni.eu), non a un
    # singolo blog — vedi anche i due indici unici parziali sotto.
    blog_id: Mapped[uuid.UUID | None] = mapped_column(
        ForeignKey("blogs.id", ondelete="CASCADE"), nullable=True
    )
    email: Mapped[str] = mapped_column(String(255), nullable=False)
    locale: Mapped[str | None] = mapped_column(String(2), nullable=True)
    status: Mapped[NewsletterSubscriberStatus] = mapped_column(
        Enum(
            NewsletterSubscriberStatus,
            name="newsletter_subscriber_status",
            native_enum=True,
            values_callable=lambda enum_cls: [e.value for e in enum_cls],
        ),
        default=NewsletterSubscriberStatus.PENDING,
        nullable=False,
    )
    # Utente registrato collegato all'email, se esiste — solo per l'export/
    # cancellazione GDPR self-service (app/domain/gdpr.py); l'iscrizione
    # resta valida anche senza un account (SET NULL, non CASCADE).
    user_id: Mapped[uuid.UUID | None] = mapped_column(
        ForeignKey("users.id", ondelete="SET NULL"), nullable=True
    )
    confirm_token_hash: Mapped[str | None] = mapped_column(String(64), nullable=True, index=True)
    confirm_token_expires_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)
    # NB: niente colonna `unsubscribe_token_hash`/token opaco a hash come gli
    # altri (api_tokens, MFA): a differenza del token di conferma (verificato
    # una sola volta, un lookup per hash va benissimo), il link di
    # disiscrizione deve restare valido e riproducibile in ogni email inviata
    # per sempre, e il worker deve poterlo *ricostruire* ad ogni invio senza
    # aver mai persistito il valore in chiaro da nessuna parte. È quindi
    # firmato in modo stateless via HMAC-SHA256 a partire dal solo subscriber
    # id (app/domain/newsletter.py: sign_unsubscribe_token/
    # verify_unsubscribe_token) — niente colonna, niente giro a DB per
    # verificarlo, coerente con il resto della riga (nessuno stato aggiuntivo
    # da mantenere in sync).
    consent_ip: Mapped[str | None] = mapped_column(String(64), nullable=True)
    consent_user_agent: Mapped[str | None] = mapped_column(String(512), nullable=True)
    confirmed_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)
    unsubscribed_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)
    unsubscribe_reason: Mapped[str | None] = mapped_column(String(500), nullable=True)

    __table_args__ = (
        # Una sola riga per email per lista: per un blog specifico...
        Index(
            "uq_newsletter_subscriber_blog_email",
            "blog_id",
            func.lower(email),
            unique=True,
            postgresql_where=(blog_id.is_not(None)),
        ),
        # ...o per il digest di piattaforma (blog_id null).
        Index(
            "uq_newsletter_subscriber_platform_email",
            func.lower(email),
            unique=True,
            postgresql_where=(blog_id.is_(None)),
        ),
    )


class NewsletterCampaign(Base, UUIDPKMixin, TimestampMixin):
    __tablename__ = "newsletter_campaigns"

    # None: campagna del digest di piattaforma.
    blog_id: Mapped[uuid.UUID | None] = mapped_column(
        ForeignKey("blogs.id", ondelete="CASCADE"), nullable=True
    )
    kind: Mapped[NewsletterCampaignKind] = mapped_column(
        Enum(
            NewsletterCampaignKind,
            name="newsletter_campaign_kind",
            native_enum=True,
            values_callable=lambda enum_cls: [e.value for e in enum_cls],
        ),
        nullable=False,
    )
    # Solo per kind=post_notification. L'indice unico parziale sotto impedisce
    # due campagne automatiche per lo stesso post (es. ripubblicazione dopo
    # un ritorno in bozza) — vedi il controllo pre-insert in app/api/v1/posts.py.
    post_id: Mapped[uuid.UUID | None] = mapped_column(
        ForeignKey("posts.id", ondelete="CASCADE"), nullable=True
    )
    # None per un invio automatico (post_notification): non c'è un autore umano.
    created_by_id: Mapped[uuid.UUID | None] = mapped_column(
        ForeignKey("users.id", ondelete="SET NULL"), nullable=True
    )
    subject: Mapped[str] = mapped_column(String(255), nullable=False)
    # None per post_notification: generato dal worker a partire dal post al
    # momento dell'invio, non salvato in anticipo qui.
    body_markdown: Mapped[str | None] = mapped_column(Text, nullable=True)
    status: Mapped[NewsletterCampaignStatus] = mapped_column(
        Enum(
            NewsletterCampaignStatus,
            name="newsletter_campaign_status",
            native_enum=True,
            values_callable=lambda enum_cls: [e.value for e in enum_cls],
        ),
        default=NewsletterCampaignStatus.DRAFT,
        nullable=False,
    )
    scheduled_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)
    sent_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)
    recipient_count: Mapped[int] = mapped_column(Integer, default=0, nullable=False)
    failed_count: Mapped[int] = mapped_column(Integer, default=0, nullable=False)
    # Id degli iscritti già notificati per questa campagna: se il worker viene
    # interrotto a metà invio (crash, riavvio) il broker riconsegna lo stesso
    # messaggio da capo (app/workers/newsletter_consumer.py::_on_message,
    # nack/requeue) — senza questo elenco, _process_campaign rispedirebbe
    # l'email anche a chi l'ha già ricevuta.
    sent_to_subscriber_ids: Mapped[list[uuid.UUID]] = mapped_column(
        ARRAY(UUID(as_uuid=True)), default=list, nullable=False
    )

    __table_args__ = (
        Index(
            "uq_newsletter_campaign_post",
            "post_id",
            unique=True,
            postgresql_where=(post_id.is_not(None)),
        ),
    )
