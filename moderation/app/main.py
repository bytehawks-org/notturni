"""Servizio di moderazione automatica delle immagini (nudità/contenuti
sensibili), self-hosted: nessuna immagine lascia mai l'infrastruttura,
coerente con l'impostazione EU-centrica/GDPR del progetto (CLAUDE.md).

Container separato dal backend principale apposta: le dipendenze ML
(torch/transformers, centinaia di MB) resterebbero fuori posto in
un'immagine pensata per essere leggera e veloce da ricostruire ad ogni
deploy. Il backend lo chiama internamente via HTTP (vedi
NOCT_MODERATION_SERVICE_URL in compose.yaml e app/core/moderation.py nel
backend) — non è mai esposto pubblicamente.

Modello: Falconsai/nsfw_image_detection (ViT, Apache-2.0), scaricato una
volta in fase di build dell'immagine (vedi Dockerfile) cosicché l'avvio del
container non richieda accesso di rete.
"""

import io
import logging
from contextlib import asynccontextmanager

from fastapi import FastAPI, File, HTTPException, UploadFile
from PIL import Image
from pydantic import BaseModel
from transformers import pipeline

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger("moderation")

MODEL_NAME = "Falconsai/nsfw_image_detection"
# Etichette del modello (vedi il suo config.json): classificatore binario.
SENSITIVE_LABEL = "nsfw"
# Soglia di confidenza sopra la quale un'immagine è segnalata: tarata in modo
# prudente (falsi negativi meno gravi di falsi positivi che bloccherebbero
# contenuti legittimi) — va rivista con un campione reale prima di un uso
# in produzione, non c'è ancora un dataset di validazione per questo progetto.
THRESHOLD = 0.7

_classifier = None


def get_classifier():
    global _classifier
    if _classifier is None:
        logger.info("Caricamento del modello %s...", MODEL_NAME)
        _classifier = pipeline("image-classification", model=MODEL_NAME)
        logger.info("Modello caricato.")
    return _classifier


@asynccontextmanager
async def lifespan(app: FastAPI):
    # Carica subito, non al primo /classify: i pesi sono già nella cache
    # dell'immagine (scaricati in fase di build, vedi Dockerfile), quindi
    # non serve rete — ma istanziare la pipeline resta un'operazione da
    # qualche secondo di CPU che altrimenti rallenterebbe la prima richiesta.
    get_classifier()
    yield


app = FastAPI(title="Notturni Moderation Service", lifespan=lifespan)


class ClassifyResponse(BaseModel):
    is_sensitive: bool
    label: str
    score: float


@app.get("/health")
async def health() -> dict[str, str]:
    return {"status": "ok"}


@app.post("/classify", response_model=ClassifyResponse)
async def classify(file: UploadFile = File(...)) -> ClassifyResponse:
    content = await file.read()
    try:
        image = Image.open(io.BytesIO(content)).convert("RGB")
    except Exception as exc:
        raise HTTPException(status_code=400, detail=f"Immagine non valida: {exc}") from exc

    classifier = get_classifier()
    results = classifier(image)
    top = max(results, key=lambda r: r["score"])
    is_sensitive = top["label"] == SENSITIVE_LABEL and top["score"] >= THRESHOLD
    return ClassifyResponse(is_sensitive=is_sensitive, label=top["label"], score=top["score"])
