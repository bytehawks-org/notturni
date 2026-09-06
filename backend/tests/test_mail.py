import pytest

from app.core.config import settings
from app.core.mail import MailNotConfigured, send_email


def test_send_email_raises_when_smtp_not_configured(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(settings, "smtp_host", None)
    with pytest.raises(MailNotConfigured):
        send_email(to="mario@example.com", subject="x", body="y")


def test_send_email_sends_via_smtp(monkeypatch: pytest.MonkeyPatch) -> None:
    sent: dict[str, object] = {}

    class _FakeSmtp:
        def __init__(self, host: str, port: int, timeout: int) -> None:
            sent["host"] = host
            sent["port"] = port

        def __enter__(self) -> "_FakeSmtp":
            return self

        def __exit__(self, *exc: object) -> None:
            return None

        def starttls(self) -> None:
            sent["starttls"] = True

        def login(self, user: str, password: str) -> None:
            sent["login"] = (user, password)

        def send_message(self, message: object) -> None:
            sent["message"] = message

    monkeypatch.setattr(settings, "smtp_host", "smtp.example.com")
    monkeypatch.setattr(settings, "smtp_port", 2525)
    monkeypatch.setattr(settings, "smtp_use_tls", True)
    monkeypatch.setattr(settings, "smtp_user", "utente")
    monkeypatch.setattr(settings, "smtp_password", "segreto")
    monkeypatch.setattr("app.core.mail.smtplib.SMTP", _FakeSmtp)

    send_email(to="mario@example.com", subject="Codice", body="123456")

    assert sent["host"] == "smtp.example.com"
    assert sent["port"] == 2525
    assert sent["starttls"] is True
    assert sent["login"] == ("utente", "segreto")
    message = sent["message"]
    assert message["To"] == "mario@example.com"
    assert message["Subject"] == "Codice"
    assert message.get_content().strip() == "123456"
