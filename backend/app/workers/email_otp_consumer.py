"""Consumer della coda email_otp.

PLACEHOLDER: logga soltanto il codice invece di inviarlo per email. Manca
ancora l'integrazione con un provider SMTP/transazionale (non specificato in
CLAUDE.md) — da collegare prima di usare l'MFA via email in un ambiente reale.

Uso (dalla directory backend/, con il venv attivo):
    python -m app.workers.email_otp_consumer
"""

import json
import logging

from app.core.broker import EMAIL_OTP_QUEUE, connect_with_retry

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger("email_otp_consumer")


def _on_message(channel, method, _properties, body) -> None:
    payload = json.loads(body)
    logger.info(
        "TODO invio email reale non configurato — OTP %s per %s",
        payload["code"],
        payload["email"],
    )
    channel.basic_ack(delivery_tag=method.delivery_tag)


def main() -> None:
    connection = connect_with_retry()
    channel = connection.channel()
    channel.queue_declare(queue=EMAIL_OTP_QUEUE, durable=True)
    channel.basic_consume(queue=EMAIL_OTP_QUEUE, on_message_callback=_on_message)
    logger.info("In ascolto sulla coda %s...", EMAIL_OTP_QUEUE)
    channel.start_consuming()


if __name__ == "__main__":
    main()
