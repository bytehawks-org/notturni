# Notturni

Piattaforma di microblogging opensource, multilingua, EU centrica & GDPR compliance.

Questo file documenta la struttura del repository e come avviare l'ambiente di
sviluppo — per una guida passo passo al primo avvio locale e un primo giro di
test, vedi [GETTING_STARTED.md](GETTING_STARTED.md); per l'elenco completo
delle specifiche di prodotto e il loro stato di avanzamento, vedi
[ROADMAP.md](ROADMAP.md).

## Stack

- **Frontend:** Next.js (App Router), Tailwind CSS
- **Backend:** Python 3.12+, FastAPI, SQLAlchemy (async), Alembic
- **Autenticazione:** password (Argon2) + sessioni JWT, MFA (TOTP/email), SSO
  OAuth2/OIDC (Authlib), API token per accesso diretto/motore core
- **Database:** PostgreSQL 16+
- **Cache & State:** Redis 7+
- **Message Broker:** RabbitMQ
- **Object Storage:** MinIO (S3-compatible, via boto3)
- **Dev locale:** Podman (rootless, senza Kubernetes)
- **Produzione:** Kubernetes (K3s), Traefik (ingress), Longhorn (storage), cert-manager

## Funzionalità implementate

- **Auth completa:** registrazione/login con password, sessioni JWT (access +
  refresh con rotation), MFA via TOTP o email, SSO (Google/Microsoft/GitHub/
  LinkedIn) con account linking e gate 2FA.
- **Blog, post e commenti:** CRUD con le regole di dominio del blueprint (slug,
  limite 5 blog/utente, moderazione commenti). Post in Markdown, con workflow
  di stato completo (bozza → in revisione → pubblicato, con pianificazione
  della pubblicazione) e ruolo Revisore collegato a una capacità reale.
- **Media e backup su S3:** immagini incorporabili nei post (pubbliche) e una
  copia di backup del Markdown di ogni post ad ogni salvataggio (privata),
  entrambe sotto `s3://{bucket}/{site_slug}/userdata/{user}/{blog}/...`.
- **Multilingua (i18n):** post e pagine statiche traducibili (locale + slug per
  lingua); la struttura di path `/it/`, `/en/`, ... è una convenzione lasciata
  al routing pubblico del frontend (non ancora costruito: oggi il frontend
  seleziona la lingua via parametro, non via path).
- **Pagine statiche del sito principale:** Chi siamo, Contatti, Privacy, ecc.,
  gestite da Amministratore/Super Admin.
- **Aspetto personalizzabile per blog:** palette/tipografia/layout in JSON
  libero (`blog_configs`), con i vincoli del blueprint (max 5 colori, max 3 font).
- **Profilo utente:** bio, avatar (upload su MinIO/S3), link social.
- **Follow:** utenti che seguono altri utenti o blog.
- **Amministrazione:** gestione utenti (ruolo, attivazione) — l'assegnazione
  dei ruoli di amministrazione è riservata al Super Admin.
- **API token:** accesso diretto per il motore core, predisposto per il futuro
  utilizzo diretto da parte degli utenti.
- **Frontend:** interfaccia autore (dashboard, editor, profilo) e
  amministrativa (pagine statiche, utenti), tema chiaro/scuro/automatico
  (alba-tramonto in base alla posizione, calcolata solo lato client).
- **Self-hosting:** hostname/FQDN e modalità di installazione configurabili
  (`NOCT_INSTANCE_FQDN`, `NOCT_DEPLOYMENT_MODE=solo|platform`) — "solo" per un
  blog/sito personale a singolo proprietario (il primo utente registrato
  diventa Super Admin, la registrazione si chiude dopo), "platform" per lo
  stack multiutente descritto nel resto di questo documento. Ogni entità nel
  database ha un UUID come chiave primaria, prerequisito per un'eventuale
  federazione futura (vedi [ROADMAP.md](ROADMAP.md#5-sviluppi-futuri-non-ancora-iniziati),
  ATProto in prima battuta).

Dettagli, esempi di richiesta/risposta e limitazioni note (SSO non testabile
end-to-end senza credenziali reali, invio email OTP non collegato a un
provider SMTP) sono in [backend/API.md](backend/API.md). Per lo stato di
avanzamento di ogni funzionalità, incluse quelle ancora parziali o non
iniziate, vedi [ROADMAP.md](ROADMAP.md).

## Struttura del repository

```text
.
├── backend/            # API FastAPI
│   ├── app/
│   │   ├── core/          # configurazione (env NOCT_*), DB, sicurezza, storage S3, broker
│   │   ├── models/         # entità SQLAlchemy (User, Blog, Post, Page, Comment, Follow, ...)
│   │   ├── domain/          # regole di business (auth, mfa, sso, i18n, autorizzazione, ...)
│   │   ├── api/v1/            # router FastAPI (auth, blogs, posts, comments, pages, users, tokens)
│   │   └── workers/            # consumer RabbitMQ (backup post su S3; invio OTP email — placeholder)
│   ├── scripts/            # script di bootstrap (primo API token)
│   ├── tests/               # suite pytest (vedi sotto)
│   ├── alembic/              # migrazioni del database
│   ├── schema.sql             # schema corrente, generato da Alembic (sola documentazione)
│   └── Dockerfile
├── frontend/            # applicazione Next.js
│   ├── src/
│   │   ├── app/            # login/register, dashboard (autore), admin, profilo pubblico
│   │   ├── lib/             # client API, sessione (auth-context), tema (theme-context, sun.ts)
│   │   └── components/       # UI condivisa (Button, Card, ...) e ThemeToggle
│   └── Dockerfile
├── k8s/                  # manifest Kubernetes (primo draft)
├── compose.yaml         # stack locale via Podman/Docker compose
└── .env.example          # variabili d'ambiente (prefisso NOCT_)
```

## Avvio rapido in locale

```bash
cp .env.example .env
podman compose up -d --build
```

Servizi esposti:

- Frontend: <http://localhost:3000>
- Backend (API docs): <http://localhost:8000/docs>
- Backend (health): <http://localhost:8000/api/v1/health>
- RabbitMQ (console): <http://localhost:15672>
- MinIO (console): <http://localhost:9001>

Al primo avvio le migrazioni del database non vengono applicate automaticamente;
vanno eseguite manualmente (vedi [backend/README.md](backend/README.md)):

```bash
podman exec -it <container-backend> alembic upgrade head
```

Se `podman compose` non è disponibile (richiede `podman-compose` o il plugin
`docker-compose`), vedi [GETTING_STARTED.md](GETTING_STARTED.md#prerequisiti)
per le alternative, oppure avvia backend/frontend in locale seguendo i
rispettivi README.

## Schema del database

Lo schema (tabelle, tipi enum, vincoli) è versionato tramite le migrazioni
Alembic in `backend/alembic/versions/`. Una copia SQL di sola lettura, utile per
consultazione rapida senza dover leggere Python, è in
[backend/schema.sql](backend/schema.sql) — va rigenerata con
`alembic upgrade head --sql` ad ogni modifica dello schema, non modificata a mano.

## API e autenticazione

Riferimento di tutti gli endpoint, i due meccanismi di autenticazione
(sessione utente vs API token), formato del token e come ottenere il primo:
[backend/API.md](backend/API.md).

## Test

Suite pytest ripetibile (schema ricreato da zero e tabelle svuotate ad ogni
run — nessuno stato residuo tra un'esecuzione e l'altra):

```bash
cd backend
pip install -r requirements-dev.txt
python -m pytest
```

Serve solo Postgres raggiungibile (usa un database separato, `notturni_test`
di default); MinIO e RabbitMQ sono sostituiti da fake/mock nei test, non
servono in esecuzione. Dettagli in [backend/README.md](backend/README.md#test).

## Backend e frontend in locale (senza container)

Vedi [backend/README.md](backend/README.md) e [frontend/README.md](frontend/README.md).

## Kubernetes

Vedi [k8s/README.md](k8s/README.md).

## Specifiche e roadmap

Tabella di tutte le specifiche di prodotto con il relativo stato di
avanzamento (implementato / parziale / da fare) e il backlog dei prossimi
sviluppi: [ROADMAP.md](ROADMAP.md).
