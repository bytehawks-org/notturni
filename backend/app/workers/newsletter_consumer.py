"""Consumer della coda newsletter_send: invia l'email di conferma iscrizione
(double opt-in) oppure una campagna (notifica automatica di un nuovo post, o
invio manuale) agli iscritti confermati di una lista (blog o digest di
piattaforma, `blog_id is None`).

A differenza degli altri consumer (email_otp, post_backup), l'invio di una
campagna richiede il database (iscritti, post/blog, stato della campagna):
apre una propria sessione async per messaggio con `SessionLocal`, la stessa
session factory usata da FastAPI ma senza `Depends` (specifico del ciclo di
vita di una request HTTP, qui non c'è).

Uso (dalla directory backend/, con il venv attivo):
    python -m app.workers.newsletter_consumer
"""

import asyncio
import json
import logging
import uuid
from datetime import datetime, timezone

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.core.broker import NEWSLETTER_SEND_QUEUE, connect_with_retry
from app.core.config import settings
from app.core.database import SessionLocal
from app.core.mail import MailNotConfigured, send_email
from app.domain.newsletter import CONFIRM_TOKEN_TTL_HOURS, sign_unsubscribe_token
from app.domain.permalinks import build_permalink
from app.models.blog import Blog
from app.models.newsletter import (
    NewsletterCampaign,
    NewsletterCampaignKind,
    NewsletterCampaignStatus,
    NewsletterSubscriber,
    NewsletterSubscriberStatus,
)
from app.models.post import Post

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger("newsletter_consumer")

# Estratto del contenuto del post nell'email di notifica: non c'è un campo
# "excerpt" dedicato su Post (todo/PUBLICATIONS.md non lo prevede), quindi si
# tronca il markdown grezzo — l'email resta testo semplice, nessun rendering.
POST_EXCERPT_LENGTH = 400


def _confirm_link(token: str) -> str:
    return f"https://{settings.instance_fqdn}/newsletter/confirm?token={token}"


def _unsubscribe_link(subscriber_id: uuid.UUID) -> str:
    return (
        f"https://{settings.instance_fqdn}/newsletter/unsubscribe"
        f"?token={sign_unsubscribe_token(subscriber_id)}"
    )


def _post_url(blog: Blog, post: Post) -> str:
    return f"https://{settings.instance_fqdn}{build_permalink(blog.slug, post)}"


def _excerpt(content: str) -> str:
    text = " ".join(content.split())
    if len(text) <= POST_EXCERPT_LENGTH:
        return text
    return text[:POST_EXCERPT_LENGTH].rsplit(" ", 1)[0] + "…"


def _footer(*, list_label: str, subscriber: NewsletterSubscriber) -> str:
    confirmed_label = (
        subscriber.confirmed_at.strftime("%d/%m/%Y") if subscriber.confirmed_at else "recentemente"
    )
    return (
        "\n\n---\n"
        f"Stai ricevendo questa email perché ti sei iscritto/a alla newsletter di {list_label} "
        f"il {confirmed_label}.\n"
        f"Per non ricevere più queste email, disiscriviti qui: {_unsubscribe_link(subscriber.id)}"
    )


def _send_confirmation(payload: dict) -> None:
    link = _confirm_link(payload["token"])
    send_email(
        to=payload["email"],
        subject=f"Conferma la tua iscrizione alla newsletter di {payload['list_label']}",
        body=(
            f"Conferma la tua iscrizione alla newsletter di {payload['list_label']} cliccando "
            f"questo link:\n\n{link}\n\n"
            f"Il link scade tra {CONFIRM_TOKEN_TTL_HOURS} ore. Se non hai richiesto tu questa "
            "iscrizione, ignora pure questa email."
        ),
    )


async def _recipients(session: AsyncSession, campaign: NewsletterCampaign) -> list[NewsletterSubscriber]:
    stmt = select(NewsletterSubscriber).where(
        NewsletterSubscriber.status == NewsletterSubscriberStatus.CONFIRMED
    )
    if campaign.blog_id is None:
        stmt = stmt.where(NewsletterSubscriber.blog_id.is_(None))
    else:
        stmt = stmt.where(NewsletterSubscriber.blog_id == campaign.blog_id)
    return list((await session.execute(stmt)).scalars().all())


async def _build_content(
    session: AsyncSession, campaign: NewsletterCampaign, blog: Blog | None
) -> tuple[str, str] | None:
    """(oggetto, corpo) dell'email, senza il footer per-iscritto (aggiunto
    per ogni destinatario da `_footer`). None se il post di riferimento non
    esiste più (post_notification orfana — nulla da inviare)."""
    if campaign.kind == NewsletterCampaignKind.POST_NOTIFICATION:
        if campaign.post_id is None or blog is None:
            return None
        post = await session.get(Post, campaign.post_id)
        if post is None:
            return None
        body = f"{post.title}\n\n{_excerpt(post.content)}\n\nLeggi tutto: {_post_url(blog, post)}"
        return campaign.subject, body
    return campaign.subject, campaign.body_markdown or ""


async def _process_campaign(session: AsyncSession, campaign_id: str) -> None:
    campaign = await session.get(NewsletterCampaign, uuid.UUID(campaign_id))
    if campaign is None:
        logger.warning("Campagna newsletter %s non trovata, scarto il messaggio", campaign_id)
        return
    if campaign.status == NewsletterCampaignStatus.CANCELED:
        logger.info("Campagna %s annullata, non invio nulla", campaign_id)
        return

    campaign.status = NewsletterCampaignStatus.SENDING
    await session.commit()

    blog = await session.get(Blog, campaign.blog_id) if campaign.blog_id else None
    list_label = blog.title if blog is not None else "Notturni"

    content = await _build_content(session, campaign, blog)
    if content is None:
        campaign.status = NewsletterCampaignStatus.FAILED
        await session.commit()
        logger.warning("Campagna %s senza contenuto valido (post mancante?), segnata failed", campaign_id)
        return
    subject, body = content

    recipients = await _recipients(session, campaign)
    sent_count = 0
    failed_count = 0
    for subscriber in recipients:
        try:
            send_email(
                to=subscriber.email,
                subject=subject,
                body=body + _footer(list_label=list_label, subscriber=subscriber),
            )
            sent_count += 1
        except MailNotConfigured:
            # sviluppo locale senza SMTP: non è un fallimento del singolo
            # destinatario, ma dell'intera infrastruttura — stesso comportamento
            # solo-log di email_otp_consumer, non conta come "failed_count".
            logger.warning(
                "NOCT_SMTP_HOST non configurato — invio a %s non effettuato (solo log).", subscriber.email
            )
        except Exception:
            logger.exception("Invio newsletter fallito per %s (campagna %s)", subscriber.email, campaign_id)
            failed_count += 1

    campaign.recipient_count = sent_count
    campaign.failed_count = failed_count
    campaign.status = NewsletterCampaignStatus.SENT
    campaign.sent_at = datetime.now(timezone.utc)
    await session.commit()
    logger.info(
        "Campagna %s inviata: %d destinatari, %d falliti", campaign_id, sent_count, failed_count
    )


async def _handle_payload(payload: dict) -> None:
    if payload.get("kind") == "confirmation":
        _send_confirmation(payload)
        return
    if payload.get("kind") == "campaign":
        async with SessionLocal() as session:
            try:
                await _process_campaign(session, payload["campaign_id"])
            except Exception:
                # fallimento non per-destinatario (DB irraggiungibile, bug):
                # prova comunque a segnare la campagna come fallita, così non
                # resta bloccata in "sending" all'infinito. Serve il rollback
                # esplicito prima: una `AsyncSession` con una query fallita
                # resta in transazione abortita (asyncpg) finché non viene
                # chiuso il blocco — qualunque query successiva sulla stessa
                # sessione fallirebbe a sua volta con "current transaction is
                # aborted", mascherando l'errore vero e facendo girare a vuoto
                # il nack/requeue ad ogni tentativo (bug scoperto dal vivo).
                logger.exception("Elaborazione campagna %s fallita", payload.get("campaign_id"))
                await session.rollback()
                try:
                    campaign = await session.get(NewsletterCampaign, uuid.UUID(payload["campaign_id"]))
                    if campaign is not None:
                        campaign.status = NewsletterCampaignStatus.FAILED
                        await session.commit()
                except Exception:
                    logger.exception("Impossibile segnare la campagna come failed, richiedo nack/requeue")
                    raise
        return
    logger.warning("Messaggio newsletter_send con kind sconosciuto: %r", payload.get("kind"))


def _on_message(channel, method, _properties, body, *, loop: asyncio.AbstractEventLoop) -> None:
    payload = json.loads(body)
    try:
        # Un solo event loop per l'intera vita del processo (`loop`, creato in
        # `main()`), non un `asyncio.run()` nuovo ad ogni messaggio: il pool di
        # connessioni asyncpg di `SessionLocal` (app/core/database.py, un
        # `AsyncEngine` a livello di modulo, condiviso) lega ogni connessione
        # al loop che l'ha aperta — un `asyncio.run()` per messaggio chiude
        # quel loop a fine chiamata e la connessione pooled resta legata a un
        # loop già distrutto, corrotta per il messaggio successivo
        # ("InterfaceError: another operation is in progress",
        # "InFailedSQLTransactionError" a catena — bug scoperto dal vivo).
        loop.run_until_complete(_handle_payload(payload))
        channel.basic_ack(delivery_tag=method.delivery_tag)
    except MailNotConfigured:
        logger.warning("NOCT_SMTP_HOST non configurato — messaggio non inviato (solo log, sviluppo locale).")
        channel.basic_ack(delivery_tag=method.delivery_tag)
    except Exception:
        logger.exception("Elaborazione messaggio newsletter fallita, richiedo nack/requeue")
        channel.basic_nack(delivery_tag=method.delivery_tag, requeue=True)


def main() -> None:
    loop = asyncio.new_event_loop()
    connection = connect_with_retry()
    channel = connection.channel()
    channel.queue_declare(queue=NEWSLETTER_SEND_QUEUE, durable=True)
    channel.basic_qos(prefetch_count=1)
    channel.basic_consume(
        queue=NEWSLETTER_SEND_QUEUE,
        on_message_callback=lambda ch, method, properties, body: _on_message(
            ch, method, properties, body, loop=loop
        ),
    )
    logger.info("In ascolto sulla coda %s...", NEWSLETTER_SEND_QUEUE)
    channel.start_consuming()


if __name__ == "__main__":
    main()
