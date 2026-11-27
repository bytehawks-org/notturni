# Changelog

All notable changes, grouped by the date of the commit. Entries are reconstructed from the Git history (2026-08-30 to 2026-10-25) and summarize themes; see the commit log for the full detail. Dates are ISO 8601.

Two phases shape the history:

- **Python v1** (2026-08-30 to 2026-09-20): FastAPI backend, Next.js frontend, Podman and K3s deployment.
- **Rust rewrite** (2026-10-22 onward): backend and workers rewritten in Rust; the Python code was removed on 2026-10-24.

For the product roadmap and status of each feature, see [ROADMAP.md](ROADMAP.md). For backend details, see [docs/BACKEND.md](docs/BACKEND.md).

## 2026-11-27

### Changed
- Solo mode: the primary blog's sub-pages are served at the domain root: `/pagina/...`, `/pub/...`, `/bibliografia`, `/media`, `/link`, `/search`. The old `/<primary-slug>/...` addresses of those pages and of the feed files answer `308` to the root path. With `NOCT_SUBDOMAINS_ENABLED=true` the root routing applies to the apex host only; with `false` it applies to every host. Routing: `frontend/src/proxy.ts` and `frontend/src/lib/solo-routing.ts`.
- Solo mode: the root sitemap and the per-Space sitemap list the primary blog's `/pagina/` and `/pub/` entries at their root paths.
- Solo mode: a primary blog that opts out of search or AI crawling gets `Disallow` lines for its root routes (never `Disallow: /`). Code: `PRIMARY_ROOT_DISALLOW` in `crates/noct-posts/src/seo.rs`.
- Solo mode: slugs `bibliografia`, `link`, `media`, `pagina` and `pub` are site routes. New Spaces cannot take them (`400`). The primary blog gets a suffixed slug (`-2`, `-3`, ...) when its title matches one of them.
- Solo mode: at startup the backend logs one warning line with the existing Spaces whose slug collides with a site route. It does not rename or delete them. Code: `warn_route_collisions` in `crates/noct-blogs/src/primary.rs`. Upgrade note: owners of such a Space must rename it by hand.
- Solo mode: the primary blog's address and links never include its slug in the server-rendered home and blog directory.
- Frontend: the per-Space sitemap is read without cache, so a Space that turns indexing off leaves the sitemaps and `robots.txt` at once. Cost: one backend call per request.
- Backend: the paths of a Space's sub-pages come from one helper, `space_path_for` in `crates/noct-core/src/permalink.rs` (`/` for the primary blog's home in `solo`, root paths for its sub-pages).

### Fixed
- Editor: a multi-image upload closed the picker without adding anything. The paragraph inserted after the gallery replaced the gallery node. Both nodes are now inserted in one operation. Regression test: `insertGalleryBlock`.

## 2026-11-25

### Added
- Image gallery in posts (E04, code complete; live and browser checks pending). Markdown format: a fenced block with info string `gallery`, one image per line, max 10 lines. A gallery with one image is saved as a plain image. Sensitive tiles use the `sensitive` or `sensitive:<categories>` title, as single images do.
- Backend: `POST` and `PATCH` of a post return `400` when a `gallery` block has more than 10 image lines (`rules::validate_galleries`, `MAX_GALLERY_IMAGES = 10`). Drafts are checked too.
- Public post: gallery layouts for 1, 2, 3 and 4 or more images. With more than 4 images, the fourth tile shows "+N" (N = images beyond four). The lightbox moves through the whole gallery (arrows, Home/End, Escape, focus back to the opener). Sensitive tiles are skipped until revealed.
- Editor: gallery block with a multi-file picker (1 to 10 files per upload, one `POST /blogs/{slug}/media` per file, per-file errors and retry), alt text, move up/down and remove for each image. Alt text loses `[` and `]` on save.
- Newsletter: the gallery is sent as a sequence of plain images.
- Tests: unit tests for `validate_galleries`, `expand_gallery_fences`, the frontend gallery helpers, renderer, lightbox, editor node view, and a backend integration test for `post_media` with a gallery.
- Solo personal site: one primary blog per instance, created at the first registration and at backend startup (idempotent). Marker `platform_config.primary_blog_id` (migration `0005`, nullable, no foreign key). Title from the new optional `NOCT_SITE_NAME` or else the owner's username; audit event `blog.primary_created`. A blog removed by the purge job is recreated at the next restart. `GET /api/v1/config` returns `primary_blog_slug`. See [docs/API.md](docs/API.md#solo-mode-primary-blog-and-personal-site-routes) and [docs/CONFIGURATION.md](docs/CONFIGURATION.md#solo-personal-site).
- Frontend: `/` shows the primary blog in `solo`; `/post/<slug>` serves its posts; `/<primary>` and `/<primary>/<post>` redirect (308) to `/` and `/post/<post>`; root `feed.xml` and `atom.xml` serve the primary blog.
- Backend helpers: `AppState::is_solo()` and the permalink helper `noct-posts::permalink` (primary blog at `/post/<slug>`, others at `/<blog>/<slug>`).
- Tests: `solo_primary_blog`, `solo_primary_permalink`, `solo_slugs`, `solo_admin_config`, `solo_interests`, `solo_site_texts` (backend, against PostgreSQL) and Vitest tests for the solo UI.
- Workflow `publish-chart.yml`: publishes the Helm chart as an OCI artifact to `oci://ghcr.io/bytehawks-org/charts/notturni`. Pushes to `develop` publish `<chart version>-develop` and `<chart version>-develop.<sha7>` with `appVersion: develop` (default images tagged `develop`); `vX.Y.Z` tags publish that version. See [docs/HELM.md](docs/HELM.md#installing-from-the-oci-registry-no-clone).

### Changed
- CI and image publishing no longer reference the removed `migration/rust` branch (workflows `ci`, `codeql`, `sbom`, `publish-images`; the fixed `0.2.0-rust` image tag is gone). Images, chart `appVersion` and the Kubernetes manifests now use `0.2.1`, published by pushing the `v0.2.1` git tag. `docs/INSTALL.md` section 3.4 updated.
- Solo: the primary blog no longer counts toward `max_blogs_per_user` (creation and ownership transfer). Platform mode is unchanged, also when a marker is left over from a switch from `solo`. Tests: `solo_primary_blog` (solo limit, platform limit, leftover marker).
- Solo: the primary blog cannot be transferred. `POST /api/v1/blogs/{slug}/transfer` returns `400` ("Il blog primario non può essere trasferito."); the owner stays unchanged. Other blogs transfer as before. Platform mode is unchanged. Tests: `solo_primary_blog`.
- Solo: space slugs are checked only for format and collisions with site routes (no minimum length, no reserved list); create-space form shows the path address and no minimum length.
- Solo: `reserved_blog_names` is rejected by `PATCH /admin/config` (`400`) and returned as `[]`; the admin settings page hides the reserved-names and interests editors.
- Solo: user interests are off (`GET /api/v1/interests` returns `[]`, profile `PATCH` with a non-empty list returns `400`, UI hidden). Stored values stay in the database and in the GDPR export.
- Solo: `PUT /api/v1/admin/site-texts/{locale}` returns `403`; the `/admin/testi` page redirects to `/admin`. Footer texts stay editable.
- Texts in `.env.example` and the Helm chart now show the path address of a space as `/<slug>` (they said `/blog/<slug>`).
- Platform behaviour is unchanged.

### Fixed
- Frontend `getPrimaryBlogSlug()` revalidates every 5 seconds instead of using a long cache. A cached `null` from before the first registration no longer delays the home page and the redirects.
- Solo: the global sitemap and the per-Space sitemap list the primary blog as `/` and its posts as `/post/<slug>`, instead of the `/<slug>/<post>` addresses that redirect.

### Verified
- Live check on podman compose, solo and platform (2026-11-25): stack start, primary blog and routes, API restrictions, sitemaps. Not yet checked: the browser redirect of `/admin/testi` and the visual check of the solo UI. See [ROADMAP.md](ROADMAP.md).

## 2026-11-23

### Added
- Lean **solo** install profile: PostgreSQL, backend and frontend only, with local storage on a volume, in-memory cache and bus, and no image moderation. `.env.example` now defaults to it. See [docs/CONFIGURATION.md](docs/CONFIGURATION.md#deployment-profiles-solo-and-platform).
- `NOCT_SUBDOMAINS_ENABLED` (default `true`; solo sets `false`). The backend exposes it in the public config and in the admin infrastructure block, together with `worker_inline`, `local_storage_public_url`, `modules`, `bus` and `cache`; the frontend drops the wildcard CSP origin and the subdomain rewrite and shows path addresses when it is `false`.
- Compose profiles: `compose.yaml` starts the solo stack; `compose.platform.yaml` with `--profile platform` adds Redis, NATS, Garage, `s3-public`, moderation and the worker; `mail-relay` and `dev-mail` profiles for the SMTP relay and MailHog. The local storage lives in a named volume.
- `noct-ctl user create` and `POST /api/v1/admin/users` (Super Admin only), plus an admin form (`/admin/utenti/nuovo`): collaborator accounts in solo mode, where registration is closed. Generated passwords are shown once.
- `noct-ctl user reset-password`: password recovery without SMTP; revokes sessions and writes an audit event.
- Admin infrastructure panel (read-only) in `/admin/impostazioni` and a first-run wizard at `/admin/setup` that only prints `.env` and Helm values.
- Helm chart `helm/notturni` (solo by default, platform through values), a `helm` job in CI (`helm lint`, `helm template`, Helm 3.19.0) and chart tests. See [helm/notturni/README.md](helm/notturni/README.md), which also holds the manual smoke-test checklist. Deployment guide: [docs/HELM.md](docs/HELM.md).
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
