# Notturni backend (Rust)

Workspace Cargo: un'API HTTP (`noct-api`), un processo di elaborazione in background (`noct-worker`) e
uno strumento di sviluppo e amministrazione (`noct-ctl`), più i crate del core e i moduli opzionali.
Postgres, NATS JetStream, Redis e uno storage S3-compatible (o il filesystem) sono i servizi esterni.
Il frontend (Next.js) resta in [`frontend/`](../frontend/README.md). Il contratto HTTP è in
[API.md](API.md).

## Struttura

```text
bins/
  noct-api/      API HTTP; ospita anche il supervisore dei moduli a caldo e i test end-to-end
  noct-worker/   relay dell'outbox, job periodici, consumer di eventi e job
  noct-ctl/      validate, migrate, openapi, token, audit, garage-bootstrap...
crates/          core: noct-core (registry, runtime, hook), manifest/codegen/migrate, auth, users, profiles,
                 blogs, posts, media, audit, events, platform, admin; infrastruttura: bus, cache, storage,
                 mail, http, markdown, security; noct-bundle (l'elenco dei moduli compilati)
modules/         moduli opzionali: taxonomy, publications, pages, link_preview, notes, fragments, comments,
                 discovery, wiki, git_sync, interests, auth_mfa, auth_sso, reports, newsletter, custom_domains, gdpr,
                 image_moderation, backup (+ sandbox, di riferimento)
```

Ogni modulo è dichiarativo: `module.toml` (identità, dipendenze, eventi, job, configurazione, hook),
`api.toml` (rotte e DTO), `db.toml` (schema). Da lì il codice generato (`noct-codegen`) produce DTO,
router e migrazioni; la logica che non è CRUD sta in handler `custom` scritti a mano. Quali moduli sono
**attivi** si decide a runtime: `NOCT_MODULES` è il seme della prima installazione, poi vale lo stato nel
database, modificabile a caldo da `/api/v1/admin/modules` senza riavvii.

## Sviluppo

```bash
# servizi di prova: Postgres 16+, Redis, NATS con JetStream (-js), un S3 finto (moto) — vedi sotto
export NOCT_TEST_DATABASE_URL=postgres://postgres@127.0.0.1:5432/postgres
export NOCT_TEST_REDIS_URL=redis://127.0.0.1:6379/0
export NOCT_TEST_NATS_URL=nats://127.0.0.1:4222
export NOCT_TEST_S3_ENDPOINT=http://127.0.0.1:5000

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked          # ogni test crea e distrugge un database suo
cargo deny check                          # licenze, fonti, vulnerabilità (RustSec)

# ciclo di sviluppo di un modulo
$EDITOR modules/taxonomy/db.toml modules/taxonomy/api.toml
cargo run -p noct-ctl -- migrate plan        # cosa cambierebbe
cargo run -p noct-ctl -- migrate generate    # scrive migrations/NNNN_*.sql + schema.lock.json (da rivedere e committare)
cargo run -p noct-ctl -- validate            # manifest e drift (per la CI)

# avvio (variabili in .env.example)
set -a; source .env; set +a
cargo run -p noct-api                        # API su :8000; con NATS i consumer vanno in noct-worker
cargo run -p noct-worker
curl localhost:8000/api/v1/health/ready
curl localhost:8000/api/v1/openapi.json
```

I test non usano Redis, NATS o S3 dove non servono (servizi in memoria o in locale); quelli che li
richiedono lo dichiarano. `pg_dump` (pacchetto `postgresql-client`) serve al test del backup. Un servizio
di prova minimo: `redis-server`, `nats-server -js`, `moto_server` (`pip install 'moto[server]'`).

Nuovo modulo: copiare `modules/sandbox` (o `taxonomy` se non servono handler), rinominare, aggiungerlo a
`Cargo.toml` (workspace) e a `crates/noct-bundle` (dipendenza opzionale + una riga in `all()`). Un modulo
può contribuire alle rotte `/admin/*` (contatori, impostazioni di piattaforma), alla configurazione
pubblica e all'esportazione del blog con `[lifecycle] admin_sections`; ai dati personali di un utente con
`personal_data`; al login con `login_gates`; ai campi di blog e post con `extensions`.

## Primo avvio

1. `scripts/gen-env.sh` (crea `.env` con segreti casuali; il backend rifiuta i segnaposto di `.env.example`) e adattare i valori. Installazione completa: [INSTALL.md](INSTALL.md).
2. `podman compose up -d --build` avvia l'intero stack (vedi [GETTING_STARTED.md](../GETTING_STARTED.md));
   oppure, fuori da compose, i servizi a mano e `cargo run -p noct-api`.
3. Le migrazioni dei moduli attivi si applicano all'avvio (advisory lock: sicuro con più repliche;
   `NOCT_AUTO_MIGRATE=false` per farlo altrove con `noct-ctl migrate apply`).
4. Il Super Admin si crea all'avvio da `NOCT_SUPER_ADMIN_*` (idempotente).
5. Un token core per l'accesso machine-to-machine: `cargo run -p noct-ctl -- token create --name core-engine`
   (il valore si mostra una sola volta; in database resta l'hash).

## Operazioni

| Cosa | Dove |
| --- | --- |
| Job periodici | `schedule = "1m"` in `module.toml`; lo accoda una sola replica per intervallo (`noct_schedules`) |
| Campagne newsletter pianificate | job `newsletter.campaigns.dispatch`, ogni minuto |
| Manutenzione dell'audit | job `audit.maintenance`, ogni giorno: archivio su storage delle settimane ISO concluse, poi cancellazione oltre `audit_retention_days`, mai oltre l'ultima settimana archiviata. `noct-ctl audit maintain`, `noct-ctl audit restore 2026w36` |
| Cancellazione dei blog | job `blogs.purge_deleted`, ogni giorno: i blog cancellati da più di 30 giorni vanno via con i loro file |
| Backup | modulo `backup`: copia del Markdown di ogni post pubblicato; ogni giorno `pg_dump` e rispecchiamento dei bucket verso uno storage S3 **esterno** (`NOCT_BACKUP_S3_*`) |
| Garage | `noct-ctl garage-bootstrap` (layout, chiave, bucket; idempotente) |
| Messaggi falliti | `noct-ctl dead-letters`; coda in uscita: `noct-ctl outbox` |
| Moduli a caldo | `GET /admin/modules`, `POST /admin/modules/{name}/enable|disable` (Super Admin); API e worker convergono entro ~5 secondi |
| Stato dei servizi | `GET /api/v1/health` (liveness), `GET /api/v1/health/ready` (database, cache, bus, storage) |

Immagine container: [`Dockerfile`](../Dockerfile) (cargo-chef, multi-arch, un'immagine con i tre binari; il
comando decide il ruolo). Manifest Kubernetes in [`k8s/`](../k8s/README.md). CI in `.github/workflows/`.

## Dalla v1: dove è finito cosa

La documentazione e le note storiche citano a volte file della v1 (Python/FastAPI), rimossi con il blocco
di chiusura della riscrittura e consultabili nella cronologia git. L'equivalente Rust:

| v1 (Python) | Rust |
| --- | --- |
| `app/api/v1/auth.py`, `domain/auth.py`, `passwords.py`, `password_reset.py` | `crates/noct-auth` |
| `app/domain/usernames.py` | `crates/noct-security/src/validate.rs` |
| `app/api/v1/users.py`, `domain/profile.py`, `email_change.py`, `fediverse.py` | `crates/noct-profiles` |
| `app/api/v1/blogs/`, `domain/blog_rules.py`, `blog_config.py`, `blog_lifecycle.py` | `crates/noct-blogs` (+ job `purge_deleted`) |
| `app/api/v1/posts.py`, `domain/permalinks.py`, `tags.py`, `seo.py`, `content_media.py` | `crates/noct-posts`, `crates/noct-media` |
| `app/domain/authorization.py`, `blog_scope.py`, `rate_limit.py`, `core/http.py` | `crates/noct-core` (`authz.rs`, `rate_limit.rs`) |
| `app/api/v1/comments.py`, `core/captcha.py` | `modules/comments` |
| `app/api/v1/feed.py`, `search.py` | `modules/discovery` |
| `app/api/v1/{pages,fragments,interests,link_preview,reports,newsletter,tokens}.py` | `modules/{pages,fragments,interests,link_preview,reports,newsletter}`, `crates/noct-auth` (token) |
| `app/domain/mfa.py`, `sso.py` | `modules/auth_mfa`, `modules/auth_sso` |
| `app/domain/custom_domains.py`, `gdpr.py`, `gdpr_queue.py` | `modules/custom_domains`, `modules/gdpr` |
| `app/api/v1/admin.py` | `crates/noct-admin` (+ rotte dei moduli: `reports`, `comments`, `gdpr`, `newsletter`) |
| `app/domain/platform_config.py`, `verification.py`, `display_names.py` | `crates/noct-platform`, `crates/noct-profiles`, `crates/noct-posts` |
| `app/domain/audit.py`, `workers/audit_maintenance.py` | `crates/noct-audit` |
| `app/workers/{email_otp,newsletter,post_backup}_consumer.py`, `backup.py` | job e consumer dei moduli, `modules/backup`; tutti nel processo `noct-worker` |
| `app/core/{storage,mail,revalidation,moderation}.py`, `redis.py`, `broker.py` | `crates/noct-{storage,mail,http,cache,bus}` |
| `scripts/garage_bootstrap.py`, `create_api_token.py` | `noct-ctl garage-bootstrap`, `noct-ctl token create` |
| `alembic/`, `schema.sql` | `crates/*/migrations/`, `modules/*/migrations/` (generate da `db.toml`) |
