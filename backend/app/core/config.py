from typing import Literal

from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_prefix="NOCT_", env_file=".env", extra="ignore")

    app_env: str = "local"

    # Identità dell'istanza (self-hosting): hostname/FQDN pubblico e nome
    # breve usato come namespace nei path S3 (vedi s3_bucket_content sotto).
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

    # Backend di storage per media/avatar/backup post: "s3" (default, vedi
    # sotto — MinIO/AWS S3/Cloudflare R2, qualunque endpoint S3-compatible)
    # oppure "localstorage" (filesystem locale, servito direttamente dal
    # backend su /storage — pensato per installazioni "solo" self-hosted
    # senza hardware/competenze per gestire uno storage S3-compatible).
    storage_backend: Literal["s3", "localstorage"] = "s3"

    # Storage S3-compatible (CLAUDE.md #4: endpoint custom sempre iniettato,
    # per compatibilità trasparente tra MinIO locale e AWS/Cloudflare in
    # prod). Auth: access key + secret se entrambe valorizzate, altrimenti
    # ruolo AWS (default credential chain di boto3: instance profile/IRSA,
    # variabili AWS_* d'ambiente, ecc. — vedi get_s3_client in storage.py).
    s3_endpoint_url: str | None = "http://localhost:9000"
    # URL usato per costruire i link pubblici agli oggetti: in locale coincide
    # con l'endpoint, in produzione può essere un dominio/CDN diverso davanti al bucket
    s3_public_url: str | None = None
    s3_region: str | None = None
    s3_access_key_id: str | None = "admin"
    s3_secret_access_key: str | None = "foo"
    s3_bucket_avatars: str = "avatars"
    # media incorporati nei post + backup del markdown (path: {site_slug}/userdata/{user}/{blog}/...)
    s3_bucket_content: str = "notturni-content"

    # Storage su filesystem locale, alternativa a "s3" sopra quando
    # storage_backend="localstorage". base_path: radice su disco (montare un
    # volume persistente se il container viene ricreato). public_url: base
    # URL sotto cui il backend serve quei file (StaticFiles su /storage, vedi
    # app/main.py) — deve essere raggiungibile dal browser, non solo in rete
    # interna.
    local_storage_base_path: str = "./data/storage"
    local_storage_public_url: str = "http://localhost:8000/storage"

    # Servizio di moderazione automatica delle immagini (self-hosted, vedi
    # moderation/), chiamato da app/domain/moderation.py all'upload di un
    # media. None: moderazione disattivata (nessuna chiamata, mai bloccante).
    moderation_service_url: str | None = None

    # Audit log (app/domain/audit.py + app/workers/audit_maintenance.py).
    # retention_days: quanti giorni di eventi restano nel database prima della
    # cancellazione periodica. Il job non cancella comunque mai eventi non
    # ancora archiviati su storage (vedi audit_archive_enabled): la finestra
    # non archiviata fa da limite duro, retention_days è solo l'obiettivo.
    # Default 105 = ~15 settimane, così sono sempre presenti almeno 90 giorni.
    audit_retention_days: int = 105
    # Scarico periodico degli eventi su storage (S3/localstorage) per settimane
    # ISO chiuse, in NDJSON gzippato, prima della cancellazione dal database.
    # A False: nessun archivio, il job cancella solo in base a retention_days.
    audit_archive_enabled: bool = True
    # Bucket dedicato agli archivi di audit: sempre privato, mai servito ai
    # visitatori (a differenza di s3_bucket_avatars/_content).
    s3_bucket_audit: str = "notturni-audit"

    # origini ammesse per le chiamate del frontend dal browser (CORS), separate da virgola
    cors_origins: str = "http://localhost:3000"

    # Invalidazione on-demand della cache dei Server Component del frontend
    # (Next.js) dopo una modifica a contenuti pubblici (post, config/impostazioni
    # blog, pagine statiche). Il backend fa una POST fire-and-forget a
    # `frontend_revalidate_url` con l'elenco dei tag da invalidare; il secret
    # deve coincidere con `NOCT_REVALIDATE_SECRET` del servizio frontend. Se
    # una delle due non è valorizzata la chiamata è disattivata e il frontend
    # si affida alla sola rivalidazione a tempo. Vedi app/core/revalidation.py.
    frontend_revalidate_url: str | None = None
    revalidate_secret: str | None = None

    @property
    def cors_allowed_origins(self) -> list[str]:
        return [origin.strip() for origin in self.cors_origins.split(",") if origin.strip()]

    @property
    def s3_public_base_url(self) -> str:
        return self.s3_public_url or self.s3_endpoint_url or ""

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
