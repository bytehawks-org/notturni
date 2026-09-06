"""Invio email via SMTP (MFA email OTP, app/workers/email_otp_consumer.py).

Solo stdlib (`smtplib`): niente dipendenza aggiuntiva, il worker che la usa è
già sincrono/bloccante (consumer pika, come app/workers/post_backup_consumer.py)
quindi non serve un client asincrono."""

import smtplib
from email.message import EmailMessage

from app.core.config import settings


class MailNotConfigured(Exception):
    """`NOCT_SMTP_HOST` non valorizzato: nessun invio reale possibile."""


def send_email(*, to: str, subject: str, body: str) -> None:
    if not settings.smtp_host:
        raise MailNotConfigured()

    message = EmailMessage()
    message["Subject"] = subject
    message["From"] = settings.smtp_from_email
    message["To"] = to
    message.set_content(body)

    with smtplib.SMTP(settings.smtp_host, settings.smtp_port, timeout=10) as client:
        if settings.smtp_use_tls:
            client.starttls()
        if settings.smtp_user and settings.smtp_password:
            client.login(settings.smtp_user, settings.smtp_password)
        client.send_message(message)
