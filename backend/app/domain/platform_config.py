"""Impostazioni di piattaforma persistite (todo/UX_REDESIGN.md B6): lettura
con creazione lazy della riga di default e helper di enforcement usati da
registrazione, creazione blog, commenti anonimi, SSO e accesso admin."""

from datetime import datetime, timezone

from sqlalchemy.ext.asyncio import AsyncSession

from app.core.config import settings
from app.domain.blog_rules import MAX_BLOGS_PER_USER
from app.models.platform_config import PlatformConfig

REGISTRATION_MODES = ("open", "invite", "closed")
SUPPORTED_LOCALES = ("it", "en")
SSO_PROVIDER_NAMES = ("google", "microsoft", "github", "linkedin")
# app/workers/audit_maintenance.py::prune: sotto una settimana il valore
# viene ignorato (nessuna cancellazione) — stesso limite imposto qui in
# validazione, così l'admin lo scopre subito invece che in un log del worker.
MIN_AUDIT_RETENTION_DAYS = 7
MAX_AUDIT_RETENTION_DAYS = 3650


async def get_platform_config(session: AsyncSession) -> PlatformConfig:
    config = await session.get(PlatformConfig, 1)
    if config is None:
        config = PlatformConfig(
            id=1,
            default_locale=settings.default_locale,
            registration_mode="open",
            sso_providers=[],
            mfa_required_for_admins=False,
            reserved_blog_names=[],
            moderation_threshold=0.8,
            max_blogs_per_user=MAX_BLOGS_PER_USER,
            anonymous_comments_allowed=True,
            audit_retention_days=settings.audit_retention_days,
        )
        session.add(config)
        await session.commit()
        await session.refresh(config)
    return config


def touch(config: PlatformConfig, *, by_id) -> None:
    config.updated_at = datetime.now(timezone.utc)
    config.updated_by_id = by_id
