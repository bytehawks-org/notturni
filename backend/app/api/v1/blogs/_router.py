from fastapi import APIRouter

# Router unico condiviso da tutti i sotto-moduli del package `blogs` (crud,
# config, media, categories, bibliography, pages, invitations, members). Vive
# in un modulo a sé — non in __init__ — per evitare import circolari: i
# sotto-moduli fanno `from app.api.v1.blogs._router import router`, l'__init__
# li importa a sua volta per registrarne le rotte.
router = APIRouter()
