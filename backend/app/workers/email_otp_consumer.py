"""Consumer della coda email_otp: invia il codice via SMTP (app/core/mail.py).

Senza NOCT_SMTP_HOST configurato l'invio resta solo loggato (comodo in
sviluppo locale senza SMTP a disposizione) — non deve mai essere il caso in
produzione. Un errore SMTP genuino (host irraggiungibile, credenziali
sbagliate) fa nack/requeue del messaggio, come il consumer del backup post.

Uso (dalla directory backend/, con il venv attivo):
    python -m app.workers.email_otp_consumer
"""

import json
import logging

from app.core.broker import EMAIL_OTP_QUEUE, connect_with_retry
from app.core.mail import MailNotConfigured, send_email
from app.domain.mfa import EMAIL_OTP_TTL_MINUTES

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger("email_otp_consumer")


_SUBJECTS = {
    "login": "Il tuo codice di accesso Notturni",
    "password_reset": "Reimposta la tua password Notturni",
}


def _body_for(purpose: str, code: str) -> str:
    if purpose == "password_reset":
        return (
            f"Il tuo codice per reimpostare la password su Notturni è: {code}\n\n"
            f"Scade tra {EMAIL_OTP_TTL_MINUTES} minuti. Se non hai richiesto tu "
            "questo reset, ignora pure questa email: la tua password attuale resta invariata."
        )
    return (
        f"Il tuo codice di accesso a Notturni è: {code}\n\n"
        f"Scade tra {EMAIL_OTP_TTL_MINUTES} minuti. Se non hai richiesto tu "
        "questo accesso, ignora pure questa email."
    )


def _on_message(channel, method, _properties, body) -> None:
    payload = json.loads(body)
    email, code = payload["email"], payload["code"]
    purpose = payload.get("purpose", "login")
    try:
        send_email(
            to=email,
            subject=_SUBJECTS.get(purpose, _SUBJECTS["login"]),
            body=_body_for(purpose, code),
        )
        logger.info("OTP inviato via email a %s", email)
        channel.basic_ack(delivery_tag=method.delivery_tag)
    except MailNotConfigured:
        logger.warning(
            "NOCT_SMTP_HOST non configurato — OTP %s per %s non inviato (solo log, sviluppo locale).",
            code,
            email,
        )
        channel.basic_ack(delivery_tag=method.delivery_tag)
    except Exception:
        logger.exception("Invio email OTP fallito per %s, richiedo nack/requeue", email)
        channel.basic_nack(delivery_tag=method.delivery_tag, requeue=True)


def main() -> None:
    connection = connect_with_retry()
    channel = connection.channel()
    channel.queue_declare(queue=EMAIL_OTP_QUEUE, durable=True)
    channel.basic_qos(prefetch_count=1)
    channel.basic_consume(queue=EMAIL_OTP_QUEUE, on_message_callback=_on_message)
    logger.info("In ascolto sulla coda %s...", EMAIL_OTP_QUEUE)
    channel.start_consuming()


if __name__ == "__main__":
    main()
