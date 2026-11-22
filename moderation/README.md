# Notturni — image moderation service

Self-hosted NSFW classifier (nudity and sensitive content), containerized
separately from the main backend. The ML dependencies (torch/transformers,
~2 GB image) do not belong in an image that must stay light and fast to
rebuild on every deploy. No image ever leaves the infrastructure, in line
with the EU-centric, GDPR-oriented design of the project (see
[docs/ABSTRACT.md](../docs/ABSTRACT.md)).

Model: [Falconsai/nsfw_image_detection](https://huggingface.co/Falconsai/nsfw_image_detection)
(ViT, Apache-2.0). It is downloaded once, at image build time (see the
`Dockerfile`), not at first start. The container needs no network access at
runtime.

The service is never exposed publicly. The backend calls it internally over
HTTP (`NOCT_MODERATION_SERVICE_URL`, set automatically by `compose.yaml`).
See `crates/noct-http` (client), `crates/noct-media` (called on every upload)
and [docs/API.md](../docs/API.md#automatic-image-moderation).
If the service is unreachable or does not answer in time, the backend
treats the image as "not sensitive" and never blocks the upload (fail open).

## Start

```bash
uvicorn app.main:app --host 0.0.0.0 --port 8100
```

Health check: `GET /health`.

## API

**`POST /classify`**: `multipart/form-data`, field `file` (the image).

```json
{"is_sensitive": false, "label": "normal", "score": 0.999}
```

`label` and `score` are the model label and confidence (possible labels:
`normal`, `nsfw`). `is_sensitive` is `true` only when `label == "nsfw"` and
`score` exceeds the configured threshold (`THRESHOLD` in `app/main.py`,
default 0.7). The threshold is not yet calibrated on a real sample. It is
only a cautious starting value.

## Local development

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install torch --index-url https://download.pytorch.org/whl/cpu
pip install -r requirements.txt
uvicorn app.main:app --reload --port 8100
```

The first start downloads the model weights (~350 MB) from Hugging Face,
unless they are already in the local cache (`~/.cache/huggingface`).
