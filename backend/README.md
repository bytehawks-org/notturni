# Notturni backend

FastAPI + SQLAlchemy (async) + Alembic + PostgreSQL.

## Setup

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
```

Copiare `.env.example` (nella root del repo) in `.env` e avviare l'infra locale (`podman compose up -d` dalla root — vedi [GETTING_STARTED.md](../GETTING_STARTED.md) per i dettagli e le alternative).

## Migrazioni

```bash
alembic revision --autogenerate -m "descrizione"
alembic upgrade head
```

Dopo ogni modifica allo schema, rigenerare anche il corpo di `schema.sql` (sola
documentazione, non usato per applicare lo schema — mantenere l'intestazione con
il commento esplicativo):

```bash
alembic upgrade head --sql
```

## Avvio

```bash
uvicorn app.main:app --reload --port 8000
```

Health-check (verifica anche la connessione al DB): `GET /api/v1/health`.

## API e autenticazione

Ci sono due meccanismi di autenticazione distinti (stesso header
`Authorization: Bearer`, formati e scopi diversi):

- **Sessione utente** (login con password, MFA TOTP/email, SSO) — richiesta da
  `/auth/me`, `/auth/mfa/*`, `/blogs`, `/posts`, `/comments`.
- **API token** (accesso diretto/motore core) — richiesta da `/tokens`.

Il primo API token va creato con lo script di bootstrap:

```bash
python -m scripts.create_api_token --name "core-engine"
```

Per l'MFA via email serve RabbitMQ in esecuzione; il consumer che "invia" il
codice (in realtà solo lo logga: nessun provider email reale è configurato) si
avvia con:

```bash
python -m app.workers.email_otp_consumer
```

L'upload avatar (`POST /users/me/avatar`) e i media incorporati nei post
(`POST /blogs/{slug}/media`) richiedono il backend di storage configurato
(`NOCT_STORAGE_BACKEND`, default `s3`) raggiungibile. Con `s3` (MinIO in
locale): i bucket (`avatars`, `notturni-content`) vengono creati
automaticamente al primo upload, con policy pubblica in lettura (solo sul
prefisso `.../media/...` per il bucket contenuti — i backup dei post restano
privati). Con `localstorage`: i file vengono scritti su
`NOCT_LOCAL_STORAGE_BASE_PATH` e serviti direttamente dal backend su
`/storage`, nessun bucket/policy da creare.

Ogni post creato/modificato accoda anche un backup del suo Markdown su S3
(RabbitMQ, coda `post_backup`): senza il worker seguente in esecuzione i
messaggi restano semplicemente in coda, il salvataggio del post non fallisce
mai per questo:

```bash
python -m app.workers.post_backup_consumer
```

Endpoint disponibili, esempi di richiesta/risposta e regole di autorizzazione
sono documentati in [API.md](API.md).

## Test

```bash
pip install -r requirements-dev.txt
```

Serve un Postgres raggiungibile (stessa istanza di sviluppo va bene: i test
usano un database separato, `notturni_test` di default — vedi `.env.test`,
valori fissi e non sensibili, già pronto senza doverlo copiare). Non serve né
lo storage S3/MinIO né RabbitMQ: nei test sono sostituiti da fake/mock (vedi
`tests/conftest.py`).

```bash
python -m pytest
```

Ad ogni run lo schema di `notturni_test` viene ricreato da zero dai modelli
correnti (non da Alembic) e ogni test parte da tabelle vuote — ripetibile
quante volte serve, non lascia stato tra un run e l'altro. Copre: auth
(password, refresh/rotation, MFA TOTP ed email, username riservati), account
linking SSO (logica di dominio, non l'HTTP — vedi limitazione in `API.md`),
API token, blog/post (incluse le regole di dominio e le traduzioni i18n),
commenti (moderazione), pagine statiche, profilo utente (bio, social link,
avatar) e follow.
