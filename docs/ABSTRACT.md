# Notturni - product abstract

## Goal and idea

Notturni is an open source, multi-user platform for microblogging and newsletters. It targets
multiple languages and stresses GDPR compliance. Its design follows an EU-centric view.

The core idea: people express themselves in the most natural form, which is words. The platform
gives them the clarity and richness of their own language. It does not use artificial
intelligence. It is a keyboard that separates thoughts from a clean, friendly, and welcoming
interface.

Reference points for the product: medium.com, fika.bar (editor feel), and tumblr.com.

## Product rules

- **Blogs per user:** a user can own up to 5 blogs or publications. The limit is a platform
  setting (`max_blogs_per_user`).
- **Blog names:** at least 4 alphanumeric characters. A hyphen (`-`) is allowed. Names of 3
  characters or fewer are reserved for the platform, for a future premium feature.
- **Reserved names:** a blacklist of words (for example `blog`, `www`, `mail`, `journal`, `api`,
  `admin`, `monitor`, `stats`, `status`) cannot be used as blog names.
- **Blog addresses:** each blog is served at `https://<name>.notturni.eu`. The public
  platform site at `https://notturni.eu` aggregates articles by language, similar to dev.to.
  The platform blog for news and updates is at `https://blog.notturni.eu`.
- **Pen name:** the author name shown on a blog or article can differ from the real name of the
  user. This protects privacy.
- **Platform roles:** four roles: Super Admin, Admin, Moderator, and User. User is the default.
  The first Super Admin is created at deployment from `NOCT_SUPER_ADMIN_*` settings.
- **Blog roles:** roles are per blog, not per platform: Author (default for a User), Co-Author,
  Reviewer, and Mediator.
- **Comments:** only registered users can comment by default. A blog owner can also allow
  anonymous comments. Anonymous comments always go through moderation before publication.
- **Dashboards:** the user dashboard is modern, responsive, and usable on desktop and mobile.
  The admin dashboard covers the features that a platform of this kind needs for
  administration and moderation.
- **Editor:** clean and focused on text and visual content.
- **Customization:** users can change colors, typography, and content layout. Changes must stay
  within the aesthetic constraints below.

## Aesthetics

- Clean design. Colors are soft and calm, and they suggest friendliness.
- Elegant, modern typography: serif for titles, sans-serif for body text and links.
- A palette of at most 5 colors.
- At most 3 fonts.

## Architecture and stack

The backend is written in Rust. The product started as a Python prototype (v1). The Rust
rewrite is the current implementation. See [docs/BACKEND.md](BACKEND.md) for the history.

| Component | Technology | Role |
| --- | --- | --- |
| Backend API | Rust, axum, sqlx (Cargo workspace) | HTTP API (`noct-api`), job and event worker (`noct-worker`), and CLI (`noct-ctl`) |
| Modules | Declarative `module.toml`, `api.toml`, `db.toml` | Optional features (comments, newsletter, MFA, SSO, GDPR, and others). Modules are enabled and disabled at runtime from the admin API, without a restart |
| Database | PostgreSQL 16 or later | Primary data store. Each entity uses a UUID primary key |
| Cache and rate limiting | Redis 7 or later | Cache and rate limiting. Distributed locks are not in use yet |
| Messaging | NATS JetStream | Events and jobs. A transactional outbox in the database feeds the relay in the worker |
| Object storage | Garage (S3-compatible), or AWS S3 and other S3 endpoints | Media and post backups. The S3 endpoint is set by configuration, never fixed in code |
| Local filesystem storage | `NOCT_STORAGE_BACKEND=localstorage` | Alternative storage for single-owner (`solo`) installs without S3 |
| Frontend | Next.js (App Router, server-side rendering), Tailwind CSS | Public site, user dashboard, and admin dashboard |
| Image moderation | Separate self-hosted service | Checks each image upload for sensitive content. It fails open: a service error never blocks an upload |
| Mail | SMTP | Transactional email and newsletters |
| Ingress | Traefik | Routes traffic in Kubernetes |
| TLS | cert-manager with Let's Encrypt | Certificates in production |
| Local development | Podman (rootless), `compose.yaml` | Full local stack |
| Production | Kubernetes (K3s on a single node first) | Deployment. Storage uses `local-path` today. Longhorn is the target for a multi-node cluster |

Architecture constraints:

- Development runs on x86_64. Production targets ARM64. Dockerfiles and manifests must not
  depend on one architecture.
- Every S3 client takes its endpoint from configuration.
- Environment variables use the `NOCT_` prefix.
- `NOCT_DEPLOYMENT_MODE` selects `platform` (multi-user, default) or `solo` (one owner; the
  first registered user becomes Super Admin, and registration then closes). In `solo` the site is a
  personal site: its primary blog is the home page `/`, its posts are at `/post/<slug>`, its sub-pages are at the
  root (`/pagina/...`, `/pub/...`, `/bibliografia`, `/media`, `/link`, `/search`), and other spaces are
  reached by path only. Interests are off. See [API.md](API.md#solo-mode-primary-blog-and-personal-site-routes).

## Authentication

- **Passwords:** hashed with Argon2. Access tokens are JWTs, and refresh tokens are rotated.
- **Multi-factor authentication (MFA):** TOTP (authenticator app) or email one-time codes. The
  email code is sent asynchronously through the worker.
- **Single sign-on (SSO):** Google, Microsoft, GitHub, and LinkedIn, using OAuth2 and OpenID
  Connect.
- **Account linking:** an SSO login whose email matches an existing account requires the second
  factor, if the account has one, before the new provider is linked.
- **API tokens:** opaque values with the prefix `noct_`. The database stores only the SHA-256
  hash. The `owner_type` field separates core-engine tokens (machine-to-machine, no user) from
  user tokens. User tokens are planned for direct API access without the editor or admin
  interface.

## Deployment targets

- **Local development:** Podman and `compose.yaml`.
- **Production:** Kubernetes manifests in `k8s/`, targeting K3s on a single node first, with
  multi-node expansion later.
- **Backups:** the platform copies its important data (PostgreSQL dumps and buckets) to an
  external S3-compatible store.

## Future developments

Planned, not implemented in this phase:

- Federation with other instances and clones. AT Protocol (Bluesky) comes first, then
  ActivityPub (Mastodon). The two stacks are incompatible and need separate implementations.
- Counters for likes, quotes, and shares.
- Custom domains for a blog.

## Related docs

- [README.md](../README.md): overview, repository layout, and local start.
- [GETTING_STARTED.md](../GETTING_STARTED.md): first local run and first tests.
- [ROADMAP.md](../ROADMAP.md): status of each product item and the backlog.
- [docs/BACKEND.md](BACKEND.md): backend structure, development, and operations.
- [docs/API.md](API.md): HTTP API reference.
- [docs/INSTALL.md](INSTALL.md): installation guides.
