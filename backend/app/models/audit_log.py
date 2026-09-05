import enum
import uuid
from datetime import datetime

from sqlalchemy import DateTime, Enum, Index, String, func, text
from sqlalchemy.dialects.postgresql import INET, JSONB, UUID
from sqlalchemy.orm import Mapped, mapped_column

from app.models.base import Base, UUIDPKMixin


class AuditActorType(str, enum.Enum):
    """Chi ha originato l'evento. `user`: un utente autenticato via sessione.
    `core_token`/`user_token`: un accesso diretto via API token (motore core o
    futuro token utente). `system`: un processo interno senza attore umano
    (bootstrap, job schedulati). `anonymous`: richiesta non autenticata (es.
    tentativo di login fallito)."""

    USER = "user"
    CORE_TOKEN = "core_token"
    USER_TOKEN = "user_token"
    SYSTEM = "system"
    ANONYMOUS = "anonymous"


class AuditLog(Base, UUIDPKMixin):
    """Registro append-only delle azioni sensibili (autenticazione,
    amministrazione di piattaforma, moderazione).

    Niente `updated_at` né `TimestampMixin`: le righe non si modificano mai,
    `occurred_at` è l'unico istante che conta. Niente foreign key verso
    `users`/`blogs`/`posts`: una riga di audit deve sopravvivere alla
    cancellazione dell'entità che cita — per questo `actor_id`/`target_id`
    sono UUID nudi affiancati da una label testuale (`actor_label`) fotografata
    al momento dell'evento.

    Retention a database e scarico periodico su storage: blocchi successivi
    (vedi `NOCT_AUDIT_RETENTION_DAYS` e ROADMAP § 3). Qui si scrive soltanto."""

    __tablename__ = "audit_log"

    occurred_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), nullable=False, server_default=func.now(), index=True
    )

    actor_type: Mapped[AuditActorType] = mapped_column(
        Enum(
            AuditActorType,
            name="audit_actor_type",
            native_enum=True,
            values_callable=lambda enum_cls: [e.value for e in enum_cls],
        ),
        nullable=False,
    )
    actor_id: Mapped[uuid.UUID | None] = mapped_column(UUID(as_uuid=True), nullable=True)
    # snapshot leggibile dell'attore al momento dell'evento (es. "mario <m@x.eu>"):
    # resta valido anche se l'utente viene poi rinominato o cancellato
    actor_label: Mapped[str | None] = mapped_column(String(255), nullable=True)

    # verbo puntato, gerarchico: "auth.login", "user.role_change", "blog.suspended"
    action: Mapped[str] = mapped_column(String(100), nullable=False)

    target_type: Mapped[str | None] = mapped_column(String(50), nullable=True)
    target_id: Mapped[uuid.UUID | None] = mapped_column(UUID(as_uuid=True), nullable=True)
    # scope denormalizzato: per i filtri "eventi di questo blog" senza join
    blog_id: Mapped[uuid.UUID | None] = mapped_column(UUID(as_uuid=True), nullable=True)

    ip: Mapped[str | None] = mapped_column(INET, nullable=True)
    user_agent: Mapped[str | None] = mapped_column(String(500), nullable=True)

    # contesto extra specifico dell'azione (diff prima/dopo, motivazione, ...)
    payload: Mapped[dict] = mapped_column(JSONB, nullable=False, server_default=text("'{}'::jsonb"))

    __table_args__ = (
        Index("ix_audit_log_actor_id_occurred_at", "actor_id", "occurred_at"),
        Index("ix_audit_log_blog_id_occurred_at", "blog_id", "occurred_at"),
    )
