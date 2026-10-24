"""Elenco pubblico degli interessi di piattaforma (blocco "interessi
utente"): usato dal selettore in `dashboard/profile` e dal filtro della
directory utenti (`GET /users?interest=...`). La sola sorgente di verità è
`platform_config.interests` — vedi app/domain/interests.py e
app/api/v1/admin.py per l'endpoint di modifica (solo Super Admin)."""

from fastapi import APIRouter, Depends
from pydantic import BaseModel
from sqlalchemy.ext.asyncio import AsyncSession

from app.core.database import get_session
from app.domain.platform_config import get_platform_config

router = APIRouter()


class InterestOut(BaseModel):
    key: str
    # {locale: etichetta} — il frontend risolve la lingua corrente da sé,
    # con fallback allo username-like `key` se manca la traduzione.
    translations: dict[str, str]


@router.get("", response_model=list[InterestOut])
async def list_interests(session: AsyncSession = Depends(get_session)) -> list[InterestOut]:
    config = await get_platform_config(session)
    return [InterestOut(**item) for item in config.interests]
