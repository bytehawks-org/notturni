from collections.abc import Callable

import pytest
from httpx import AsyncClient
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.email_change_request import EmailChangeRequest
from tests.conftest import AuthedUser


@pytest.fixture(autouse=True)
def _captured_emails(monkeypatch: pytest.MonkeyPatch) -> list[tuple[str, str]]:
    sent: list[tuple[str, str]] = []

    def _fake_publish(email: str, code: str) -> None:
        sent.append((email, code))

    monkeypatch.setattr("app.domain.email_change.publish_email_otp", _fake_publish)
    return sent


async def test_cancel_email_change_deletes_pending_request(
    client: AsyncClient, make_user: Callable, db_session: AsyncSession
) -> None:
    """Il pulsante "Annulla" lato dashboard puliva solo lo stato React
    locale: la richiesta restava in DB (con l'OTP ancora valido) e
    ricompariva a un refresh. Verifica che l'endpoint di cancellazione la
    rimuova davvero."""
    user: AuthedUser = await make_user("cambio-email")
    req = await client.post(
        "/api/v1/users/me/email/request", json={"new_email": "nuova@example.com"}, headers=user.headers
    )
    assert req.status_code == 202, req.text

    result = await db_session.execute(
        select(EmailChangeRequest).where(EmailChangeRequest.new_email == "nuova@example.com")
    )
    assert result.scalar_one_or_none() is not None

    cancel = await client.delete("/api/v1/users/me/email/request", headers=user.headers)
    assert cancel.status_code == 204

    result = await db_session.execute(
        select(EmailChangeRequest).where(EmailChangeRequest.new_email == "nuova@example.com")
    )
    assert result.scalar_one_or_none() is None


async def test_cancel_email_change_without_pending_request_is_a_noop(
    client: AsyncClient, make_user: Callable
) -> None:
    user: AuthedUser = await make_user("nessun-cambio")
    res = await client.delete("/api/v1/users/me/email/request", headers=user.headers)
    assert res.status_code == 204
