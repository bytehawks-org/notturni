import json
import logging
import time

import pika

from app.core.config import settings

EMAIL_OTP_QUEUE = "email_otp"
POST_BACKUP_QUEUE = "post_backup"

logger = logging.getLogger(__name__)


def connect_with_retry(
    *, max_attempts: int = 10, initial_delay_seconds: float = 2.0
) -> pika.BlockingConnection:
    """Per i worker long-running (consumer): a differenza di publish_*, qui
    non basta fallire subito — RabbitMQ potrebbe non essere ancora pronto ad
    accettare connessioni quando il container del worker parte (il
    "service_started" di compose/k8s non garantisce che il servizio dentro
    sia già in ascolto), quindi si ritenta con backoff esponenziale invece di
    andare in crash al primo tentativo."""
    delay = initial_delay_seconds
    for attempt in range(1, max_attempts + 1):
        try:
            return pika.BlockingConnection(pika.URLParameters(settings.rabbitmq_url))
        except pika.exceptions.AMQPConnectionError:
            if attempt == max_attempts:
                raise
            logger.warning(
                "Connessione a RabbitMQ fallita (tentativo %d/%d), riprovo tra %.0fs",
                attempt,
                max_attempts,
                delay,
            )
            time.sleep(delay)
            delay = min(delay * 2, 30)
    raise AssertionError("irraggiungibile")


def publish_email_otp(email: str, code: str) -> None:
    """Accoda l'invio del codice OTP via email (CLAUDE.md #3).

    Nessun consumer di invio email reale è ancora collegato: la coda esiste e
    il messaggio viene pubblicato correttamente, ma manca l'integrazione con
    un provider SMTP/transazionale — vedi app/workers/email_otp_consumer.py.
    """
    connection = pika.BlockingConnection(pika.URLParameters(settings.rabbitmq_url))
    try:
        channel = connection.channel()
        channel.queue_declare(queue=EMAIL_OTP_QUEUE, durable=True)
        channel.basic_publish(
            exchange="",
            routing_key=EMAIL_OTP_QUEUE,
            body=json.dumps({"email": email, "code": code}),
            properties=pika.BasicProperties(delivery_mode=2),
        )
    finally:
        connection.close()


def publish_post_backup(
    *, user_id: str, blog_id: str, post_id: str, title: str, content: str, locale: str
) -> None:
    """Accoda una copia di backup/fallback del post su S3 (vedi
    app/workers/post_backup_consumer.py). Il database resta la fonte di
    verità: il chiamante non deve far fallire il salvataggio del post se
    questo accodamento fallisce — vedi app/api/v1/posts.py."""
    connection = pika.BlockingConnection(pika.URLParameters(settings.rabbitmq_url))
    try:
        channel = connection.channel()
        channel.queue_declare(queue=POST_BACKUP_QUEUE, durable=True)
        channel.basic_publish(
            exchange="",
            routing_key=POST_BACKUP_QUEUE,
            body=json.dumps(
                {
                    "user_id": user_id,
                    "blog_id": blog_id,
                    "post_id": post_id,
                    "title": title,
                    "content": content,
                    "locale": locale,
                }
            ),
            properties=pika.BasicProperties(delivery_mode=2),
        )
    finally:
        connection.close()
