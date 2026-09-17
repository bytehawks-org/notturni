from sqlalchemy import Boolean, String, Text
from sqlalchemy.orm import Mapped, mapped_column

from app.models.base import Base, TimestampMixin, UUIDPKMixin


class LinkPreviewCache(Base, UUIDPKMixin, TimestampMixin):
    """Anteprima Open Graph di un URL esterno, salvata invece di rifare il
    fetch a ogni rendering (todo/EDITOR.md, app/domain/link_preview.py) — una
    sola riga per URL, condivisa da qualunque post/utente la citi (dedup
    naturale sull'unique constraint su `url_hash`). `updated_at`
    (TimestampMixin) è anche "ultima volta rifatto il fetch": non serve una
    colonna dedicata.

    `url` non è indicizzato direttamente (può superare il limite di un indice
    btree per URL molto lunghi): `url_hash` (sha256 esadecimale, lunghezza
    fissa) è la vera chiave di dedup/lookup, calcolata da
    app/domain/link_preview.py::_url_hash."""

    __tablename__ = "link_preview_cache"

    url_hash: Mapped[str] = mapped_column(String(64), unique=True, index=True, nullable=False)
    url: Mapped[str] = mapped_column(Text, nullable=False)
    title: Mapped[str | None] = mapped_column(Text, nullable=True)
    description: Mapped[str | None] = mapped_column(Text, nullable=True)
    image: Mapped[str | None] = mapped_column(Text, nullable=True)
    # False se l'ultimo fetch non ha trovato dati Open Graph reali (host
    # irraggiungibile, non HTML, nessun meta tag, ecc. — vedi fetch_link_preview):
    # una riga "vuota" così fatta viene comunque messa in cache (evita di
    # martellare un host che non risponde mai), ma con un margine di
    # freschezza più corto prima di riprovare — vedi
    # app/domain/link_preview.py::STALE_AFTER_OK/STALE_AFTER_EMPTY.
    fetch_ok: Mapped[bool] = mapped_column(Boolean, nullable=False, default=False)
