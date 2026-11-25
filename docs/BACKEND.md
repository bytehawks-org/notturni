# Notturni backend (Rust)

Cargo workspace: an HTTP API (`noct-api`), a background processing process (`noct-worker`) and a
development and administration tool (`noct-ctl`), plus the core crates and the optional modules.
Postgres, NATS JetStream, Redis and an S3-compatible storage (or the filesystem) are the external services.
The frontend (Next.js) lives in [`frontend/`](../frontend/README.md). The HTTP contract is in
[API.md](API.md).

## Structure

```text
bins/
  noct-api/      HTTP API; also hosts the hot-module supervisor and the end-to-end tests
  noct-worker/   outbox relay, periodic jobs, event consumers and jobs
  noct-ctl/      validate, migrate, openapi, token, audit, user, garage-bootstrap...
crates/          core: noct-core (registry, runtime, hook), manifest/codegen/migrate, auth, users, profiles,
                 blogs, posts, media, audit, events, platform, admin; infrastructure: bus, cache, storage,
                 mail, http, markdown, security; noct-bundle (the list of compiled modules)
modules/         optional modules: taxonomy, publications, pages, link_preview, notes, fragments, comments,
                 discovery, wiki, git_sync, interests, auth_mfa, auth_sso, reports, newsletter, custom_domains, gdpr,
                 image_moderation, backup (+ sandbox, for reference)
```

Each module is declarative: `module.toml` (identity, dependencies, events, jobs, configuration, hooks),
`api.toml` (routes and DTOs), `db.toml` (schema). From these, the generated code (`noct-codegen`) produces DTOs,
routers and migrations. Logic that is not CRUD lives in hand-written `custom` handlers. Which modules are
**active** is decided at runtime: `NOCT_MODULES` seeds the first installation. After that, the state in the
database applies. You change it hot, through `/api/v1/admin/modules`, without restarts.

## Development

```bash
# test services: Postgres 16+, Redis, NATS with JetStream (-js), a fake S3 (moto) — see below
export NOCT_TEST_DATABASE_URL=postgres://postgres@127.0.0.1:5432/postgres
export NOCT_TEST_REDIS_URL=redis://127.0.0.1:6379/0
export NOCT_TEST_NATS_URL=nats://127.0.0.1:4222
export NOCT_TEST_S3_ENDPOINT=http://127.0.0.1:5000

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked          # each test creates and drops its own database
cargo deny check                          # licenses, sources, vulnerabilities (RustSec)

# module development cycle
$EDITOR modules/taxonomy/db.toml modules/taxonomy/api.toml
cargo run -p noct-ctl -- migrate plan        # what would change
cargo run -p noct-ctl -- migrate generate    # writes migrations/NNNN_*.sql + schema.lock.json (review and commit)
cargo run -p noct-ctl -- validate            # manifest and drift (for CI)

# startup (variables in .env.example)
set -a; source .env; set +a
cargo run -p noct-api                        # API on :8000; with NATS, the consumers run in noct-worker
cargo run -p noct-worker
curl localhost:8000/api/v1/health/ready
curl localhost:8000/api/v1/openapi.json
```

Tests do not use Redis, NATS or S3 where they are not needed (in-memory or local services). Tests that
need them declare it. `pg_dump` (package `postgresql-client`) is required by the backup test. A minimal
set of test services: `redis-server`, `nats-server -js`, `moto_server` (`pip install 'moto[server]'`).

New module: copy `modules/sandbox` (or `taxonomy` if no handlers are needed), rename it, and add it to
`Cargo.toml` (workspace) and to `crates/noct-bundle` (optional dependency + one line in `all()`). A module
can contribute to the `/admin/*` routes (counters, platform settings), to the public configuration and to
the blog export with `[lifecycle] admin_sections`; to a user's personal data with `personal_data`; to the
login with `login_gates`; and to blog and post fields with `extensions`.

## First start

1. `scripts/gen-env.sh` (creates `.env` with random secrets; the backend rejects the placeholders of `.env.example`), then adapt the values. Full installation: [INSTALL.md](INSTALL.md).
2. `podman compose up -d --build` starts the whole stack (see [GETTING_STARTED.md](../GETTING_STARTED.md)).
   Or, outside compose, start the services by hand and run `cargo run -p noct-api`.
3. The migrations of the active modules apply at startup (advisory lock: safe with several replicas;
   set `NOCT_AUTO_MIGRATE=false` to run them elsewhere with `noct-ctl migrate apply`).
4. The Super Admin is created at startup from `NOCT_SUPER_ADMIN_*` (idempotent).
5. A core token for machine-to-machine access: `cargo run -p noct-ctl -- token create --name core-engine`
   (the value is shown only once; the database keeps only the hash).

## Operations

| What | Where |
| --- | --- |
| Periodic jobs | `schedule = "1m"` in `module.toml`; only one replica enqueues it per interval (`noct_schedules`) |
| Scheduled newsletter campaigns | job `newsletter.campaigns.dispatch`, every minute |
| Audit maintenance | job `audit.maintenance`, daily: archives the completed ISO weeks to storage, then deletes records older than `audit_retention_days`, never beyond the last archived week. `noct-ctl audit maintain`, `noct-ctl audit restore 2026w36` |
| Blog deletion | job `blogs.purge_deleted`, daily: blogs deleted more than 30 days ago are removed together with their files |
| Backup | `backup` module: copy of the Markdown of every published post; daily `pg_dump` and mirroring of the buckets to an **external** S3 storage (`NOCT_BACKUP_S3_*`) |
| Garage | `noct-ctl garage-bootstrap` (layout, key, buckets; idempotent) |
| User accounts without SMTP | `noct-ctl user create --email E --username U` and `noct-ctl user reset-password --email E`. The password is read from the first line of stdin (echo off on a terminal), never from an argument: `printf '%s\n' "$PW" \| noct-ctl user create ...`. `create` makes an active `utente` account and prints its id; with an empty stdin it generates a random password and prints it once as `generated_password=...` on stdout; `reset-password` sets the new password, deletes every session, invalidates the access tokens already issued and refuses unknown or inactive users (generic error) and SSO-only accounts. Same policy and hashing as the API. Audit events `admin.user_created` / `admin.user_password_reset` (actor `system`, payload `via: noct-ctl`, no password). Running them needs shell access to the deployment (`podman exec`, `kubectl exec`) and `NOCT_DATABASE_URL`. API counterpart of `create`: `POST /admin/users` |
| Failed messages | `noct-ctl dead-letters`; outbound queue: `noct-ctl outbox` |
| Hot modules | `GET /admin/modules`, `POST /admin/modules/{name}/enable|disable` (Super Admin); API and worker converge within ~5 seconds |
| Service status | `GET /api/v1/health` (liveness), `GET /api/v1/health/ready` (database, cache, bus, storage) |

Container image: [`Dockerfile`](../Dockerfile) (cargo-chef, multi-arch, one image with the three binaries; the
command selects the role). Kubernetes manifests are in [`k8s/`](../k8s/README.md). CI is in `.github/workflows/`.

## Solo mode: primary blog and mode helpers

The mode is read in one place: `AppState::is_solo()` (`crates/noct-core/src/state.rs`), true when
`auth.deployment_mode` is `solo`. Use it for new mode checks. Older code may still read the
setting directly with `module_config_str`.

The primary blog is the one blog of a `solo` instance. Its code is in `crates/noct-blogs/src/primary.rs`:

- `ensure_primary_blog(conn, owner_id, site)` is idempotent. It takes the advisory lock
  `REGISTRATION_LOCK` (shared with registration), then checks the marker `platform_config.primary_blog_id`.
  The marker counts only if the blog row exists. Otherwise it creates the blog (title from
  `auth.site_name`, else the username; slug from the title, checked by `validate_slug_solo`), sets
  the marker, and writes the audit event `blog.primary_created`. The caller must run it in a transaction.
- Callers: `noct-auth` at the first registration (same transaction as the account) and `noct-auth`
  `startup.rs` (after the `NOCT_SUPER_ADMIN_*` bootstrap, for the oldest active Super Admin).
- The marker has no foreign key because `noct-platform` migrates before `blogs` exists. The migration is
  `crates/noct-platform/migrations/0005_schema_update.sql`.
- Permalinks: `crates/noct-posts/src/permalink.rs` (`permalink_for`, `Permalinks::load`) returns `/post/<slug>`
  for the primary blog in `solo` and `/<blog>/<slug>` for the others. `out.rs`, `seo.rs` and the
  bibliography handlers use it. Other modules (`modules/notes`, `fragments`, `publications`, `comments`,
  `newsletter`, `pages`) and `crates/noct-media` still build `/{blog}/{post}` by hand; they depend on the
  `308` redirect in the frontend (see ROADMAP.md, section 8).
- Erasure (GDPR) keeps owned blogs, so the marker stays valid. After `blogs.purge_deleted` removes the row,
  the next startup recreates the primary blog. A blog in the grace period (`deleted_at` set) keeps the
  marker, and `GET /config` returns `primary_blog_slug` as `null` until it is restored.

Other `solo` rules: slug checks (`rules::validate_slug_solo`, route collision list `SOLO_ROUTE_COLLISIONS`,
which must match the static frontend routes; a drift test checks it, skipping names with a dot and `[`, `(`, `_` prefixes); no reserved list in admin config;
no interests (`modules/interests`, `noct-profiles`); site texts `PUT` returns `403` (`noct-admin`).

## From v1: where things went

The documentation and the historical notes sometimes cite v1 files (Python/FastAPI). They were removed with the
closing step of the rewrite and can be found in the git history. The Rust equivalents:

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
| `app/api/v1/admin.py` | `crates/noct-admin` (+ module routes: `reports`, `comments`, `gdpr`, `newsletter`) |
| `app/domain/platform_config.py`, `verification.py`, `display_names.py` | `crates/noct-platform`, `crates/noct-profiles`, `crates/noct-posts` |
| `app/domain/audit.py`, `workers/audit_maintenance.py` | `crates/noct-audit` |
| `app/workers/{email_otp,newsletter,post_backup}_consumer.py`, `backup.py` | module jobs and consumers, `modules/backup`; all run in the `noct-worker` process |
| `app/core/{storage,mail,revalidation,moderation}.py`, `redis.py`, `broker.py` | `crates/noct-{storage,mail,http,cache,bus}` |
| `scripts/garage_bootstrap.py`, `create_api_token.py` | `noct-ctl garage-bootstrap`, `noct-ctl token create` |
| `alembic/`, `schema.sql` | `crates/*/migrations/`, `modules/*/migrations/` (generated from `db.toml`) |
