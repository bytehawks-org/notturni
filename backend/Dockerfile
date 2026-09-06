# syntax=docker/dockerfile:1
FROM python:3.12-slim

WORKDIR /app

# pg_dump per il backup infrastrutturale (app/workers/backup.py) — non serve
# far combaciare la major version con il Postgres server: pg_dump supporta
# server con version <= alla propria (qui prende quella corrente di Debian,
# più recente di postgres:16 usato in compose.yaml/k8s).
RUN apt-get update && apt-get install --no-install-recommends -y postgresql-client \
    && rm -rf /var/lib/apt/lists/*

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY . .

RUN useradd --create-home --shell /usr/sbin/nologin appuser \
    && chown -R appuser:appuser /app
USER appuser

EXPOSE 8000

CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "8000"]
