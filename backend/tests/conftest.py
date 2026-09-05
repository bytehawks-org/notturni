import io
import itertools
import os
from collections.abc import AsyncIterator, Callable
from dataclasses import dataclass
from pathlib import Path

# Va fatto PRIMA di qualunque import da `app`: Settings() viene istanziato al
# primo import di app.core.config, quindi le variabili d'ambiente devono
# essere già presenti a quel punto.
_ENV_TEST_PATH = Path(__file__).resolve().parent.parent / ".env.test"


def _load_env_test() -> None:
    for line in _ENV_TEST_PATH.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, _, value = line.partition("=")
        os.environ.setdefault(key.strip(), value.strip())


_load_env_test()

import pytest
import pytest_asyncio
from httpx import ASGITransport, AsyncClient
from sqlalchemy import text
from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker, create_async_engine

from app.core.config import settings
from app.core.database import get_session
from app.domain.api_tokens import generate_api_token
from app.main import app
from app.models import ApiToken, ApiTokenOwnerType, Base

# Un engine per test (creato e smaltito nello stesso evento asyncio del test
# stesso) invece di uno globale: pytest-asyncio 1.x usa un event loop diverso
# per ogni test di default, e un engine/pool asyncpg condiviso tra loop
# diversi rompe con "attached to a different loop".


@pytest_asyncio.fixture(scope="session", autouse=True)
async def _prepare_schema() -> None:
    """Ricrea lo schema da zero una volta per sessione di test, allineato ai
    modelli correnti (non passa da Alembic: più veloce, e i test non devono
    dipendere dalla cronologia delle migrazioni). L'engine usato qui viene
    smaltito subito dopo, senza restare vivo tra un test e l'altro."""
    engine = create_async_engine(settings.database_url)
    async with engine.begin() as conn:
        await conn.run_sync(Base.metadata.drop_all)
        await conn.run_sync(Base.metadata.create_all)
    await engine.dispose()


@pytest_asyncio.fixture
async def db_session() -> AsyncIterator[AsyncSession]:
    engine = create_async_engine(settings.database_url)
    session_factory = async_sessionmaker(engine, expire_on_commit=False)

    async with engine.begin() as conn:
        table_names = ", ".join(f'"{t.name}"' for t in Base.metadata.sorted_tables)
        await conn.execute(text(f"TRUNCATE {table_names} RESTART IDENTITY CASCADE"))

    async with session_factory() as session:
        yield session

    await engine.dispose()


@pytest_asyncio.fixture
async def client(db_session: AsyncSession) -> AsyncIterator[AsyncClient]:
    """Client HTTP in-process sulla stessa sessione/engine di db_session, così
    un test può creare dati via API e ispezionarli via db_session (o
    viceversa) senza problemi di visibilità tra connessioni diverse."""

    async def _override_get_session() -> AsyncIterator[AsyncSession]:
        yield db_session

    app.dependency_overrides[get_session] = _override_get_session
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as ac:
        yield ac
    app.dependency_overrides.pop(get_session, None)


@dataclass
class AuthedUser:
    username: str
    email: str
    password: str
    access_token: str
    refresh_token: str

    @property
    def headers(self) -> dict[str, str]:
        return {"Authorization": f"Bearer {self.access_token}"}


@pytest_asyncio.fixture
async def make_user(client: AsyncClient) -> Callable:
    """Factory: registra e logga un nuovo utente, ritorna un AuthedUser."""
    counter = itertools.count()

    async def _make(username: str | None = None, password: str = "Password123!") -> AuthedUser:
        n = next(counter)
        username = username or f"utente{n}"
        email = f"{username}@example.com"

        res = await client.post(
            "/api/v1/auth/register",
            json={"username": username, "email": email, "password": password},
        )
        assert res.status_code == 201, res.text

        login = await client.post("/api/v1/auth/login", json={"email": email, "password": password})
        assert login.status_code == 200, login.text
        data = login.json()

        return AuthedUser(
            username=username,
            email=email,
            password=password,
            access_token=data["access_token"],
            refresh_token=data["refresh_token"],
        )

    return _make


@pytest_asyncio.fixture
async def make_admin(make_user: Callable, db_session: AsyncSession) -> Callable:
    """Come make_user, ma promuove l'utente ad Amministratore di piattaforma
    (non esiste un endpoint per farlo: è un'azione di provisioning)."""
    from sqlalchemy import select

    from app.models.user import PlatformRole, User

    async def _make(username: str | None = None) -> AuthedUser:
        authed = await make_user(username)
        result = await db_session.execute(select(User).where(User.username == authed.username))
        user = result.scalar_one()
        user.platform_role = PlatformRole.AMMINISTRATORE
        await db_session.commit()
        return authed

    return _make


@pytest_asyncio.fixture
async def core_api_token(db_session: AsyncSession) -> str:
    """Un token core valido, senza passare dallo script di bootstrap (che
    scrive su un processo/DB separato) — inserimento diretto, stessa logica."""
    plaintext, prefix, token_hash = generate_api_token()
    db_session.add(
        ApiToken(
            name="test-core-token",
            owner_type=ApiTokenOwnerType.CORE,
            token_prefix=prefix,
            token_hash=token_hash,
        )
    )
    await db_session.commit()
    return plaintext


class FakeS3Client:
    """Sostituisce boto3 in-memory: stessa interfaccia usata da app.core.storage."""

    def __init__(self) -> None:
        self.objects: dict[tuple[str, str], bytes] = {}
        self.buckets: set[str] = set()

    def head_bucket(self, Bucket: str) -> None:  # noqa: N803 (nomi boto3)
        if Bucket not in self.buckets:
            raise ValueError("bucket inesistente")

    def create_bucket(self, Bucket: str) -> None:  # noqa: N803
        self.buckets.add(Bucket)

    def put_bucket_policy(self, Bucket: str, Policy: str) -> None:  # noqa: N803
        pass

    def put_object(self, Bucket: str, Key: str, Body: bytes, ContentType: str) -> None:  # noqa: N803
        self.objects[(Bucket, Key)] = Body

    def get_object(self, Bucket: str, Key: str) -> dict:  # noqa: N803
        return {"Body": io.BytesIO(self.objects[(Bucket, Key)])}

    def delete_object(self, Bucket: str, Key: str) -> None:  # noqa: N803
        self.objects.pop((Bucket, Key), None)


@pytest.fixture
def fake_s3(monkeypatch: pytest.MonkeyPatch) -> FakeS3Client:
    fake_client = FakeS3Client()
    monkeypatch.setattr("app.core.storage.get_s3_client", lambda: fake_client)
    return fake_client


@pytest.fixture
def captured_emails(monkeypatch: pytest.MonkeyPatch) -> list[tuple[str, str]]:
    """Cattura i codici OTP che verrebbero pubblicati su RabbitMQ, senza
    richiedere RabbitMQ in esecuzione durante i test."""
    sent: list[tuple[str, str]] = []

    def _fake_publish(email: str, code: str) -> None:
        sent.append((email, code))

    monkeypatch.setattr("app.domain.mfa.publish_email_otp", _fake_publish)
    return sent


@pytest.fixture
def captured_post_backups(monkeypatch: pytest.MonkeyPatch) -> list[dict]:
    """Cattura le richieste di backup su S3 che verrebbero accodate su
    RabbitMQ, senza richiedere RabbitMQ in esecuzione durante i test."""
    sent: list[dict] = []

    def _fake_publish(**kwargs) -> None:
        sent.append(kwargs)

    monkeypatch.setattr("app.api.v1.posts.publish_post_backup", _fake_publish)
    return sent
