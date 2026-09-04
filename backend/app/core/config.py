from typing import Literal

from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_prefix="NOCT_", env_file=".env", extra="ignore")

    app_env: str = "local"

    # Identità dell'istanza (self-hosting): hostname/FQDN pubblico e nome
    # breve usato come namespace nei path S3 (vedi minio_bucket_content sotto).
    instance_fqdn: str = "notturni.eu"
    site_slug: str = "notturni"
    # "solo": pensata per un singolo proprietario (blog/sito personale) — la
    # registrazione si chiude dopo il primo utente. "platform": multiutente,
    # comportamento CLAUDE.md di default. Vedi app/domain/auth.py.
    deployment_mode: Literal["solo", "platform"] = "platform"

    # Bootstrap del primo Super Admin all'avvio del backend (CLAUDE.md #5),
    # per accedere all'area di amministrazione del dashboard senza
    # promuoverlo a mano sul database. Tutte e tre richieste per attivarlo;
    # vedi app/domain/auth.py::bootstrap_super_admin.
    super_admin_username: str | None = None
    super_admin_email: str | None = None
    super_admin_password: str | None = None

    postgres_user: str
    postgres_password: str
    postgres_db: str
    postgres_host: str = "localhost"
    postgres_port: int = 5432

    rabbitmq_user: str = "admin"
    rabbitmq_password: str = "foo"
    rabbitmq_host: str = "localhost"
    rabbitmq_port: int = 5672

    # sessioni utente (JWT access token + refresh token opaco, vedi app/core/security.py)
    jwt_secret: str
    jwt_access_token_ttl_minutes: int = 15
    jwt_refresh_token_ttl_days: int = 30
    jwt_mfa_challenge_ttl_minutes: int = 5

    # richiesta da Authlib/Starlette per il flow OAuth2 (state/nonce in sessione firmata)
    session_secret: str

    oauth_redirect_base_url: str = "http://localhost:8000"
    oauth_google_client_id: str | None = None
    oauth_google_client_secret: str | None = None
    oauth_microsoft_client_id: str | None = None
    oauth_microsoft_client_secret: str | None = None
    oauth_github_client_id: str | None = None
    oauth_github_client_secret: str | None = None
    oauth_linkedin_client_id: str | None = None
    oauth_linkedin_client_secret: str | None = None

    # Storage S3-compatible (CLAUDE.md #4: endpoint custom sempre iniettato,
    # per compatibilità trasparente tra MinIO locale e AWS/Cloudflare in prod)
    minio_endpoint_url: str = "http://localhost:9000"
    # URL usato per costruire i link pubblici agli oggetti: in locale coincide
    # con l'endpoint, in produzione può essere un dominio/CDN diverso davanti al bucket
    minio_public_url: str | None = None
    minio_root_user: str = "admin"
    minio_root_password: str = "foo"
    minio_bucket_avatars: str = "avatars"
    # media incorporati nei post + backup del markdown (path: {site_slug}/userdata/{user}/{blog}/...)
    minio_bucket_content: str = "notturni-content"

    # Servizio di moderazione automatica delle immagini (self-hosted, vedi
    # moderation/), chiamato da app/domain/moderation.py all'upload di un
    # media. None: moderazione disattivata (nessuna chiamata, mai bloccante).
    moderation_service_url: str | None = None

    # origini ammesse per le chiamate del frontend dal browser (CORS), separate da virgola
    cors_origins: str = "http://localhost:3000"

    @property
    def cors_allowed_origins(self) -> list[str]:
        return [origin.strip() for origin in self.cors_origins.split(",") if origin.strip()]

    @property
    def minio_public_base_url(self) -> str:
        return self.minio_public_url or self.minio_endpoint_url

    @property
    def database_url(self) -> str:
        return (
            f"postgresql+asyncpg://{self.postgres_user}:{self.postgres_password}"
            f"@{self.postgres_host}:{self.postgres_port}/{self.postgres_db}"
        )

    @property
    def rabbitmq_url(self) -> str:
        return (
            f"amqp://{self.rabbitmq_user}:{self.rabbitmq_password}"
            f"@{self.rabbitmq_host}:{self.rabbitmq_port}/"
        )


settings = Settings()
