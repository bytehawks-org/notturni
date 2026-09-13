import uuid
from datetime import datetime

from sqlalchemy import ARRAY, Boolean, DateTime, Float, ForeignKey, Integer, String
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
    updated_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True), nullable=True)
    updated_by_id: Mapped[uuid.UUID | None] = mapped_column(ForeignKey("users.id"), nullable=True)
