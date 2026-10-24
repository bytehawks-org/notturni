# Migrazione backend/worker Python → Rust: analisi e piano

Branch: `migration/rust`. Il frontend (Next.js) resta invariato. Il servizio
`moderation/` (PyTorch/transformers) è trattato a parte (vedi §6).

## 1. Stato attuale (misurato)

| Area | Dimensione | Note |
|---|---|---|
| `backend/app` | ~17.7k LOC Python | 198 route in 28 moduli, `/api/v1` |
| Route più dense | `users.py` (26), `admin.py` (19), `newsletter.py` (18), `auth.py` (16), `posts.py` (12) | |
| `domain/` | ~3.4k LOC, ~45 moduli | logica di business già separata dagli handler: base ideale per i crate |
| `models/` | 35 tabelle, 18 `Enum`, `JSONB`, `ARRAY`, `INET`, `UUID` | |
| Migrazioni Alembic | 56 file (head unico dopo merge) + `schema.sql` (1143 righe) | |
| Worker | `email_otp_consumer`, `post_backup_consumer`, `newsletter_consumer` (AMQP); `audit_maintenance`, `backup` (periodici, `--loop`) | |
| Script | `garage_bootstrap`, `create_api_token` | |
| Test | ~8.6k LOC pytest, 53 file, DB + Redis reali | **oracolo di compatibilità** |
| Contratto | `backend/API.md` (2.5k righe) | il frontend dipende da questo, non da OpenAPI generato |

Dipendenze Python → equivalente Rust:

| Python | Rust |
|---|---|
| FastAPI/Uvicorn | `axum` + `tokio` + `tower-http` (CORS, trace, limit) |
| SQLAlchemy async + asyncpg | `sqlx` (Postgres, query verificate a compile-time, `migrate!`) |
| Alembic | `sqlx migrate` (SQL puro) |
| Pydantic / pydantic-settings | `serde` + `validator`; config con `figment` o `envy` (prefisso `NOCT_`) |
| argon2-cffi | `argon2` (RustCrypto) — hash PHC compatibili, nessun reset password |
| PyJWT (HS) | `jsonwebtoken` |
| pyotp / qrcode (SVG) | `totp-rs` (feature `qr`) o `totp-lite` + `qrcode` (SVG) |
| Authlib (OAuth2/OIDC) | `oauth2` + `openidconnect` |
| boto3 | `aws-sdk-s3` (path-style, endpoint custom, checksum `when_required`) |
| pika | `lapin` |
| redis-py | `redis` (tokio, `ConnectionManager`) |
| httpx | `reqwest` (rustls) |
| markdown + nh3 | `pulldown-cmark` + `ammonia` (stessa base di nh3) |
| dnspython | `hickory-resolver` |
| smtplib | `lettre` |
| itsdangerous / SessionMiddleware | `tower-sessions` oppure cookie firmato (`axum-extra` `SignedCookieJar`) |
| pg_dump nei worker | invariato: binario `pg_dump` nell'immagine, invocato via `tokio::process` |

## 2. Rischi e punti delicati

1. **Compatibilità di contratto**: 198 endpoint con payload, status code, messaggi di errore e tri-stato PATCH (`model_fields_set`) da replicare byte-per-byte. Mitigazione: suite di contract test (§5).
2. **PATCH tri-stato** → `Option<Option<T>>` con `serde_with::double_option`; da centralizzare in un tipo `Patch<T>` condiviso.
3. **Hash password/token esistenti**: argon2 PHC e sha256 (`noct_`) sono portabili; JWT HS256 con lo stesso `NOCT_JWT_SECRET` → sessioni attive sopravvivono al cutover. Verificare claim e `credentials_changed_at`.
4. **Schema DB**: Alembic è la sorgente storica. Strategia: `0001_baseline.sql` = `schema.sql` allineato all'head Alembic; le installazioni esistenti vengono "adottate" (tabella `_sqlx_migrations` pre-popolata con la baseline). Nuove migrazioni solo sqlx. Enum PG, `ARRAY`, `JSONB`, `INET` mappati a mano con `sqlx::Type`.
5. **SSO non testabile end-to-end** (nessuna credenziale OAuth reale): rischio di regressione invisibile; coprire con mock del provider (wiremock).
6. **Moderazione immagini**: PyTorch non ha un equivalente Rust maturo a parità di modello. Vedi §6.
7. **Markdown/sanitizzazione**: output HTML di `markdown` (Python) ≠ `pulldown-cmark` in dettagli (tabelle, sane_lists). Snapshot test sui contenuti reali.
8. **Compile time/CI**: workspace grande → usare `cargo-chef` nel Dockerfile, `sccache`, `SQLX_OFFLINE` (`cargo sqlx prepare`).
9. **Vincolo di sessione** (CLAUDE.md §2): un blocco per sessione. Il piano sotto è dimensionato di conseguenza.

## 3. Architettura target (workspace Cargo)

```
rust/                      # (o radice repo: da decidere, vedi §8)
├── Cargo.toml             # [workspace], dipendenze condivise
├── migrations/            # SQL sqlx, 0001_baseline.sql + incrementali
├── crates/
│   ├── noct-config/       # Settings da env NOCT_*, validazione all'avvio
│   ├── noct-db/           # pool sqlx, tipi enum, helper transazione, test util
│   ├── noct-core/         # errori (AppError → HTTP), tipi comuni, Patch<T>, paginazione
│   ├── noct-storage/      # trait Storage { put, get, delete, presign, public_url }
│   │                      #   impl S3 (aws-sdk-s3) + impl LocalFs; scelta da NOCT_STORAGE_BACKEND
│   ├── noct-cache/        # Redis: rate_limit, lock
│   ├── noct-broker/       # lapin: publish/consume, code e DLQ, retry
│   ├── noct-mail/         # lettre + template
│   ├── noct-markdown/     # pulldown-cmark + ammonia
│   ├── noct-auth/         # password, JWT, API token, sessioni, MFA (TOTP/email), SSO, extractor axum
│   ├── noct-authz/        # ruoli piattaforma + ruoli per-blog (ex domain/authorization.py)
│   ├── noct-audit/        # record() transazionale
│   ├── noct-domain/       # (opzionale) regole pure: blog_rules, usernames, permalinks, tags, i18n, seo
│   ├── api-auth/          # route /auth, /tokens
│   ├── api-users/         # /users, /interests, profilo, GDPR
│   ├── api-blogs/         # /blogs/* (crud, config, branding, members, invitations, categories, media, pages, notes, bibliography, publications, lifecycle, overview)
│   ├── api-posts/         # /posts, fragments, link_preview
│   ├── api-comments/      # commenti + reports
│   ├── api-feed/          # feed, search, pages pubbliche
│   ├── api-newsletter/
│   ├── api-admin/
│   └── api-health/
├── bins/
│   ├── noct-api/          # compone i router dei crate api-*; unico binario HTTP
│   ├── noct-worker/       # subcommand: email-otp | post-backup | newsletter | audit-maintenance | backup  (--loop/--interval)
│   └── noct-ctl/          # migrate | bootstrap-garage | create-api-token | bootstrap-super-admin
└── tests/                 # contract/integration test
```

Principi:
* Ogni `api-*` espone `pub fn router() -> Router<AppState>`; `noct-api` li monta sotto `/api/v1` → aggiungere un endpoint non tocca gli altri crate.
* `AppState` (pool, redis, broker, storage `Arc<dyn Storage>`, config) definito in `noct-core`.
* **Un solo binario worker** con subcommand → una sola immagine, N Deployment K8s (come oggi). Stesso schema per `noct-ctl` (Job/initContainer di migrazione).
* Storage: trait con due implementazioni; il `localstorage` mantiene il serving `/storage` via `tower-http::ServeDir` montato da `noct-api`. Vincoli CLAUDE.md §3 preservati (endpoint S3 da env, credenziali opzionali con default chain, `NOCT_S3_REGION`).
* Immagine: multi-stage `rust:slim` → `debian:bookworm-slim` (o `distroless/cc`) con `postgresql-client`; build multi-arch x86_64/ARM64 (`--platform`, niente tag arch-specifici), TLS con `rustls` per evitare dipendenze OpenSSL.

## 4. Strategia di migrazione: strangler, non big-bang

Rust e Python convivono dietro lo stesso ingress/proxy finché il porting non è completo:

1. `noct-api` (Rust) in ascolto su `:8000`; per le route non ancora portate fa **reverse proxy** verso il backend Python (`NOCT_LEGACY_BACKEND_URL`). Il frontend non cambia mai URL.
2. Ogni fase sposta un gruppo di endpoint in Rust e rimuove la regola di proxy corrispondente.
3. Fine: rimozione del backend Python, `requirements.txt`, Alembic, Dockerfile Python.

DB condiviso durante la transizione → **nessuna migrazione di schema incompatibile** finché il Python è vivo; Alembic resta autoritativo fino alla Fase 8, poi baseline sqlx.

## 5. Strategia di test (oracolo)

* **Contract test**: riusare i pytest esistenti come client HTTP puro contro `NOCT_API_URL` (httpx), così la stessa suite gira contro Python e Rust. I test che toccano direttamente i modelli SQLAlchemy vanno riscritti come client-only o come test Rust.
* Test Rust: `sqlx::test` (DB per-test), `testcontainers` per Postgres/Redis/RabbitMQ/Garage.
* Golden/snapshot (`insta`): markdown→HTML, feed, risposte JSON tipiche.
* CI: `cargo fmt --check`, `clippy -D warnings`, `cargo test`, `cargo deny`, `cargo audit`; CodeQL supporta Rust → aggiornare `.github/workflows`.

## 6. Moderazione immagini (decisione richiesta)

Opzioni:
* **A (consigliata ora)**: lasciare `moderation/` in Python. È già un microservizio isolato con client HTTP e *fail open*; migrarlo non porta benefici proporzionati.
* B: portare il modello in ONNX e servirlo con `ort` (ONNX Runtime) in Rust — fattibile, ma richiede conversione/validazione del modello e immagine più pesante.
* C: `candle` (HF, pure Rust) — più rischioso per parità di risultati.

Piano: A durante tutta la migrazione, rivalutare B a fine Fase 8.

## 7. Fasi (ogni fase = un blocco/sessione, con DoD)

| # | Fase | Contenuto | Definition of Done |
|---|---|---|---|
| 0 | Fondamenta | Workspace, `noct-config`, `noct-core`, `noct-db`, CI Rust, Dockerfile cargo-chef multi-arch, `/api/v1/health`, reverse proxy verso Python | `compose up` con Rust davanti a Python, health ok, tutto il resto proxato |
| 1 | Test harness | Rendere la suite pytest client-only parametrizzabile su `NOCT_API_URL`; baseline verde su Python | stessa suite verde via proxy Rust |
| 2 | Infra crate | `noct-storage` (S3 + local), `noct-cache` (rate limit), `noct-broker`, `noct-mail`, `noct-markdown` con test unit/integration | test crate verdi, parità funzionale con `core/*.py` |
| 3 | Auth | `noct-auth`, `noct-authz`, `noct-audit`, `api-auth`, `api-tokens`; cookie refresh httpOnly, rotation, MFA TOTP+email, password reset, SSO | `test_auth*`, `test_tokens*` verdi; login incrociato (token emesso da Python valido su Rust e viceversa) |
| 4 | Utenti e profilo | `api-users`, interessi, GDPR, cambio email, verifica domini | `test_users*`, `test_profile*` verdi |
| 5 | Blog | `api-blogs` (tutti i sotto-moduli), media/branding su `noct-storage` | `test_blogs*`, `test_notes*`, `test_blog_invitations` verdi |
| 6 | Contenuti | `api-posts`, fragments, link_preview, comments, reports, feed, search, pages | `test_posts*`, `test_comments*` verdi; feed identico (diff JSON) |
| 7 | Newsletter, admin, worker | `api-newsletter`, `api-admin`, `noct-worker` (5 subcommand), `noct-ctl` (garage-bootstrap, create-api-token, super admin) | tutti i worker in compose/K8s su Rust; backup/restore verificato |
| 8 | Cutover DB e pulizia | Baseline sqlx + adozione DB esistenti, rimozione proxy, Alembic, Python, aggiornamento `k8s/`, `compose.yaml`, doc (`ROADMAP.md`, `backend/API.md`, README, GETTING_STARTED) | stack completo senza Python (tranne moderation); verifica live con `curl` |
| 9 | Hardening | Benchmark vs Python (latenza, RSS), tuning pool, OpenTelemetry/tracing, `cargo audit` in CI | report prestazioni |

Ordine pensato per dipendenze: infra → auth (tutto ne dipende) → blog → contenuti → resto.

## 8. Decisioni aperte (servono prima della Fase 0)

1. **Posizione del workspace**: `backend/` sostituito in-place (storico git più pulito a fine corsa) oppure nuova cartella `rust/` accanto al Python durante la transizione (consigliato per lo strangler; rinomina a fine Fase 8).
2. **Framework**: `axum` (consigliato: ecosistema tower, manutenzione attiva) vs `actix-web`.
3. **Documentazione API**: generare OpenAPI con `utoipa` (nuovo, migliora il lavoro sul frontend) oppure mantenere solo `API.md`.
4. **Livello `sqlx`**: query SQL dirette (consigliato, schema già maturo) vs ORM (`sea-orm`).
5. **Moderazione**: opzione A/B/C del §6.
6. **Ruolo CI**: build immagini ARM64 su runner nativo o cross-compile.
