import enum
import uuid

from sqlalchemy import Enum, ForeignKey, String, UniqueConstraint
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.models.base import Base, TimestampMixin, UUIDPKMixin


class SsoProvider(str, enum.Enum):
    """Provider attivi (CLAUDE.md #3)."""

    GOOGLE = "google"
    MICROSOFT = "microsoft"
    GITHUB = "github"
    LINKEDIN = "linkedin"


class SsoIdentity(Base, UUIDPKMixin, TimestampMixin):
    """Collegamento tra un utente e la sua identità presso un provider SSO.

    L'account linking (CLAUDE.md #3) risolve i conflitti in base all'email:
    se l'email del payload SSO corrisponde a un utente esistente si collega
    questa identità a quell'utente (previa verifica 2FA se configurata),
    altrimenti se ne crea uno nuovo — vedi app/domain/sso.py."""

    __tablename__ = "sso_identities"

    user_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), nullable=False)
    provider: Mapped[SsoProvider] = mapped_column(
        Enum(
            SsoProvider,
            name="sso_provider",
            native_enum=True,
            values_callable=lambda enum_cls: [e.value for e in enum_cls],
        ),
        nullable=False,
    )
    provider_user_id: Mapped[str] = mapped_column(String(255))
    email: Mapped[str] = mapped_column(String(255))

    user: Mapped["User"] = relationship(back_populates="sso_identities")

    __table_args__ = (
        UniqueConstraint("provider", "provider_user_id", name="uq_sso_identity_provider_user"),
    )
