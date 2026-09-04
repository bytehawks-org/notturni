import uuid

from sqlalchemy import ForeignKey, Text, UniqueConstraint
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.models.base import Base, TimestampMixin, UUIDPKMixin


class PostFragment(Base, UUIDPKMixin, TimestampMixin):
    """Porzione di testo evidenziata e salvata da un lettore (non l'autore) su
    un post pubblicato — raccolta unificata in /dashboard/frammenti. Il testo
    è duplicato qui, non un offset nel contenuto: la ri-evidenziazione ad
    ogni lettura successiva (frontend, ricerca del testo nel DOM reso) resta
    valida anche se il post viene poi modificato, a differenza di un offset
    di carattere che si disallineerebbe. `created_at` (TimestampMixin) è la
    data di cattura mostrata nella raccolta."""

    __tablename__ = "post_fragments"

    user_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("users.id"), nullable=False, index=True)
    post_id: Mapped[uuid.UUID] = mapped_column(ForeignKey("posts.id"), nullable=False, index=True)
    text: Mapped[str] = mapped_column(Text, nullable=False)

    user: Mapped["User"] = relationship()
    post: Mapped["Post"] = relationship()

    __table_args__ = (
        # stesso frammento salvato due volte dallo stesso utente sullo stesso
        # post: idempotente, non un duplicato (vedi create_fragment).
        UniqueConstraint("user_id", "post_id", "text", name="uq_post_fragment_user_post_text"),
    )
