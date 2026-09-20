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
import html as html_lib
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
from app.domain.markdown_render import render_markdown_to_safe_html
from app.domain.newsletter import CONFIRM_TOKEN_TTL_HOURS, sign_unsubscribe_token
from app.domain.permalinks import build_permalink
from app.domain.platform_config import get_platform_config
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


def _html_footer(*, list_label: str, subscriber: NewsletterSubscriber) -> str:
    confirmed_label = (
        subscriber.confirmed_at.strftime("%d/%m/%Y") if subscriber.confirmed_at else "recentemente"
    )
    return (
        '<hr style="border:none;border-top:1px solid #ddd;margin:24px 0;">'
        f'<p style="font-size:12px;color:#666;">'
        f"Stai ricevendo questa email perché ti sei iscritto/a alla newsletter di "
        f"{html_lib.escape(list_label)} il {confirmed_label}.<br>"
        f'Per non ricevere più queste email, <a href="{html_lib.escape(_unsubscribe_link(subscriber.id))}">'
        "disiscriviti qui</a>.</p>"
    )


def _html_body(
    *,
    body_markdown: str,
    list_label: str,
    subscriber: NewsletterSubscriber,
    banner_url: str | None,
    banner_alt: str,
) -> str:
    banner = (
        f'<img src="{html_lib.escape(banner_url)}" alt="{html_lib.escape(banner_alt)}" '
        'style="max-width:100%;display:block;margin-bottom:20px;">'
        if banner_url
        else ""
    )
    return (
        '<!DOCTYPE html><html><body style="font-family:sans-serif;max-width:600px;'
        'margin:0 auto;color:#111;line-height:1.5;">'
        f"{banner}{render_markdown_to_safe_html(body_markdown)}"
        f"{_html_footer(list_label=list_label, subscriber=subscriber)}"
        "</body></html>"
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
) -> tuple[str, str, str] | None:
    """(oggetto, corpo testuale, sorgente Markdown per il corpo HTML) —
    separati perché il testuale della notifica automatica non è vero
    Markdown (il link "Leggi tutto" resta un URL nudo, leggibile in un
    client senza HTML), mentre la versione HTML lo rende un link cliccabile.
    Nessun footer per-iscritto qui (aggiunto per ogni destinatario da
    `_footer`/`_html_footer`). None se il post di riferimento non esiste più
    (post_notification orfana — nulla da inviare)."""
    if campaign.kind == NewsletterCampaignKind.POST_NOTIFICATION:
        if campaign.post_id is None or blog is None:
            return None
        post = await session.get(Post, campaign.post_id)
        if post is None:
            return None
        excerpt = _excerpt(post.content)
        url = _post_url(blog, post)
        text_body = f"{post.title}\n\n{excerpt}\n\nLeggi tutto: {url}"
        markdown_body = f"### {post.title}\n\n{excerpt}\n\n[Leggi tutto]({url})"
        return campaign.subject, text_body, markdown_body
    body = campaign.body_markdown or ""
    return campaign.subject, body, body


async def _process_campaign(session: AsyncSession, campaign_id: str) -> None:
    campaign = await session.get(NewsletterCampaign, uuid.UUID(campaign_id))
    if campaign is None:
        logger.warning("Campagna newsletter %s non trovata, scarto il messaggio", campaign_id)
        return
    if campaign.status == NewsletterCampaignStatus.CANCELED:
        logger.info("Campagna %s annullata, non invio nulla", campaign_id)
        return
    if campaign.status == NewsletterCampaignStatus.SENT:
        # Ridelivery del messaggio dopo un invio già completato con successo
        # (nack/requeue arrivato dopo il commit finale ma prima dell'ack, o
        # più messaggi in coda per la stessa campagna): non rispedire nulla.
        logger.info("Campagna %s già inviata, ignoro la ridelivery", campaign_id)
        return

    campaign.status = NewsletterCampaignStatus.SENDING
    await session.commit()

    blog = await session.get(Blog, campaign.blog_id) if campaign.blog_id else None
    list_label = blog.title if blog is not None else "Notturni"
    # B: configurazione newsletter (banner, nome mittente) — per blog se la
    # campagna è di un blog, altrimenti quella di piattaforma per il digest.
    if blog is not None:
        sender_name = blog.newsletter_sender_name
        banner_url = blog.newsletter_banner_url
        banner_alt = blog.newsletter_banner_alt_text
    else:
        platform = await get_platform_config(session)
        sender_name = platform.newsletter_sender_name
        banner_url = platform.newsletter_banner_url
        banner_alt = platform.newsletter_banner_alt_text

    content = await _build_content(session, campaign, blog)
    if content is None:
        campaign.status = NewsletterCampaignStatus.FAILED
        await session.commit()
        logger.warning("Campagna %s senza contenuto valido (post mancante?), segnata failed", campaign_id)
        return
    subject, text_body, markdown_body = content

    # sent_to_subscriber_ids copre il caso di ridelivery a metà: il worker
    # interrotto tra un invio e l'altro fa ripartire _process_campaign da
    # capo (status resta SENDING), ma senza rispedire a chi ha già ricevuto
    # l'email nel tentativo precedente.
    already_sent = set(campaign.sent_to_subscriber_ids)
    recipients = [s for s in await _recipients(session, campaign) if s.id not in already_sent]
    sent_ids = list(campaign.sent_to_subscriber_ids)
    failed_count = 0
    for subscriber in recipients:
        try:
            send_email(
                to=subscriber.email,
                subject=subject,
                body=text_body + _footer(list_label=list_label, subscriber=subscriber),
                html_body=_html_body(
                    body_markdown=markdown_body,
                    list_label=list_label,
                    subscriber=subscriber,
                    banner_url=banner_url,
                    banner_alt=banner_alt,
                ),
                from_name=sender_name or list_label,
            )
        except MailNotConfigured:
            # sviluppo locale senza SMTP: non è un fallimento del singolo
            # destinatario, ma dell'intera infrastruttura — stesso comportamento
            # solo-log di email_otp_consumer, non conta come "failed_count".
            logger.warning(
                "NOCT_SMTP_HOST non configurato — invio a %s non effettuato (solo log).", subscriber.email
            )
            continue
        except Exception:
            logger.exception("Invio newsletter fallito per %s (campagna %s)", subscriber.email, campaign_id)
            failed_count += 1
            continue
        sent_ids.append(subscriber.id)
        # Commit per destinatario, non solo a fine ciclo: se il worker viene
        # interrotto qui, la ridelivery successiva riparte dal destinatario
        # giusto invece di rispedire a chi è già in sent_ids.
        campaign.sent_to_subscriber_ids = list(sent_ids)
        await session.commit()

    campaign.recipient_count = len(sent_ids)
    campaign.failed_count = failed_count
    campaign.status = NewsletterCampaignStatus.SENT
    campaign.sent_at = datetime.now(timezone.utc)
    await session.commit()
    logger.info(
        "Campagna %s inviata: %d destinatari, %d falliti", campaign_id, len(sent_ids), failed_count
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
