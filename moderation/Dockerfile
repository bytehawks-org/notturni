# syntax=docker/dockerfile:1
FROM python:3.12-slim

WORKDIR /app

# HF_HOME sotto /app: la cache dei pesi del modello, scaricata in fase di
# build qui sotto, finisce così nello stesso chown finale verso appuser.
ENV HF_HOME=/app/.cache/huggingface

COPY requirements.txt .
# torch dall'indice CPU-only di PyTorch (funziona anche su ARM64, per la
# produzione — CLAUDE.md #4: Dockerfile agnostici rispetto all'architettura):
# quello di default su PyPI porta con sé l'intero stack CUDA per niente, un
# inutile ~7 GB in un servizio pensato per girare su CPU.
RUN pip install --no-cache-dir torch==2.9.1 --index-url https://download.pytorch.org/whl/cpu \
    && pip install --no-cache-dir -r requirements.txt

COPY . .

# Scarica una volta i pesi del modello (Falconsai/nsfw_image_detection) in
# fase di build, non al primo avvio: il container non ha bisogno di accesso
# di rete a runtime, coerente con l'uso self-hosted (CLAUDE.md, EU/GDPR).
RUN python -c "from app.main import get_classifier; get_classifier()"

RUN useradd --create-home --shell /usr/sbin/nologin appuser \
    && chown -R appuser:appuser /app
USER appuser

EXPOSE 8100

CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "8100"]
