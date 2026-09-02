# Notturni — servizio di moderazione immagini

Classificatore NSFW self-hosted (nudità/contenuti sensibili), containerizzato
separatamente dal backend principale: le dipendenze ML (torch/transformers,
~2 GB di immagine) resterebbero fuori posto in un'immagine pensata per
essere leggera e veloce da ricostruire ad ogni deploy. Nessuna immagine
lascia mai l'infrastruttura — coerente con l'impostazione EU-centrica/GDPR
del progetto (vedi [CLAUDE.md](../CLAUDE.md)).

Modello: [Falconsai/nsfw_image_detection](https://huggingface.co/Falconsai/nsfw_image_detection)
(ViT, Apache-2.0) — scaricato una volta in fase di build dell'immagine (vedi
`Dockerfile`), non al primo avvio: il container non ha bisogno di accesso
di rete a runtime.

Non è mai esposto pubblicamente: il backend lo chiama internamente via HTTP
(`NOCT_MODERATION_SERVICE_URL`, impostato automaticamente da
`compose.yaml`) — vedi `backend/app/domain/moderation.py` e
[backend/API.md](../backend/API.md#moderazione-automatica-delle-immagini).
Se il servizio non è raggiungibile o non risponde in tempo, il backend
assume "non sensibile" e non blocca mai l'upload (fail open).

## Avvio

```bash
uvicorn app.main:app --host 0.0.0.0 --port 8100
```

Health-check: `GET /health`.

## API

**`POST /classify`** — `multipart/form-data`, campo `file` (immagine).

```json
{"is_sensitive": false, "label": "normal", "score": 0.999}
```

`label`/`score` sono l'etichetta e la confidenza del modello (etichette
possibili: `normal`, `nsfw`); `is_sensitive` è `true` solo se `label ==
"nsfw"` e `score` supera la soglia configurata (`THRESHOLD` in
`app/main.py`, 0.7 di default — non ancora tarata su un campione reale,
solo una scelta prudente di partenza).

## Sviluppo locale

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install torch --index-url https://download.pytorch.org/whl/cpu
pip install -r requirements.txt
uvicorn app.main:app --reload --port 8100
```

Il primo avvio scarica i pesi del modello (~350 MB) da Hugging Face se non
già in cache locale (`~/.cache/huggingface`).
