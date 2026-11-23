# Changelog

All notable changes, grouped by the date of the commit. Entries are reconstructed from the Git history (2026-08-30 to 2026-10-25) and summarize themes; see the commit log for the full detail. Dates are ISO 8601.

Two phases shape the history:

- **Python v1** (2026-08-30 to 2026-09-20): FastAPI backend, Next.js frontend, Podman and K3s deployment.
- **Rust rewrite** (2026-10-22 onward): backend and workers rewritten in Rust; the Python code was removed on 2026-10-24.

For the product roadmap and status of each feature, see [ROADMAP.md](ROADMAP.md). For backend details, see [docs/BACKEND.md](docs/BACKEND.md).

## 2026-11-23

### Added
- Lean **solo** install profile: PostgreSQL, backend and frontend only, with local storage on a volume, in-memory cache and bus, and no image moderation. `.env.example` now defaults to it. See [docs/CONFIGURATION.md](docs/CONFIGURATION.md#deployment-profiles-solo-and-platform).
- `NOCT_SUBDOMAINS_ENABLED` (default `true`; solo sets `false`). The backend exposes it in the public config and in the admin infrastructure block, together with `worker_inline`, `local_storage_public_url`, `modules`, `bus` and `cache`; the frontend drops the wildcard CSP origin and the subdomain rewrite and shows path addresses when it is `false`.
- Compose profiles: `compose.yaml` starts the solo stack; `compose.platform.yaml` with `--profile platform` adds Redis, NATS, Garage, `s3-public`, moderation and the worker; `mail-relay` and `dev-mail` profiles for the SMTP relay and MailHog. The local storage lives in a named volume.
- `noct-ctl user create` and `POST /api/v1/admin/users` (Super Admin only), plus an admin form (`/admin/utenti/nuovo`): collaborator accounts in solo mode, where registration is closed. Generated passwords are shown once.
- `noct-ctl user reset-password`: password recovery without SMTP; revokes sessions and writes an audit event.
- Admin infrastructure panel (read-only) in `/admin/impostazioni` and a first-run wizard at `/admin/setup` that only prints `.env` and Helm values.
- Helm chart `helm/notturni` (solo by default, platform through values), a `helm` job in CI (`helm lint`, `helm template`, Helm 3.19.0) and chart tests. See [helm/notturni/README.md](helm/notturni/README.md), which also holds the manual smoke-test checklist.
- Files served from `/storage/` (local storage) now carry `X-Content-Type-Options: nosniff` and `Cache-Control: public, max-age=3600`; the former `no_store_sensitive` middleware is now `response_hardening`.
- Vitest and Testing Library for the frontend, with a CI job; tests for the in-memory bus across a restart (`solo_membus.rs`).

### Changed
- In solo the UI hides the verification seal and role labels, and replaces the automatic-moderation line of the media library with a manual-marking hint. Permissions and API fields are unchanged.
- `k8s/configmap.yaml` sets `NOCT_DEPLOYMENT_MODE=platform` and `NOCT_SUBDOMAINS_ENABLED=true` explicitly.
- The frontend allows the local storage origin in `img-src` (`NOCT_LOCAL_STORAGE_PUBLIC_URL`) and reads `NOCT_DEPLOYMENT_MODE` at runtime.
- Public media bibliography blurs images that are flagged sensitive or carry categories (marking by hand without the moderation module).
- Breaking for compose users: `podman compose up -d --build` no longer starts Redis, NATS, Garage, moderation or the worker. Use `-f compose.yaml -f compose.platform.yaml --profile platform`, and `scripts/gen-env.sh --platform` (new option) to generate and uncomment the S3 and Garage secrets.

### Documentation
- Solo and platform profiles, how to switch components on, MemoryBus limits, image moderation off (operator responsibility, GDPR and NIS2), collaborators and their roles, password recovery, the module-table caveat, Helm and compose installation, and the roadmap rows: [docs/CONFIGURATION.md](docs/CONFIGURATION.md), [docs/INSTALL.md](docs/INSTALL.md), [GETTING_STARTED.md](GETTING_STARTED.md), [ROADMAP.md](ROADMAP.md).

## 2026-10-25

### Added
- Spaces for blogs, wiki and docs, with a file tree, revisions, display modes and Git synchronization.
- Optional wiki and docs spaces (opt-in), synchronized over Git, also through SSH.
- Privacy and cookie policy pages, portal identity and SEO settings, sitemap, link map, and a list of known SSH hosts.
- Tag page and post actions.

### Changed
- Site URL, API URL and platform name are configurable at runtime in the frontend.
- Uploaded images are stored in WebP variants.
- Repository cleaned up after the Rust migration; Kubernetes manifests updated.
- A GitHub Action workflow was modified.

### Fixed
- Notes, tags and link cards rendering; full cache invalidation.
- Findings from Lighthouse audits.

## 2026-10-24

### Changed
- Backend and workers rewritten from Python to Rust in phases: declarative module system, core infrastructure, authentication and audit, blog, post and image core, categories, publications, notes, fragments, comments, discovery, follow and author views, two-factor authentication, external identity providers, profiles, newsletter, reports, custom domains and GDPR.
- The migration ends with removal of the legacy Python code, operational hardening and NATS JetStream for events and work queues.
- Migration plan revised toward a greenfield, declarative and modular design; decisions recorded, including hot activation of modules.
- Frontend fonts are self-hosted with Fontsource packages instead of `next/font/google`.
- GitHub Actions updated to runtimes on Node.js 24.
- Multi-architecture images are built on native runners and combined into one manifest. QEMU is no longer used.

### Added
- Images published from the `migration/rust` branch with the image tag `0.2.0-rust`. Kubernetes manifests point to this tag.

### Security
- Security review: example secrets are rejected at startup, Kubernetes hardening, and an installation guide.
- Rust core rule: no `unsafe` code.

## 2026-10-22

### Changed
- Object storage migrated from MinIO to Garage (S3-compatible).

## 2026-09-20

Tag `0.1.4-alpha` points to the Kubernetes image tag commit (`b5c1569`).

### Added
- Super Admin is excluded from the user directory. Per-blog storage quota and gold, silver and blue verification seals.
- Editor: text alignment and underline, code block syntax highlighting, monospace font, a two-row toolbar, and images picked from the media library (own blogs or all blogs).
- Dashboard links and languages for user interests.
- SSO sign-in buttons.
- Tests for the user directory, storage quota, verification seals and monospace font.

### Changed
- Gold and silver seal assignment inverted.
- Kubernetes: `local-path` storage class, MinIO image pulled from Quay, first-release configuration, central image tag through Kustomize `images`.
- MinIO exposed read-only on the `s3` host; `s3` added to the reserved blog names.
- Pull requests #43 and #44 merged (public release fixes on K3s).

### Fixed
- Content Security Policy with a per-request nonce, which unblocks login and registration in production.
- The SSO callback always ends with a redirect, never with raw JSON.
- SMTP: `SMTP_SSL` on port 465; STARTTLS only for other ports.

## 2026-09-19

Tag `0.1.1` points to the merge of pull request #42.

### Added
- Global search, user directory, user interests, and admin-initiated password reset.
- Newsletter and mailing list with double opt-in: subscription, management and sending, in backend and frontend. Campaign editing and cancellation, a rich editor, and alt text for cover images.
- Homepage languages with post counts, username availability check, password change, and multi-blog views.
- Kubernetes manifests for the worker and moderation services; cert-manager and production TLS.

### Fixed
- Infinite loop in the newsletter worker; the newsletter toggle shows the real state.
- Two diverging Alembic heads merged; `schema.sql` realigned.
- Six defects found in the review of pull requests #38 to #42, before the public release.

### Documentation
- ROADMAP and API reference updated.

## 2026-09-18

### Fixed
- Backend: security and correctness fixes from the post-merge review.
- Frontend and Kubernetes: accessibility, security and bug fixes.
- Compile errors in the reference frontend kit.
- Domain verification keeps its `failed` state after a page refresh.
- Remaining issues from the review pass; ROADMAP and API reference updated.

## 2026-09-17

Tags `0.1.2-aplha` and `0.1.3-aplha` (sic: tag names kept as they are in the repository) point to the merge of pull request #35.

### Added
- Subdomain-per-blog routing on K3s.

### Security
- CodeQL findings fixed: server-side request forgery (SSRF), polynomial regular expression (ReDoS), and URL sanitization.
- Security hardening for the alpha release.
- SAST and DAST fixes merged (pull request #37).

## 2026-09-16

### Added
- Public profile shows the verified domain as handle and signature. Avatar and favicon shown everywhere; post index with an H1 heading.
- Bibliography feature merged (pull request #32); profile fields and checks merged (pull request #31).

### Changed
- Performance and SEO: fixed N+1 queries for comments, server-side rendered public profile, `next/image`, lazy-loaded editor.

## 2026-09-15

### Added
- Profile verification: email confirmation with double check, username change cooldown, custom domain through DNS, badges, Fediverse ID, fixed sidebar.
- Structured notes in the editor, with a modal and optional bibliographic fields.
- Bibliography fields compatible with BibTeX (type, publisher or journal, year and date, URL).

### Changed
- Editor refinements: image lightbox, automatic slug, always-visible rail, collapsible sidebar.

## 2026-09-14

### Added
- Blog typography and layout applied to the public rendering.
- SEO: JSON-LD on posts; metadata on public profile and blog pages.
- Role-based access to the blog settings tabs.
- Optional blog cover image and favicon.
- Configurable audit log retention in the admin settings.
- RSS 2.0 and Atom 1.0 feeds, for the platform and for each blog.
- Link preview cache in Redis and in persistent storage, deduplicated by URL.
- Audit log entries for blog invitations, memberships and static pages.
- Pull request #30 merged (new UX and UI). Remaining hard-coded Italian strings moved to the i18n catalogs.

### Changed
- Three-column footer and bottom bar on every public page. Link back to the portal in the blog top bar.

## 2026-09-13

### Added
- Interface i18n with next-intl and 1:1 alignment with the design mockups (blocks A0 to A10): login and MFA, editor chrome, user profile, fragments shelf.
- Backend endpoints for the redesign: overviews, per-user lists, followed feed, directory with counts.
- Blog settings with a danger zone: pause, transfer, export, and deletion with a grace period.
- Extended comment queue: blocked users per blog, reporting to the platform, automatic closing.
- Reader reports, admin reports panel, and a mandatory audit note.
- Blog media library, blog notes as entities (type, URL or DOI, duplicates, merge, BibTeX export), and publications (post series as chapters, public index).
- Persisted platform settings, GDPR request queue, and per-account interface language.

## 2026-09-12

### Changed
- UI redesign, phases 1 to 5 (partial): new colour palette and fonts (Lora and Source Sans 3), primitive components, layout shell (dashboard shell, site header and footer, blog header), public blog directory at `/blogs`, restyle of the blog dashboard tabs (partial).
- Desktop sidebar of the dashboard and admin is always full height.

## 2026-09-07

### Added
- GitHub Actions workflow publishes images to GHCR.
- Kubernetes manifests adapted for a first test on K3s without domain or TLS.

### Security
- User session moved from `localStorage` to an httpOnly cookie, with CSRF protection.

### Changed
- SAST, DAST and SBOM reports are visible without blocking the pipeline.

## 2026-09-06

Tag `v0.1.0` points to the merge of pull request #13.

### Added
- Interface to create and revoke user API tokens.
- Rate limiting through Redis on login and link preview.
- Moderator role linked to a cross-blog comment moderation panel.
- Real SMTP delivery for the email OTP MFA code; containerized SMTP service for Podman and Kubernetes.
- Periodic backup of PostgreSQL and MinIO to an external S3 bucket.
- Self-service GDPR export and account deletion.
- Public comments: three modes, threads, and CAPTCHA for anonymous users.
- Fragments can be public or private, chosen at save time or later.
- QR code for TOTP setup, generated by the backend.
- Crawler opt-in, separately for search engines and AI or LLM crawlers, per blog and per post.
- SEO: `metadataBase`, canonical URLs, Open Graph, `sitemap.xml`.
- GitHub Actions for SAST, DAST and SBOM.
- Public homepage for each blog.

### Changed
- URL structure: platform pages under `/p/{slug}`, admin sections under `/admin/*`, post permalinks without date (`/{blog}/{slug}`).
- Audit log records the web or API channel, blog alias and comment moderation.
- Blog typography and palette limited to the product aesthetic guidelines.

### Documentation
- Internal link audit documented after the URL restructuring.

## 2026-09-05

### Added
- Audit log system: writes, retention, archive and query.
- Local filesystem storage backend (`localstorage`).

### Changed
- Storage client naming generalized from MinIO to S3.
- Delivery optimizations and code reorganization (ROADMAP.md section 6).
- Pull requests #9, #10 and #11 merged (administration, local storage, audit log).

## 2026-09-04

### Added
- Fragments: highlighting and a personal collection of text excerpts.
- Platform administration as a separate app, with a blog section and search.
- Post moderation in the administration area.
- Super Admin bootstrap through environment variables, without manual database steps.

### Changed
- Administration moved into a separate frontend app (`frontend/admin/`), then brought back into a single frontend app.
- Pull requests #7 and #8 merged (fourth batch and fragments).

## 2026-09-03

### Added
- Multilingual static pages, user identity, and a Bluesky-style composer.

### Fixed
- Blur, link card and blank line issues after media and links.

## 2026-09-02

### Changed
- Feature branches merged into the main line: pull requests #1 (WYSIWYG editor), #2 (second batch) and #3 (third batch).

## 2026-09-01

### Added
- Blog, user and editor features: visibility and invitations, user mentions, footnotes.

### Documentation
- Internal development guidelines revised for block-based work sessions.

## 2026-08-31

### Added
- Readable permalinks and a public post page, without subdomain.
- Homepage: chronological feed of posts from all blogs, with a trending section.
- Tags on posts (maximum 5), with hashtag extraction from the text.
- Categories per blog and a category filter in the feed.
- Extended user profile: name, country, mother tongue and fallback languages.
- Blog pen name and default author display name; configurable social links.
- Editor: H1 to H3 headings, tables, image alt text, language picker for translations, and tag labels on the public post page.
- Self-hosted image moderation service (`moderation/`), with an NSFW classifier. Sensitive images are blurred and revealed on click.

### Changed
- Editor restyled: no borders, larger serif title, and actions moved to the top.

### Documentation
- README updated with implemented features and open work.

## 2026-08-30

### Added
- First commit of the project.
- Rich text editor (Tiptap) and cover image for posts.
