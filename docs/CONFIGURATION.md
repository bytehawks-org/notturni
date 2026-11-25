# Configuration: environment variables

Reference for all `NOCT_*` variables (plus the two `NEXT_PUBLIC_*` variables of the frontend). The template is `.env.example`. With
`podman compose up`, `compose.yaml` overrides many values with the internal service names. In Kubernetes, they live in
`k8s/configmap.yaml` (non-sensitive values) and `k8s/secret.yaml` (secrets).

- The **Scope** column shows who reads the variable: `core` (platform, `crates/noct-core/src/config.rs`) or the module that declares it in the `[config]` section of its `module.toml`. A name belongs to one module only.
- **Secret**: do not commit it, do not log it. In Kubernetes, it goes in the `Secret`. `scripts/gen-env.sh` generates random values.
- An empty value is the same as an unset variable.
- `cargo run -p noct-ctl -- modules` lists the compiled modules. `noct-ctl validate` checks manifests and schema.
- At startup, the backend rejects the placeholders of `.env.example` (`NOCT_JWT_SECRET`, the Super Admin password).
- The test module `sandbox` (`NOCT_SANDBOX_*`) exists only in tests and is not documented here.


## Instance identity

| Variable | Scope | Default | Required | Description | Example |
|---|---|---|---|---|---|
| `NOCT_INSTANCE_FQDN` | core | `notturni.eu` | no | Public domain: base of the links in emails and of the federation identifiers, email domain of the verification seal, and the domain that the link map uses to recognize internal links between posts (must match `NOCT_SITE_URL`). | `notturni.example.com` |
| `NOCT_SITE_SLUG` | core | `notturni` | no | Namespace of the paths on S3: prevents collisions between several installations on the same bucket. Lowercase letters, digits and `-` only. | `staging` |
| `NOCT_DEPLOYMENT_MODE` | auth | `platform` | no | `platform` = multi-user. `solo` = one owner: the first user becomes Super Admin, and registration closes (further accounts: see [Collaborators in solo mode](#collaborators-in-solo-mode)). The frontend reads it at runtime too. `.env.example` and the Helm chart default to `solo`. See [Deployment profiles](#deployment-profiles-solo-and-platform). | `solo` |
| `NOCT_SUBDOMAINS_ENABLED` | core + frontend | `true` | no | `true` = blogs are also served on `<slug>.<domain>` (platform behavior). `false` = path addresses only (`/<slug>`): the frontend drops the wildcard CSP origin and the subdomain rewrite, and shows path addresses. The backend only reports the flag (public config and admin infrastructure block); it does not enforce path-only routing. `.env.example` sets `false`; `k8s/configmap.yaml` sets `true`. | `false` |
| `NOCT_REGISTRATION_MODE` | auth | `open` | no | **Initial** value (first installation) of `platform_config`: `open` · `invite` · `closed`. Change it later in `/admin/config`. | `invite` |
| `NOCT_DEFAULT_LOCALE` | auth | `it` | no | Initial interface language (`it` · `en`). Change it later in `/admin/config`. | `en` |
| `NOCT_DEFAULT_INTERESTS` | interests | list of 16 entries | no | JSON with the initial list of interests. Empty = default. | `[{"key":"music","translations":{"it":"Musica","en":"Music"}}]` |
| `NOCT_EXPERIMENTAL_FEATURES` | core | empty (all off) | no | **Experimental, opt-in.** Comma-separated list of `wiki` and/or `docs`: enables the creation of Spaces of that type and their display mode. When off, the platform offers only blogs (the API answers `400` and the interface does not offer the types). Turning a feature off later does not delete or hide the Spaces already created. An unknown name prevents startup. In Kubernetes, it lives in the ConfigMap `notturni-config`; backend and worker must be restarted. | `wiki,docs` |
| `NOCT_MODULES` | core | default modules | no | Optional modules active at the **first** installation (comma-separated). After that, the state in the database applies: see the [caveat](#existing-database-moved-to-solo). Change it in `/admin/modules`. The solo list in `.env.example` is the default set without `image_moderation`. | `wiki,git_sync,newsletter` |
| `NOCT_BIND_ADDR` | core | `0.0.0.0:8000` | no | API listen address. | `127.0.0.1:8000` |
| `NOCT_TRUSTED_PROXIES` | core | `1` | no | Number of trusted proxies in front of the API (real IP in rate limiting and audit). | `2` |
| `NOCT_APP_ENV` | k8s | — | no | Environment label present in the ConfigMap (`production`). The code does not appear to read it. | `production` |

## First Super Admin

| Variable | Scope | Default | Required | Description | Example |
|---|---|---|---|---|---|
| `NOCT_SUPER_ADMIN_USERNAME` | auth | — | no | Created at startup if it does not exist (idempotent). If one of the three variables is missing, the creation is disabled. | `superadmin` |
| `NOCT_SUPER_ADMIN_EMAIL` | auth | — | no | Email of the Super Admin (login is by email). | `admin@example.com` |
| `NOCT_SUPER_ADMIN_PASSWORD` | auth | — | no | **Secret.** The backend rejects the placeholder of `.env.example`. | `<long random password>` |

## Secrets and sessions

| Variable | Scope | Default | Required | Description | Example |
|---|---|---|---|---|---|
| `NOCT_JWT_SECRET` | core | — | **yes** | **Secret.** Signs JWTs, derived OTPs and signed links: at least 32 random bytes. The backend rejects short values or the placeholder. | `openssl rand -base64 48` |
| `NOCT_JWT_ACCESS_TOKEN_TTL_MINUTES` | core | `15` | no | Lifetime of the access token. | `15` |
| `NOCT_JWT_REFRESH_TOKEN_TTL_DAYS` | auth | `30` | no | Lifetime of the refresh token (session cookie). | `30` |
| `NOCT_SECRETS_KEY` | auth_mfa | derived from `NOCT_JWT_SECRET` | recommended | **Secret.** Dedicated key that encrypts at rest the TOTP secrets **and the Git sync credentials (HTTPS tokens and SSH deploy keys)**. If empty, it is derived from the JWT secret: rotating the JWT secret would make the already stored values unreadable. Set it in production. | `openssl rand -base64 32` |
| `NOCT_MFA_DISABLE_REQUIRES_CODE` | auth_mfa | `true` | no | Disabling MFA requires a valid code. `false` = the v1 behavior. | `true` |
| `NOCT_SESSION_COOKIE_SECURE` | auth | `true` | no | `Secure` attribute of the refresh cookie. Use `false` only in development over http. | `false` |
| `NOCT_SESSION_COOKIE_SAMESITE` | auth | `lax` | no | `SameSite` attribute of the cookie. | `lax` |
| `NOCT_SESSION_COOKIE_DOMAIN` | auth | — | no | Set only if the frontend and the backend share a parent domain. | `.notturni.example.com` |

## Browser: CORS and frontend

| Variable | Scope | Default | Required | Description | Example |
|---|---|---|---|---|---|
| `NOCT_CORS_ORIGINS` | core | `http://localhost:3000` | no | Allowed origins (comma-separated) for browser calls. Must include every origin the frontend is served from. An origin that is missing neither receives nor sends the cookie. | `https://notturni.example.com` |
| `NOCT_CORS_ORIGIN_REGEX` | core | — | no | Additional wildcard for the per-blog subdomains. The match is on the whole origin. | `https://([a-z0-9-]+\.)?notturni\.example\.com` |
| `NOCT_FRONTEND_REVALIDATE_URL` | core | — | no | On-demand invalidation of the SSR cache after a change to public content. Empty = time-based cache only. | `http://frontend:3000/api/revalidate` |
| `NOCT_REVALIDATE_SECRET` | core + frontend | — | no | **Secret.** Must match between backend and frontend. | `openssl rand -base64 32` |
| `NOCT_BACKEND_INTERNAL_URL` | frontend | `NEXT_PUBLIC_API_URL`, then `http://localhost:8000` | recommended | Internal backend URL used by server-side rendering (avoids leaving the cluster network). | `http://backend:8000` |
| `NOCT_SITE_NAME` | frontend, auth | `Notturni` (frontend); unset (auth) | no | Frontend: platform name in the header, login, dashboard, titles, feed, manifest and interface texts. The name set in `/admin/seo` takes precedence. Read at runtime. Auth (backend, `module.toml` option `site_name`): in `solo` it is the title of the primary blog, and its slug is derived from it. If unset or blank, the owner's username is used. The backend does not read `/admin/seo`, so the primary blog keeps the name it got at creation. See [Solo personal site](#solo-personal-site). | `Blue Inks` |
| `NOCT_SITE_URL` | frontend | `https://$NOCT_INSTANCE_FQDN` if `NOCT_APP_ENV=production`, then `NEXT_PUBLIC_SITE_URL`, then `http://localhost:3000` | no | Public site URL (canonical, sitemap, blog subdomains, CSP). Read at runtime. | `https://notturni.example.com` |
| `NOCT_API_PUBLIC_URL` | frontend | `NEXT_PUBLIC_API_URL`, then the site URL | no | API URL used by the browser. Empty = same origin as the site (the Ingress routes `/api` to the backend). Read at runtime. | `https://api.example.com` |
| `NEXT_PUBLIC_API_URL` / `NEXT_PUBLIC_SITE_URL` | frontend | empty | no | Local development fallback only (build args, embedded in the bundle; `compose.yaml` sets them). In production, use the `NOCT_*` variables above: no rebuild is needed to change the domain. | `http://localhost:8000` |

## Database

| Variable | Scope | Default | Required | Description | Example |
|---|---|---|---|---|---|
| `NOCT_DATABASE_URL` | core | — | yes (or the `NOCT_POSTGRES_*` variables) | Single URL. Takes precedence over the separate variables. | `postgres://utente:password@postgres:5432/notturni` |
| `NOCT_POSTGRES_USER` | core | — | yes (if the URL is missing) | User. | `admin` |
| `NOCT_POSTGRES_PASSWORD` | core | empty | no | **Secret.** | `<password>` |
| `NOCT_POSTGRES_DB` | core | — | yes (if the URL is missing) | Database name. | `notturni` |
| `NOCT_POSTGRES_HOST` | core | `localhost` | no | Host. | `postgres` |
| `NOCT_POSTGRES_PORT` | core | `5432` | no | Port. | `5432` |
| `NOCT_AUTO_MIGRATE` | api / worker | `true` | no | Applies the migrations of the active modules at startup (advisory lock, safe with several replicas). `false` = they are applied elsewhere (`noct-ctl migrate apply`). On the worker it is `false` in the manifests. | `false` |

## Message bus and cache

| Variable | Scope | Default | Required | Description | Example |
|---|---|---|---|---|---|
| `NOCT_NATS_URL` | core | — (in-memory bus) | no | NATS JetStream for events and jobs. Empty = in memory (development only, one process). With NATS, the consumers run in `noct-worker`. | `nats://nats:4222` |
| `NOCT_NATS_PREFIX` | core | `noct` | no | Prefix for streams and subjects. | `noct` |
| `NOCT_WORKER_INLINE` | core | `true` without NATS, otherwise `false` | no | Runs the consumers inside the API process (needed with the in-memory bus: see [limits](#no-nats-memorybus-limits)). | `false` |
| `NOCT_REDIS_URL` | core | — | no | Single Redis URL (rate limiting, cache, locks). Takes precedence over host and port. | `redis://redis:6379/0` |
| `NOCT_REDIS_HOST` | core | — (in-memory cache) | no | Redis host. Empty = in memory. | `redis` |
| `NOCT_REDIS_PORT` | core | `6379` | no | Port. | `6379` |
| `NOCT_REDIS_DB` | core | `0` | no | Database number. | `0` |

## Object storage

| Variable | Scope | Default | Required | Description | Example |
|---|---|---|---|---|---|
| `NOCT_STORAGE_BACKEND` | core | `localstorage` | no | `s3` (Garage, AWS S3, R2…) or `localstorage` (filesystem, served by the backend on `/storage`, for `solo` installations). | `s3` |
| `NOCT_S3_ENDPOINT_URL` | core | — | with `s3` | S3 API endpoint. Never hard-coded in the code. | `http://garage:3900` |
| `NOCT_S3_PUBLIC_URL` | core | — | with `s3` | Public URL of the objects (avatars, media), served by the `s3-public` proxy. On real AWS S3, it does not match the endpoint. | `https://media.example.com` |
| `NOCT_S3_REGION` | core | — | no | Garage validates the region: it must match `s3_region` in `garage.toml`. | `garage` |
| `NOCT_S3_ACCESS_KEY_ID` | core | — (AWS role) | no | With Garage: `GK` + 24 hex characters. If both this and the secret key are empty, the SDK default credential chain applies. | `GK0123456789abcdef01234567` |
| `NOCT_S3_SECRET_ACCESS_KEY` | core | — | no | **Secret.** 64 hex characters with Garage. | `openssl rand -hex 32` |
| `NOCT_S3_BUCKET_AVATARS` | core | `avatars` | no | Avatar bucket (public). | `avatars` |
| `NOCT_S3_BUCKET_CONTENT` | core | `notturni-content` | no | Media embedded in posts, and copy of the posts' Markdown. | `notturni-content` |
| `NOCT_S3_BUCKET_AUDIT` | core | `notturni-audit` | no | Audit log archives (private). | `notturni-audit` |
| `NOCT_S3_PUBLIC_ACCESS` | compose | `website` | no | Public exposure mode of the Garage buckets (`compose.yaml` only). | `website` |
| `NOCT_LOCAL_STORAGE_BASE_PATH` | core | `./data/storage` | no | Local storage directory (in the image: `/data/storage`). | `/data/storage` |
| `NOCT_LOCAL_STORAGE_PUBLIC_URL` | core | `http://localhost:8000/storage` | no | Public URL of the local storage, as the **browser** reaches it. Behind an Ingress it must be a real URL that routes `/storage` to the backend, for example `https://example.com/storage`; the default only works on a developer machine. The Helm chart derives it from `ingress.host`. The frontend reads it too, to allow the origin in the CSP `img-src`. Only the `avatars` and `content` buckets are served; the audit archive never is. Responses carry `X-Content-Type-Options: nosniff` and `Cache-Control: public, max-age=3600`. In the local compose stack the default URL points at the backend port (8000), not at the frontend (3000). | `https://example.com/storage` |
| `NOCT_GARAGE_RPC_SECRET` | garage | — | yes (with Garage) | **Secret.** 64 hex characters, daemon RPC. | `openssl rand -hex 32` |
| `NOCT_GARAGE_ADMIN_TOKEN` | garage | — | yes (with Garage) | **Secret.** Admin API token (port 3903, not published). | `openssl rand -base64 32` |
| `NOCT_GARAGE_ADMIN_URL` | `noct-ctl` | `http://garage:3903` | no | Admin endpoint used by `garage-bootstrap`. | `http://garage:3903` |
| `NOCT_GARAGE_NODE_CAPACITY_BYTES` | `noct-ctl` | `1073741824` | no | Capacity assigned to the Garage node at bootstrap. | `10737418240` |
| `NOCT_GARAGE_WEB_UPSTREAM` | s3-public | `garage:3902` | no | Garage web upstream for the `s3-public` proxy. | `garage:3902` |

## Backup and audit

| Variable | Scope | Default | Required | Description | Example |
|---|---|---|---|---|---|
| `NOCT_BACKUP_S3_BUCKET` | backup | — (backup disabled) | no | Bucket of the **external** storage. Without it, only the Markdown copy of the posts remains. | `notturni-backup` |
| `NOCT_BACKUP_S3_ENDPOINT_URL` | backup | — | no | External S3 endpoint. | `https://s3.example.com` |
| `NOCT_BACKUP_S3_REGION` | backup | — | no | Region. | `eu-central-1` |
| `NOCT_BACKUP_S3_ACCESS_KEY_ID` | backup | — | no | Access key. | `<key id>` |
| `NOCT_BACKUP_S3_SECRET_ACCESS_KEY` | backup | — | no | **Secret.** | `<secret>` |
| `NOCT_AUDIT_ARCHIVE_ENABLED` | audit | `true` | no | Archives the completed ISO weeks of the log to storage before deleting them (the retention in days is set in `/admin/config`). | `true` |

## Email, moderation, captcha

| Variable | Scope | Default | Required | Description | Example |
|---|---|---|---|---|---|
| `NOCT_SMTP_HOST` | core | — (emails not sent) | no | SMTP server for MFA, password reset and newsletters. Empty = no emails are sent: no MFA or reset codes, no newsletter; password recovery then uses `noct-ctl` (see [below](#password-recovery-without-smtp)). | `smtp.example.com` |
| `NOCT_SMTP_PORT` | core | `587` | no | SMTP port. | `587` |
| `NOCT_SMTP_USER` | core | — | no | SMTP user. | `no-reply@example.com` |
| `NOCT_SMTP_PASSWORD` | core | — | no | **Secret.** | `<password>` |
| `NOCT_SMTP_USE_TLS` | core | `true` | no | TLS to the SMTP server. | `true` |
| `NOCT_SMTP_FROM_EMAIL` | core | `no-reply@notturni.eu` | no | Sender address. | `no-reply@example.com` |
| `NOCT_SMTP_RELAY_HOST` | smtp-relay | — (direct sending) | no | Smarthost of the containerized `smtp-relay` service. Empty = direct sending (requires SPF/DKIM/rDNS). | `smtp.example.com` |
| `NOCT_SMTP_RELAY_USERNAME` | smtp-relay | — | no | Smarthost credentials (user). | `relay-user` |
| `NOCT_SMTP_RELAY_PASSWORD` | smtp-relay | — | no | **Secret.** | `<password>` |
| `NOCT_MODERATION_SERVICE_URL` | core | — (no classification) | no | Image moderation service. Empty, or the `image_moderation` module off, means uploads are never classified: see [Image moderation off](#image-moderation-off). | `http://moderation:8100` |
| `NOCT_TURNSTILE_SITE_KEY` | core | — | no | Public site key of Cloudflare Turnstile (read by the frontend from `GET /api/v1/config`). | `0x4AAAA…` |
| `NOCT_TURNSTILE_SECRET_KEY` | comments | — | no | **Secret.** Without keys, the mode «commenti aperti a tutti» (comments open to everyone) cannot be selected. | `0x4AAAA…` |
| `NOCT_TURNSTILE_VERIFY_URL` | comments | Cloudflare endpoint | no | Changes only in tests. | — |

## External identity providers (auth_sso)

| Variable | Scope | Default | Required | Description | Example |
|---|---|---|---|---|---|
| `NOCT_OAUTH_REDIRECT_BASE_URL` | auth_sso | `http://localhost:8000` | no | Public origin of the backend. Base of the `redirect_uri` registered with the providers. | `https://api.example.com` |
| `NOCT_FRONTEND_URL` | auth_sso | `http://localhost:3000` | no | Frontend origin, where the user returns after login. | `https://notturni.example.com` |
| `NOCT_OAUTH_GOOGLE_CLIENT_ID / _CLIENT_SECRET` | auth_sso | empty | no | Enables Google when set (the secret is **secret**). | `<client id>` |
| `NOCT_OAUTH_MICROSOFT_CLIENT_ID / _CLIENT_SECRET` | auth_sso | empty | no | Enables Microsoft. | `<client id>` |
| `NOCT_OAUTH_GITHUB_CLIENT_ID / _CLIENT_SECRET` | auth_sso | empty | no | Enables GitHub. | `<client id>` |
| `NOCT_OAUTH_LINKEDIN_CLIENT_ID / _CLIENT_SECRET` | auth_sso | empty | no | Enables LinkedIn. | `<client id>` |
| `NOCT_OAUTH_ENDPOINT_BASE` | auth_sso | empty | no | Tests and local trials only: each provider's endpoints become `<base>/<provider>/…`. | — |

## Other modules

| Variable | Scope | Default | Required | Description | Example |
|---|---|---|---|---|---|
| `NOCT_DNS_SERVER` | custom_domains | — (system resolver) | no | DNS server for the TXT verification of personal domains. | `1.1.1.1:53` |
| `NOCT_DNS_TIMEOUT_MS` | custom_domains | `5000` | no | DNS verification timeout. | `5000` |
| `NOCT_LINK_PREVIEW_ALLOW_PRIVATE` | link_preview | `false` | no | Allows previews to private networks. **Disables the SSRF protection**: development only. | `false` |
| `NOCT_GIT_SYNC_ALLOW_PRIVATE` | git_sync | `false` | no | Allows repositories on private hosts (HTTPS and SSH) and `file://`. **Disables the SSRF protection**: development and tests only. | `false` |
| `NOCT_GIT_SYNC_GIT_BIN` | git_sync | `git` | no | `git` executable (present in the image). | `/usr/bin/git` |
| `NOCT_GIT_SYNC_SSH_BIN` | git_sync | `ssh` | no | SSH client used by `git` (package `openssh-client`, present in the image). Change it only for non-standard paths and in tests. | `/usr/bin/ssh` |
| `NOCT_GIT_SYNC_KEYSCAN_BIN` | git_sync | `ssh-keyscan` | no | Scanner that detects the SSH host key during configuration, to pin it. Change it only for non-standard paths and in tests. | `/usr/bin/ssh-keyscan` |

## Deployment profiles: solo and platform

One code base, two ways to run it. Nothing is removed from the code: the profile only decides which components run and which defaults apply.

| | `solo` (default of `.env.example` and the Helm chart) | `platform` (reference: `k8s/`, `compose.platform.yaml`) |
|---|---|---|
| Who | One owner, plus collaborators created by the Super Admin | Multi-user, open or invite-only registration |
| Services | PostgreSQL, backend (inline worker), frontend | Adds Redis, NATS JetStream and a separate worker, Garage with `s3-public`, image moderation, optional SMTP relay |
| Object storage | `localstorage`: a volume mounted in the backend, served on `/storage` | S3-compatible (Garage, AWS S3, R2) |
| Cache and rate limits | In memory, per process | Redis |
| Message bus | In memory (`MemoryBus`) | NATS JetStream, at-least-once |
| Blog addresses | `/<slug>` only (`NOCT_SUBDOMAINS_ENABLED=false`); the primary blog is the home `/` and its posts are `/post/<slug>` (see [Solo personal site](#solo-personal-site)) | `<slug>.<domain>` and paths |
| Verification seal and role labels | Hidden in the UI (the API still returns `verification_tier` and roles; permissions are unchanged) | Shown |
| Email | An existing SMTP server, optionally through the `smtp-relay` container; none if unset | Same |
| Image moderation | Off; sensitive images are marked by hand | On |
| Scaling | One backend replica (RWO volume, in-process bus) | Several replicas |

### Switching a component on

| Component | Env (`.env`, `k8s/configmap.yaml`) | Compose | Helm values |
|---|---|---|---|
| Redis | `NOCT_REDIS_URL` or `NOCT_REDIS_HOST` | `--profile platform` | `redis.enabled=true` |
| NATS and separate worker | `NOCT_NATS_URL`, `NOCT_WORKER_INLINE=false` on the API | `--profile platform` | `nats.enabled=true` (deploys the `worker`) |
| S3 storage | `NOCT_STORAGE_BACKEND=s3`, `NOCT_S3_*` | `--profile platform` (Garage) | `storage.type=s3`, `storage.s3.*` or `storage.s3.garage.enabled=true` |
| Subdomains | `NOCT_SUBDOMAINS_ENABLED=true`, `NOCT_CORS_ORIGIN_REGEX`, wildcard DNS and certificate | `.env` only | `subdomains.enabled=true` (needs `mode=platform`) |
| Image moderation | `NOCT_MODERATION_SERVICE_URL` and `image_moderation` in `NOCT_MODULES` | `--profile platform` | `moderation.enabled=true` |
| SMTP | `NOCT_SMTP_HOST` and related | `.env`; `--profile mail-relay` for `smtp-relay`; `--profile dev-mail` for MailHog | `smtp.host`, or `smtp.relay.enabled=true` |

With compose, `--profile platform` alone starts the services but does not wire the backend to them: use `podman compose -f compose.yaml -f compose.platform.yaml --profile platform up -d --build` (see [INSTALL.md](INSTALL.md#2-podman-and-docker-compose)). The Helm chart refuses inconsistent combinations at render time (for example more than one backend replica with local storage or without NATS).

### Existing database moved to solo

`NOCT_MODULES` applies only at the first installation. After that, the module table in the database wins. If you point a solo install at a database that already ran as `platform`, `image_moderation` stays active until you turn it off in `/admin/modules`, whatever `NOCT_MODULES` says.

### Solo personal site

In `solo` the instance is a personal site with one primary blog. The behaviour is set by the code, with no extra variable, except `NOCT_SITE_NAME`:

- **Primary blog.** Created at the first registration and at backend startup (idempotent), for the first user or the oldest active Super Admin. Its title is `NOCT_SITE_NAME`, or the owner's username if unset. Its slug is derived from the title (`-2`, `-3`, ... on a collision). The site root `/` shows it, and its posts are at `/post/<slug>`. `GET /api/v1/config` returns its slug as `primary_blog_slug`. If the blog was removed by the purge of deleted blogs, the next backend restart creates a new one.
- **Spaces.** Other blogs are reached by path only (`/<slug>/`). Subdomains are off (`NOCT_SUBDOMAINS_ENABLED=false`, the `.env.example` default).
- **Off in solo:** the reserved-names list (`reserved_blog_names`), the minimum slug length, the user interests (selector, filter and profile values; stored values stay in the database), and the editing of site texts (`PUT /api/v1/admin/site-texts/{locale}` returns `403`; the admin page is hidden). Footer texts stay editable in the admin settings page (`/admin/impostazioni`).

The full list of routes and rules is in [API.md](API.md#solo-mode-primary-blog-and-personal-site-routes).

### No NATS: MemoryBus limits

In solo mode the message bus is in-memory. Jobs still waiting in the outbox survive a restart and are processed on the next start. A job already handed to the in-memory bus but not yet consumed is lost if the process stops, and is not redelivered. Such jobs are best-effort (emails, notifications); run NATS for at-least-once delivery. The window is small, and lost jobs are not retried. Behavior is covered by `bins/noct-api/tests/solo_membus.rs`. The inline worker and the in-process bus also mean the backend must run as a single replica.

### Image moderation off

With `image_moderation` inactive (the solo default), uploads are not classified and `is_sensitive` stays `false`. Authors mark images by hand: category chips in the media library, the content-warning dialog on the cover image (`cover_image_is_sensitive`, honored only in the same PATCH that sets a new `cover_image_url`) and on inline images (the `sensitive:<category>` image title). The public pages blur an image when it is flagged or has categories.

Operator responsibility: automatic filtering is absent. Under GDPR and NIS2 the operator decides whether that is acceptable for the content hosted, and should state in the terms of use that authors must mark sensitive images. The `moderation_threshold` setting is hidden in the admin panel while the module is off. Post and comment moderation (reports, hide/show) are unaffected.

### Collaborators in solo mode

Registration stays closed after the first user, and invitations need an existing account. The Super Admin therefore creates the account, then invites it by username.

1. Create the account, active at once (no email verification exists):
   - Admin form: `/admin/utenti/nuovo` (nav entry "Collaboratori" in solo), or `POST /api/v1/admin/users` (Super Admin only, `201`, rate limit 20 per 5 minutes). The password is optional; when omitted the server generates one and returns it **once** in `generated_password`. The form keeps it in component state only. The response carries no `Cache-Control: no-store`.
   - Shell: `printf '%s\n' "$PW" | noct-ctl user create --email bob@example.com --username bob`. It prints the id. With an empty stdin it generates a password and prints `generated_password=...` once.
   - Hand the password over through a secure channel. There is no forced change at first login.
2. Invite the user by username from the blog's collaborators tab, as `co_autore` or `mediatore`. The user accepts from the dashboard.
3. SSO does not bypass the restriction: it never auto-provisions in solo. The only other account sources are the `NOCT_SUPER_ADMIN_*` bootstrap and the paths above.

| Role | Can | Cannot |
|---|---|---|
| `co_autore` | Create, edit, submit, publish, unpublish and delete posts; media, pages, categories, notes, publications, newsletter, wiki outline | Change blog settings, manage members or invitations (owner only); moderate comments |
| `mediatore` | Moderate comments (list pending, approve, reject, block authors, blocked list); see members-only blogs | Write to the blog |

The table comes from reading `authz.rs` and the call sites of `can_write_blog`; only the invite and accept flow was run end to end. Roles are not displayed in the solo UI (the invitation sentence in the dashboard still names the role).

### Password recovery without SMTP

Without `NOCT_SMTP_HOST` no reset email leaves the server. The recovery path needs shell access to the deployment:

```bash
printf '%s\n' "$NEW_PASSWORD" | podman compose exec -T backend noct-ctl user reset-password --email admin@example.com
printf '%s\n' "$NEW_PASSWORD" | kubectl -n notturni exec -i deploy/notturni-backend -- noct-ctl user reset-password --email admin@example.com
```

The command sets the password, deletes every session, bumps the credentials timestamp and writes the audit event `admin.user_password_reset` (actor `system`, no password in the payload). Unknown or inactive users get a generic error; SSO-only accounts are refused. Caveat: an access token issued in the same second as the reset stays valid until it expires (default 15 minutes, `NOCT_JWT_ACCESS_TOKEN_TTL_MINUTES`), because the comparison is in seconds.

### Admin wizard and infrastructure panel

- `/admin/impostazioni` shows a read-only infrastructure block (mode, subdomains, modules, bus `memory|nats`, cache `memory|redis`, inline worker, local storage public URL). The GUI never changes infrastructure.
- `/admin/setup` (a banner for the Super Admin in solo points to it) is a first-run wizard. It asks for mode, storage, cache, SMTP, subdomains, moderation and NATS, and only **prints** a `.env` snippet and a Helm `values.yaml`, with placeholders instead of secrets. Nothing is applied. The dismissal of the banner is stored per browser.

## Hot configuration (not environment)

Some values come from the environment only at the first installation. After that, they live in the database and can be changed without a restart:
registration mode, default language, audit retention (`/admin/config`), and active modules (`/admin/modules`,
Super Admin only). Changing the variable after the first installation has **no** effect on these.

## Minimal examples

**Solo, lean** (the defaults of `.env.example`): `NOCT_DEPLOYMENT_MODE=solo`, `NOCT_SUBDOMAINS_ENABLED=false`, `NOCT_STORAGE_BACKEND=localstorage`, `NOCT_LOCAL_STORAGE_PUBLIC_URL=https://example.com/storage`, no Redis, NATS or moderation variables, `NOCT_MODULES` without `image_moderation`.

**Local development, all in memory** (no external services):

```dotenv
NOCT_POSTGRES_USER=admin
NOCT_POSTGRES_PASSWORD=foo
NOCT_POSTGRES_DB=notturni
NOCT_JWT_SECRET=<openssl rand -base64 48>
NOCT_STORAGE_BACKEND=localstorage
NOCT_SESSION_COOKIE_SECURE=false
```

**Production (K3s)**: in addition to the secrets, set at least `NOCT_INSTANCE_FQDN`, `NOCT_CORS_ORIGINS`/`NOCT_CORS_ORIGIN_REGEX`,
`NOCT_SECRETS_KEY`, `NOCT_NATS_URL`, `NOCT_REDIS_URL`, the `NOCT_S3_*` variables, SMTP, and `NOCT_BACKUP_S3_*` pointing to a
truly external provider.

## Security and GDPR/NIS2

- `NOCT_SECRETS_KEY`: keep it outside the cluster. It is the key for the TOTP secrets and the Git credentials (tokens and SSH keys). Losing it makes those values unreadable.
- `NOCT_EXPERIMENTAL_FEATURES`: wiki and docs do not yet have the maturity of the rest of the platform. Enabling them is a deliberate choice of whoever installs the platform.
- `NOCT_LINK_PREVIEW_ALLOW_PRIVATE` and `NOCT_GIT_SYNC_ALLOW_PRIVATE` are development switches: in production they must stay `false`.
- Solo without SMTP: no email verification or reset codes exist; recovery is `noct-ctl user reset-password`, which is audited. Initial passwords of collaborators pass through the admin or the shell: treat them as secrets.
- Solo without moderation: sensitive images are not filtered automatically (see [Image moderation off](#image-moderation-off)). Local storage and the database volume hold personal data: back them up and record the retention in the register of processing activities.
- `NOCT_BACKUP_S3_*`: the backup contains personal data. The external provider must be recorded in the register of processing activities (preferably in the EU).
- The refresh token travels in a cookie. Over https, keep `NOCT_SESSION_COOKIE_SECURE=true`.
