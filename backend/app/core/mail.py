"""Invio email via SMTP (MFA email OTP, app/workers/email_otp_consumer.py).

Solo stdlib (`smtplib`): niente dipendenza aggiuntiva, il worker che la usa è
già sincrono/bloccante (consumer pika, come app/workers/post_backup_consumer.py)
quindi non serve un client asincrono."""

import smtplib
from email.headerregistry import Address
from email.message import EmailMessage
from email.utils import parseaddr

from app.core.config import settings


class MailNotConfigured(Exception):
    """`NOCT_SMTP_HOST` non valorizzato: nessun invio reale possibile."""


def send_email(*, to: str, subject: str, body: str, html_body: str | None = None, from_name: str | None = None) -> None:
    """`html_body`: se presente, l'email diventa `multipart/alternative` con
    `body` come fallback testuale (client che non renderizzano HTML, screen
    reader) — usato oggi solo dalle campagne newsletter (banner immagine,
    app/workers/newsletter_consumer.py), gli altri invii (OTP, reset
    password) restano solo testo. `from_name`: nome visualizzato nell'header
    "From", indirizzo sempre quello unico di piattaforma
    (`NOCT_SMTP_FROM_EMAIL`) — mai un dominio arbitrario scelto dall'utente."""
    if not settings.smtp_host:
        raise MailNotConfigured()

    message = EmailMessage()
    message["Subject"] = subject
    message["From"] = settings.smtp_from_email
    if from_name:
        _, from_addr = parseaddr(settings.smtp_from_email)
        try:
            message.replace_header("From", str(Address(display_name=from_name, addr_spec=from_addr)))
        except (ValueError, IndexError):
            pass  # NOCT_SMTP_FROM_EMAIL malformato: invio comunque, senza nome visualizzato
    message["To"] = to
    message.set_content(body)
    if html_body is not None:
        message.add_alternative(html_body, subtype="html")

    with smtplib.SMTP(settings.smtp_host, settings.smtp_port, timeout=10) as client:
        if settings.smtp_use_tls:
            client.starttls()
        if settings.smtp_user and settings.smtp_password:
            client.login(settings.smtp_user, settings.smtp_password)
        client.send_message(message)
