import uuid
from datetime import datetime

from sqlalchemy import ARRAY, Boolean, DateTime, Float, ForeignKey, Integer, String, Text
from sqlalchemy.dialects.postgresql import JSONB
from sqlalchemy.orm import Mapped, mapped_column

from app.models.base import Base


class PlatformConfig(Base):
    """Impostazioni di piattaforma modificabili dal Super Admin (todo/
    UX_REDESIGN.md B6, mockup 5f): una sola riga (`id=1`), creata al primo
    accesso con i default dall'ambiente (NOCT_DEFAULT_LOCALE, ...). I valori
    infrastrutturali (NOCT_*) restano di sola lettura via env."""

    __tablename__ = "platform_config"

    id: Mapped[int] = mapped_column(Integer, primary_key=True, default=1)
    default_locale: Mapped[str] = mapped_column(String(2), nullable=False, default="it")
    # open: chiunque; invite: solo con invito (non ancora implementato, equivale
    # a chiusa con messaggio diverso); closed: nessuna registrazione
    registration_mode: Mapped[str] = mapped_column(String(10), nullable=False, default="open")
    # provider SSO abilitati (tra quelli configurati via env); vuoto = tutti i configurati
    sso_providers: Mapped[list[str]] = mapped_column(ARRAY(String(20)), nullable=False, default=list)
    mfa_required_for_admins: Mapped[bool] = mapped_column(Boolean, nullable=False, default=False)
    # nomi di blog riservati in aggiunta alla blacklist di codice (blog_rules.py)
    reserved_blog_names: Mapped[list[str]] = mapped_column(ARRAY(String(63)), nullable=False, default=list)
    moderation_threshold: Mapped[float] = mapped_column(Float, nullable=False, default=0.8)
    max_blogs_per_user: Mapped[int] = mapped_column(Integer, nullable=False, default=5)
    anonymous_comments_allowed: Mapped[bool] = mapped_column(Boolean, nullable=False, default=True)
    # Giorni di conservazione degli eventi in `audit_log` prima della
    # cancellazione (app/workers/audit_maintenance.py::prune), seminata da
    # NOCT_AUDIT_RETENTION_DAYS alla creazione della riga — poi controllata
    # solo da qui, l'env resta il default per un'installazione nuova.
    audit_retention_days: Mapped[int] = mapped_column(Integer, nullable=False, default=105)
    # Footer mostrato su ogni pagina pubblica, di piattaforma e di ogni blog
    # (richiesta esplicita): 3 colonne di Markdown libero (immagini/link) più
    # una barra inferiore. Colonne 1/2 sono il default, il proprietario di un
    # blog può sovrascriverle per il proprio (`blog_configs.footer`, vedi
    # app/domain/blog_config.py) — la colonna 3 e la barra inferiore restano
    # sempre e solo di piattaforma, nessun override possibile da nessun blog.
    footer_column1_markdown: Mapped[str | None] = mapped_column(Text, nullable=True)
    footer_column2_markdown: Mapped[str | None] = mapped_column(Text, nullable=True)
    footer_column3_markdown: Mapped[str | None] = mapped_column(Text, nullable=True)
    footer_bottom_bar_markdown: Mapped[str | None] = mapped_column(Text, nullable=True)
    # Elenco di interessi selezionabili dagli utenti (blocco "interessi",
    # massimo 5 a testa — app/domain/interests.py): `[{"key": "music",
    # "translations": {"it": "Musica", "en": "Music"}}, ...]`. Seminato da
    # NOCT_DEFAULT_INTERESTS o dai default builtin alla creazione della riga,
    # poi modificabile liberamente da un Super Admin (non solo aggiunte in
    # coda a un elenco builtin, a differenza di reserved_blog_names: qui non
    # c'è un vincolo di sicurezza/namespace da preservare).
    interests: Mapped[list[dict]] = mapped_column(JSONB, nullable=False, default=list)
    # Personalizzazione delle email del digest di piattaforma (blog_id=None,
    # app/workers/newsletter_consumer.py) — stesso schema delle colonne
    # equivalenti su Blog per le campagne di un singolo blog.
    newsletter_sender_name: Mapped[str | None] = mapped_column(String(120), nullable=True)
    newsletter_banner_url: Mapped[str | None] = mapped_column(String(2048), nullable=True)
    newsletter_banner_alt_text: Mapped[str] = mapped_column(String(300), default="", nullable=False)
    # Spazio massimo (media + backup Markdown, app/core/storage.py::blog_storage_bytes)
    # consentito per ogni singolo blog, in MB. NULL = nessun limite (default,
    # comportamento invariato per le installazioni esistenti). Applicato in
    # app/domain/storage_quota.py, controllato prima di ogni upload di media/
    # cover image di un blog.
    max_blog_storage_mb: Mapped[int | None] = mapped_column(Integer, nullable=True)
    # Sigilli di verifica del profilo (User.verification_tier,
    # app/domain/verification.py) gestiti a mano dal Super Admin, in aggiunta
    # al BRONZE automatico da dominio custom verificato:
    # - GOLD: entità verificate manualmente dalla piattaforma (testate
    #   giornalistiche, agenzie, organizzazioni, personalità note), elenco di
    #   username, domini email o singole caselle email.
    # - SILVER: sostenitori economici del progetto, elenco esplicito di
    #   email/username inserito a mano (nessuna integrazione con un sistema
    #   di pagamento, solo l'elenco) — migrazione b4c5d6e7f8a9: invertito
    #   con GOLD rispetto alla prima versione di questo blocco, richiesta
    #   esplicita.
    # - BLUE: chiunque si registri con un'email il cui dominio è quello della
    #   piattaforma stessa (NOCT_PLATFORM_DOMAIN) o uno di questi domini
    #   aggiuntivi, tipicamente per organizzazioni/aziende partner.
    # Voci confrontate case-insensitive; un'email intera o un username in
    # queste liste è considerata match esatto, una voce senza "@" con un "."
    # è trattata come dominio (matcha il dominio dell'email dell'utente).
    verification_gold_identifiers: Mapped[list[str]] = mapped_column(ARRAY(String(255)), nullable=False, default=list)
    verification_silver_identifiers: Mapped[list[str]] = mapped_column(ARRAY(String(255)), nullable=False, default=list)
    verification_blue_domains: Mapped[list[str]] = mapped_column(ARRAY(String(255)), nullable=False, default=list)
    updated_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)
    updated_by_id: Mapped[uuid.UUID | None] = mapped_column(ForeignKey("users.id"), nullable=True)
