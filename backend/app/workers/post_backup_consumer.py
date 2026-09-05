"""Consumer della coda post_backup: scrive sul backend di storage attivo
(S3/MinIO o filesystem locale) la copia di backup/fallback del Markdown di
ogni post creato/modificato (app/api/v1/posts.py). A differenza del consumer
OTP email, qui la scrittura è reale e funzionante (riusa l'integrazione già
in app/core/storage.py) — non è un placeholder.

Uso (dalla directory backend/, con il venv attivo):
    python -m app.workers.post_backup_consumer
"""

import json
import logging

from app.core.broker import POST_BACKUP_QUEUE, connect_with_retry
from app.core.storage import upload_post_backup

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger("post_backup_consumer")


def _on_message(channel, method, _properties, body) -> None:
    payload = json.loads(body)
    try:
        upload_post_backup(
            user_id=payload["user_id"],
            blog_id=payload["blog_id"],
            post_id=payload["post_id"],
            content=payload["content"],
        )
        logger.info("Backup S3 scritto per il post %s", payload["post_id"])
        channel.basic_ack(delivery_tag=method.delivery_tag)
    except Exception:
        logger.exception("Backup S3 fallito per il post %s, richiedo nack/requeue", payload["post_id"])
        channel.basic_nack(delivery_tag=method.delivery_tag, requeue=True)


def main() -> None:
    connection = connect_with_retry()
    channel = connection.channel()
    channel.queue_declare(queue=POST_BACKUP_QUEUE, durable=True)
    channel.basic_qos(prefetch_count=1)
    channel.basic_consume(queue=POST_BACKUP_QUEUE, on_message_callback=_on_message)
    logger.info("In ascolto sulla coda %s...", POST_BACKUP_QUEUE)
    channel.start_consuming()


if __name__ == "__main__":
    main()
