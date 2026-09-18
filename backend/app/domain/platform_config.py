"""Impostazioni di piattaforma persistite (todo/UX_REDESIGN.md B6): lettura
con creazione lazy della riga di default e helper di enforcement usati da
registrazione, creazione blog, commenti anonimi, SSO e accesso admin."""

from datetime import datetime, timezone

from sqlalchemy.dialects.postgresql import insert as pg_insert
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
# Footer di piattaforma (colonne 1-3 + barra inferiore) e, per le sole
# colonne 1/2, il loro override a livello di blog (app/domain/blog_config.py):
# stesso limite in entrambi i punti.
MAX_FOOTER_MARKDOWN_LENGTH = 5000


async def get_platform_config(session: AsyncSession) -> PlatformConfig:
    config = await session.get(PlatformConfig, 1)
    if config is None:
        # ON CONFLICT DO NOTHING invece di un add()+commit() diretto: su
        # un'installazione appena avviata, due prime richieste concorrenti
        # possono entrambe superare il `get` sopra prima che una delle due
        # faccia commit — un insert semplice fa fallire la seconda con un
        # IntegrityError sulla PK invece di limitarsi a non fare nulla e
        # rileggere la riga che l'altra ha appena creato.
        stmt = (
            pg_insert(PlatformConfig)
            .values(
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
                # Stesso contenuto che la SiteFooter mostrava in modo fisso
                # prima di questo blocco (link al repository + "fatto in
                # UE"): seminato qui solo per non perdere quella riga su
                # un'installazione nuova, resta comunque un campo come gli
                # altri, modificabile o azzerabile da un Super Admin in
                # qualsiasi momento.
                footer_bottom_bar_markdown="[Notturni su GitHub](https://github.com/bytehawks-org/notturni) · 🇪🇺 Fatto in UE",
            )
            .on_conflict_do_nothing(index_elements=["id"])
        )
        await session.execute(stmt)
        await session.commit()
        config = await session.get(PlatformConfig, 1)
        assert config is not None
    return config


def touch(config: PlatformConfig, *, by_id) -> None:
    config.updated_at = datetime.now(timezone.utc)
    config.updated_by_id = by_id
