# Notturni

Open-source, multilingual microblogging platform, EU-centric and GDPR compliant.

This file describes the repository layout and how to start the development
environment. For a step-by-step guide to the first local start and a first
test run, see [GETTING_STARTED.md](GETTING_STARTED.md). For full installations
(Podman/Docker, K3s, Debian/Ubuntu and Rocky/Alma VMs), see
[docs/INSTALL.md](docs/INSTALL.md). For the full list of product specifications
and their status, see [ROADMAP.md](ROADMAP.md). For the product abstract, see
[docs/ABSTRACT.md](docs/ABSTRACT.md). For the change history, see
[CHANGELOG.md](CHANGELOG.md).

## Stack

- **Frontend:** Next.js (App Router), Tailwind CSS
- **Backend:** Rust (Cargo workspace, `unsafe` forbidden), axum, sqlx. Declarative modules
  (`module.toml`/`api.toml`/`db.toml`) from which DTOs, routes and migrations are generated; core and
  optional modules can be enabled at runtime (hot). See [docs/BACKEND.md](docs/BACKEND.md)
- **Authentication:** password (Argon2) + JWT sessions, MFA (TOTP encrypted at rest / email), SSO
  OAuth2/OIDC, API tokens for direct access / the core engine
- **Database:** PostgreSQL 16+
- **Cache & State:** Redis 7+ (rate limiting, locks)
- **Message Broker:** NATS JetStream (events and jobs, transactional outbox in the database)
- **Object Storage:** S3-compatible backend (AWS SDK for Rust) (Garage locally and in production, the
  project's primary choice; configurable against any S3-compatible endpoint or AWS S3 in production, with
  auth via access key or AWS role), or local filesystem (`NOCT_STORAGE_BACKEND=localstorage`, an
  alternative for `solo` installations without S3 storage)
- **Local development:** Podman (rootless, without Kubernetes)
- **Production:** Kubernetes (K3s), Traefik (ingress), Longhorn (storage), cert-manager

## Implemented features

- **Complete auth:** registration and login with password (minimum policy: 10
  characters), forgotten password (reset via email code, revokes all active
  sessions), JWT sessions (access + refresh with rotation), MFA via TOTP or
  email, SSO (Google/Microsoft/GitHub/LinkedIn, not yet exposed in the UI
  pending an end-to-end test with real credentials) with account linking and
  a 2FA gate.
- **Blogs, posts and comments:** CRUD with the domain rules in ROADMAP §1 (slug,
  limit of 5 blogs per user, comment moderation). Posts in Markdown, with a
  complete status workflow (draft → under review → published, with scheduled
  publication) and the Reviewer role (*Revisore*, reviewer) tied to a real
  capability. Each blog has a subtitle, a short description and a **visibility**
  setting: public (*pubblica*), platform members only (*solo iscritti alla
  piattaforma*), or private as a diary for the owner only (*privata*).
- **Blog collaborators:** the owner invites other users as co-author (*co-autore*)
  or mediator (*mediatore*). An invitation becomes a membership only after the
  invitee accepts it from their own dashboard. Each collaborator can choose an
  alias to sign posts on that blog.
- **WYSIWYG editor (Tiptap):** headings, bold/italic/strikethrough/code/link,
  quotes, lists, tables, images with ALT text, cover image, `@mentions` with
  autocomplete that become links to the mentioned profile (can be disabled per
  blog), footnotes. Borderless, airy experience, inspired by fika.bar. Content
  is stored as Markdown, with no rendering on the backend. Images follow the
  Bluesky style (ALT pill and removal overlay, blurred with click-to-reveal
  already in the editor when flagged as sensitive, and a modal to manually choose
  the warning: *Suggestivo* (suggestive), *Nudità* (nudity), *Esplicito*
  (explicit), *Contenuto sensibile* (sensitive content)). Pasting a link on its
  own also adds a preview card (title, description, image) below the link text.
- **Notes, media and links: automatic blog bibliography.** Each post has a list of
  footnotes, shown numbered at the bottom of the post's public page, with the
  text as a tooltip on the reference. The editor inserts a note from a modal
  (same style as the sensitive-content warning on images). Only the free text is
  required. Behind a toggle, you can add title, author, ISBN, DOI and page for
  structured bibliographies, which are then shown at the bottom of the post and
  in the bibliography. A page at `/{blog}/bibliografia` collects all notes from
  published posts, deduplicated, with the posts that cite them. Likewise,
  `/{blog}/media` and `/{blog}/link` collect the images and links cited in the
  body of posts, with the publication date of each citation.
- **Fragments:** a reader can select a portion of text (max 15%) of a published
  post with the mouse and save it to a unified personal collection
  (`/dashboard/frammenti`, with author, post title, date and a link to the
  original post). A saved fragment is highlighted again in a light cyan on every
  later reading of the post, regardless of how the reader got there.
- **Tags and categories on posts:** free tags (maximum 5, from a dedicated field
  and/or `#hashtag` in the text) and per-blog categories defined in advance by
  authors (at most one per post). Both are also used to filter the homepage feed.
- **Automatic image moderation:** a separate self-hosted service (`moderation/`,
  an open-source model that detects nudity and sensitive content) is called on
  every upload. No image leaves the infrastructure. Flagged images stay blurred
  until the user clicks them, both in post content and as cover images. "Fail
  open": a problem in the moderation service never blocks an upload.
- **Readable permalinks and aggregated homepage:** every post is reachable even
  without a subdomain, with a WordPress-style permalink (`/{blog}/{slug}`, no UUID
  in the URL). The platform homepage shows the chronological feed of all blogs,
  with a "trending" section (*di tendenza*) based on the most-used tags.
- **Media and S3 backup:** images can be embedded in posts (public), and a backup
  copy of the Markdown of each published post (private) is stored under
  `s3://{bucket}/{site_slug}/userdata/{user}/{blog}/...`. Every day, `pg_dump` and
  a copy of all buckets are also sent to an external S3 storage (`backup` module).
- **Hot-swappable modules:** the optional modules (comments, newsletter, MFA, SSO,
  GDPR, ...) can be enabled and disabled from administration without restarting
  the backend. The tables of a disabled module remain intact.
- **Multilingual (i18n):** posts and static pages can be translated (locale + slug
  per language). A fallback-language selector in the profile replaces typing the
  locale code by hand. The `/it/`, `/en/`, ... path structure is a convention left
  to the public frontend routing (not yet built: today the frontend selects the
  language via a parameter, not via the path).
- **Static pages:** About, Contact, Privacy, etc. of the main site, managed by
  Administrator/Super Admin (always active, permalink `/p/{slug}`). They can also
  be extended to individual blogs as an opt-in feature for the owner (disabled by
  default), with the same editing interface and permalink `/{blog}/pagina/{slug}`.
- **Customizable appearance per blog:** palette/typography/layout in free JSON
  (`blog_configs`), within the aesthetic constraints in ROADMAP §2 (max 5 colors,
  max 3 fonts). Default public name for the blog's authors and co-authors. Cover
  image (banner of the public homepage) and dedicated favicon, both optional.
- **User profile:** extended bio (first name, last name, country, native language
  and fallback languages), editable username (at most one change every 5 days;
  users are referenced everywhere by ID, not by string, so an allowed change is
  reflected immediately in posts, comments and `@mention` autocomplete, verified in
  real time during registration), password change while logged in (old + new
  password, revokes all active sessions, in addition to the existing email
  recovery), email change with double verification (a code to the old mailbox,
  then to the new one), a global public alias as an alternative to the username,
  a choice of what to show as the author name on one's posts (username, first and
  last name, or alias), avatar (upload to Garage/S3), and fixed social links with
  a monochrome icon from an easily editable configuration file. **Profile
  verification:** a custom domain verified through a DNS record (Bluesky style)
  grants a bronze verification badge (*bronzo*; silver, gold and blue are reserved
  for the future). Fediverse IDs (AT Protocol + ActivityPub) are created as
  placeholders and shown in the profile, in preparation for a federation that is
  not yet active.
- **Follow:** users can follow other users or blogs, including anonymously. For a
  blog presented with an alias different from the name of its manager, the real
  identity never appears outside the owner's management pages. In their own
  profile only, a blog owner sees the total follower count, summed across username
  and alias, plus the separate count for each entity.
- **Platform footer:** 3 columns and a bottom bar in free Markdown (images and
  links included), managed by a Super Admin and shown on every public page, both
  platform and blog pages. Columns 1 and 2 are a default that each blog's owner can
  override for their own blog. Column 3 and the bottom bar remain platform-only.
- **Administration:** sections under `/admin`, visible only to Administrator
  (*Amministratore*) and Super Admin, with a search field in each section.
  Sections: Users (*Utenti*; role and activation; only the Super Admin can assign
  administration roles; hidden in `solo` mode), Static pages (*Pagine statiche*,
  same Tiptap editor as posts), All blogs (*Tutti i blog*; platform list with
  owner and visibility; suspending a blog blocks public reading and writing, also
  for the owner), Moderation (*Moderazione*; list of all platform posts; hide or
  show a single post regardless of its status).
- **API tokens:** direct access for the core engine, prepared for future direct use
  by users.
- **Frontend:** author interface (dashboard, editor, profile), light/dark/automatic
  theme (dawn-dusk based on location, computed only on the client side), interface
  in Italian and English (`next-intl`, selector in the header and in the profile;
  the content language is separate), and blog palette, typography and layout
  applied to its public pages.
- **Publications:** series of posts as chapters under `/{blog}/pub/{nome}`, with an
  automatic index, explicit order and navigation between chapters.
- **RSS/Atom:** platform-wide chronological feed (`/feed.xml`, `/atom.xml`) and one
  per blog (`/{blog}/feed.xml`, `/{blog}/atom.xml`), with autodiscovery.
- **Newsletter:** public sign-up with double opt-in via email (per blog or to the
  platform digest), automatic notification on each published post (can be disabled
  per blog), manual campaigns from the dashboard, and unsubscription and data
  deletion without needing to log in.
- **SEO:** canonical URLs, Open Graph/Twitter cards and JSON-LD structured data
  (`BlogPosting`) on all public pages (posts, platform and blog homepages,
  bibliography/media/links, static pages, user profile). `sitemap.xml` and
  `robots.txt` are generated dynamically and follow the per-blog and per-post crawler
  opt-in.
- **Blog management:** overview with aggregated reads (no cookies), media library
  and note library (type, DOI, duplicates, BibTeX), comment queue with blocked
  comments and reports, pause/transfer/ZIP export/deletion with a 30-day grace
  period. Aggregated views in the dashboard sidebar (Posts, Media, Publications,
  Links, Bibliography) across **all** of the user's blogs at once, with each item
  labeled with its source blog.
- **Extended administration:** reader reports with a panel and a mandatory audit
  note, persisted platform settings, a GDPR request queue with second approval, and
  interface language (it/en).
- **Self-hosting:** hostname/FQDN and installation mode are configurable
  (`NOCT_INSTANCE_FQDN`, `NOCT_DEPLOYMENT_MODE=solo|platform`). `solo` is for a
  personal blog or site with a single owner: the first registered user becomes
  Super Admin, and registration closes afterwards. `platform` is for the multi-user
  stack described in the rest of this document. `solo` installs a lean stack
  (PostgreSQL, backend, frontend; local storage, no Redis, NATS or image
  moderation; blogs on paths). The Super Admin creates collaborator accounts
  (`noct-ctl user create` or the admin form) and recovers passwords without
  SMTP (`noct-ctl user reset-password`); see
  [docs/CONFIGURATION.md](docs/CONFIGURATION.md#deployment-profiles-solo-and-platform).
  A Helm chart in [helm/notturni/](helm/notturni/README.md) installs either
  profile on K3s. Every entity in the database has a
  UUID as its primary key, a prerequisite for possible future federation (see
  [ROADMAP.md](ROADMAP.md#5-future-developments-not-started), AT Protocol first).

Details, request/response examples and known limitations (SSO cannot be tested
end-to-end without real credentials) are in [docs/API.md](docs/API.md). For the
status of each feature, including those that are partial or not yet started, see
[ROADMAP.md](ROADMAP.md).

## What is still to do

At a high level (the full, spec-by-spec list is in [ROADMAP.md](ROADMAP.md)):

- **Subdomain routing** (`https://nomeutente.notturni.eu`) and a custom domain
  associated with a blog. Today every post is still reachable without a subdomain,
  via a permalink on `notturni.eu`.
- **GDPR consent register** (export, deletion with a queue and second approval;
  blog deletion with a grace period is already present).
- **Distributed locks** on Redis (rate limiting, by contrast, is already in use, on
  login and link preview) and **clustering** of production components (today,
  single-node Kubernetes).
- **Counts of likes, citations and shares**, and **federation** with other
  instances/platforms (AT Protocol/Bluesky first, then ActivityPub/Mastodon). These
  are explicitly out of the current scope, with the UUID primary key already in
  place for this future work.

## Repository structure

```text
.
├── bins/                # noct-api (HTTP API), noct-worker (jobs and consumers), noct-ctl (tools)
├── crates/               # Rust backend core: registry, runtime, auth, users, profiles, blog, post, media,
│                           audit, administration, bus, cache, storage, mail, markdown, security, codegen...
├── modules/              # optional modules: comments, newsletter, MFA, SSO, GDPR, reports, discovery,
│                           pages, backup... (each with module.toml, api.toml, db.toml)
├── docs/                 # API.md (HTTP contract) and BACKEND.md (backend development and operations)
├── moderation/          # FastAPI microservice for automatic image moderation
│   ├── app/                # self-hosted model (Falconsai/nsfw_image_detection via transformers)
│   └── Dockerfile           # model weights included in the image at build time
├── frontend/            # Next.js application (public + dashboard + administration)
│   ├── src/
│   │   ├── app/            # login/register, dashboard (personal sections), admin (pages/users/blogs/moderation), homepage, public post and blog page, public profile
│   │   ├── lib/             # API client, session (auth-context), theme (theme-context, sun.ts), Markdown/social/languages
│   │   └── components/       # shared UI (Button, Card, SearchInput, ...), editor/ (Tiptap, tags, categories, cover, translations), ThemeToggle
│   └── Dockerfile
├── Dockerfile           # backend image (cargo-chef, multi-arch): noct-api, noct-worker, noct-ctl
├── k8s/                  # Kubernetes manifests (Kustomize, platform reference)
├── helm/notturni/        # Helm chart (solo by default, platform through values)
├── garage/, s3-public/  # Garage configuration and the proxy for public reads
├── compose.yaml         # local stack via Podman/Docker compose: solo by default, platform components behind profiles
├── compose.platform.yaml # override that wires backend and worker to the platform services
├── deny.toml            # cargo-deny: licenses, sources, vulnerabilities
└── .env.example          # environment variables (prefix NOCT_)
```

## Quick start (local)

```bash
scripts/gen-env.sh        # .env from .env.example with random secrets (solo defaults)
podman compose up -d --build
```

This starts the lean solo stack (PostgreSQL, backend, frontend). For the full platform stack, see [docs/INSTALL.md](docs/INSTALL.md#22b-quickstart-platform).

Exposed services:

- Frontend (dashboard and administration included): <http://localhost:3000>
- Backend (OpenAPI): <http://localhost:8000/api/v1/openapi.json>
- Backend (health): <http://localhost:8000/api/v1/health>, readiness: <http://localhost:8000/api/v1/health/ready>
- Uploaded files (local storage): `http://localhost:8000/storage/...`
- Platform profile only: image moderation (health) <http://localhost:8100/health>, S3 storage (Garage) API <http://localhost:3900>, public reads <http://localhost:9000>; MailHog <http://localhost:8025> with `--profile dev-mail`

Migrations of the active modules apply automatically at startup (advisory lock: safe
with multiple replicas). Set `NOCT_AUTO_MIGRATE=false` to apply them elsewhere with
`noct-ctl migrate apply`. The Super Admin is created from `NOCT_SUPER_ADMIN_*`.

If `podman compose` is not available (it requires `podman-compose` or the
`docker-compose` plugin), see the alternatives in
[GETTING_STARTED.md](GETTING_STARTED.md#prerequisites), or start the backend and
frontend locally by following [docs/BACKEND.md](docs/BACKEND.md) and
[frontend/README.md](frontend/README.md).

## Database schema

Each module declares its tables in `db.toml`. `noct-ctl migrate generate` derives the
SQL migrations (`crates/*/migrations/`, `modules/*/migrations/`) from them and updates
`schema.lock.json`. Additive changes (tables, columns with a default or nullable,
indexes, enums) are automatic. Destructive changes require hand-written SQL.
`noct-ctl validate` checks in CI that manifests, the lock file and migrations do not
diverge.

## API and authentication

For the reference of all endpoints, the two authentication mechanisms (user session
vs API token), the token format, and how to get the first one, see
[docs/API.md](docs/API.md). The OpenAPI schema, always aligned with the active
modules, is at `/api/v1/openapi.json`.

A per-module list of all routes (verb, access, purpose) is in
[docs/API_REFERENCE.md](docs/API_REFERENCE.md). All environment variables, with
defaults and examples, are in [docs/CONFIGURATION.md](docs/CONFIGURATION.md).
Deploying with the Helm chart (profiles, values, secrets, operations):
[docs/HELM.md](docs/HELM.md).

## Tests

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo deny check
```

End-to-end tests run on real Postgres (one database per test, created and destroyed by
the test itself). Where needed, they also use Redis, NATS JetStream and a fake S3:
the `NOCT_TEST_*` variables and the test services are described in
[docs/BACKEND.md](docs/BACKEND.md#development). Frontend: `npx tsc --noEmit`,
`npx eslint` and `npm run build` in `frontend/`.

## CI and security

Four GitHub Actions workflows in [.github/workflows/](.github/workflows/):

- [ci.yml](.github/workflows/ci.yml) — on every push and pull request: `cargo fmt`,
  `cargo clippy` with warnings treated as errors, consistency of manifests and
  migrations (`noct-ctl validate`, `migrate plan/apply/verify`), the full test suite on
  Postgres, Redis, NATS and a fake S3, dependency licenses and sources
  (`cargo-deny`), and known vulnerabilities (RustSec, `cargo-audit`).
- [codeql.yml](.github/workflows/codeql.yml) — SAST (static analysis) via CodeQL on the
  backend (Rust), the moderation service (Python) and the frontend
  (TypeScript/JavaScript). Reports appear in the **Security → Code scanning** tab of the
  repository, not as artifacts. Runs on push/PR to `main`/`develop`, on manual start,
  and on a weekly schedule.
- [sbom.yml](.github/workflows/sbom.yml) — SBOM (Software Bill of Materials, CycloneDX
  format): an inventory of dependencies, not a vulnerability analysis. One layer comes
  from the sources (`Cargo.lock`, frontend, moderation; always fast), and one from the
  container images actually built (backend/frontend). Reports are uploaded as workflow
  artifacts. Same triggers as `codeql.yml`.
- [dast.yml](.github/workflows/dast.yml) — DAST (dynamic analysis): starts the whole
  stack from `compose.yaml` (with Docker instead of Podman; the file contains no
  Podman-specific syntax) and runs OWASP ZAP against the frontend's public pages and the
  backend API (from `/api/v1/openapi.json`). It is heavier than the others: manual start
  and weekly schedule only, never on every push/PR. Reports are uploaded as workflow
  artifacts.
- [publish-images.yml](.github/workflows/publish-images.yml) — multi-arch images (amd64
  and arm64) of backend, frontend and moderation, published to GHCR.

SAST, SBOM and DAST do not fail the build when they report findings (`fail_action:
false`, or no threshold check). They produce a report to review, not a gate that blocks
a merge. A threshold could be added in the future. `ci.yml`, by contrast, blocks.

## Backend and frontend locally (without containers)

See [docs/BACKEND.md](docs/BACKEND.md) and [frontend/README.md](frontend/README.md).

## Kubernetes

See [k8s/README.md](k8s/README.md) (Kustomize) and [helm/notturni/README.md](helm/notturni/README.md) (Helm chart).

## Specifications and roadmap

A table of all product specifications with their status (implemented / partial / to do)
and the backlog of upcoming work: [ROADMAP.md](ROADMAP.md).
