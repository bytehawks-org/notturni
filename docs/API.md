# Notturni API

Reference of the endpoints exposed by the backend (Rust, workspace `noct-*`). The
OpenAPI schema, always in sync with the code and limited to the active modules, is
at `/api/v1/openapi.json` (or `cargo run -p noct-ctl -- openapi`). This file
adds the context (intended use, consumers, examples, business rules) that
OpenAPI alone does not provide.

Full list of routes per module: [API_REFERENCE.md](API_REFERENCE.md). Environment variables: [CONFIGURATION.md](CONFIGURATION.md).

The contract is the one of v1 (Python/FastAPI), with some differences introduced by the port to Rust.
Two differences apply to the whole document: validation errors for request bodies are `400`
with `{"detail": "…"}` (FastAPI returned `422` with a list), and the routes and fields of an
optional module that is turned off do not exist (`404`, fields absent from the responses).

The endpoints below are used by the frontend (login, management of blogs/posts/comments/pages, profile, follow — see `frontend/README.md`) and
by direct API access (core engine, scripts, future third-party clients). For the status of each feature, see
[ROADMAP.md](../ROADMAP.md).

## Multilingual (i18n)

Posts and static pages share the same translation schema: each row
(a post, a page) has a `locale` (two-letter ISO 639-1 code: `it`,
`en`, `de`, ...) and a `translation_group_id`. Rows with the same
`translation_group_id` are translations of the same logical content, each
with its own slug (the slug can be translated too, e.g.
`il-mio-post` in Italian and `my-post` in English).

There is no dedicated "list available languages" endpoint. The frontend is
meant to route `/it/...`, `/en/...`, `/de/...` at the routing level
(both for notturni.eu and for the individual blogs) and to pass the locale extracted from the
path as a query param (`?locale=it`) or as part of the path (`/blogs/{slug}/posts`)
to the corresponding API calls. The backend does not impose or recognize the
`/it/...`/`/en/...` structure by itself. It is a frontend convention to follow
when the routing is built.

A blog has a `default_locale` (used when a post is created without
specifying `locale` explicitly). There is no default_locale for the main
site instead. Each static page must be created explicitly for each
supported language.

## Two distinct authentication mechanisms

| Mechanism            | Transport                                               | Endpoints that require it                                        | Obtained from                               |
| --------------------- | ------------------------------------------------------ | -------------------------------------------------------------------- | ---------------------------------------------- |
| **User session**     | JWT access token (15 min) via `Authorization: Bearer`; refresh token via `httpOnly` cookie | `/auth/me`, `/auth/mfa/*`, `/blogs`, `/posts`, `/comments`          | login (`/auth/login`, SSO)                  |
| **API token**        | Opaque, prefix `noct_`, via `Authorization: Bearer`, no expiry by default    | `/tokens`                                                             | bootstrap script or an already valid token    |

The first (session) represents *who you are* (a user logged in from the browser or from a
client that logged in). The second (API token) represents *direct access*: the core engine today, and in the future
also users who want to interface with the API without going through the blog's editor or admin. Details
on API tokens are at the end of this file (section unchanged from the
previous version).

### User session: access token in memory, refresh token in `httpOnly` cookie

The refresh token is no longer returned
in the JSON body. It travels only in an `httpOnly` + `Secure` +
`SameSite` cookie, set by the backend on `/auth/login`, `/auth/mfa/verify`,
`/auth/sso/{provider}/callback` and `/auth/refresh` (name `noct_refresh_token`,
path restricted to `/api/v1/auth`, so the browser does not send it on the other
API requests). The cookie attributes (`Secure`/`SameSite`/`Domain`) are
configurable via `NOCT_SESSION_COOKIE_*` (`.env.example`). This is needed for
environments without TLS yet active (K3s at the start of the rollout, before
cert-manager) or with frontend and backend on different domains. The access token stays
in the JSON body. The frontend does not persist it (no `localStorage`). It keeps it
only in memory and sends it as before via `Authorization: Bearer`. See
`frontend/src/lib/auth-context.tsx`.

Endpoints that authenticate with the cookie alone (`/auth/refresh`, `/auth/logout`)
also require the `X-CSRF-Token` header, equal to the value of the **non**
`httpOnly` cookie `noct_csrf_token` set together with the refresh token
(*double-submit* pattern: a foreign origin cannot read that cookie
to build the header, even if the browser attaches the cookie
anyway). All other endpoints that change state stay authenticated
via `Authorization: Bearer` with the access token, which is CSRF-safe in itself:
a foreign origin cannot set that header on a cross-site request.

A password change (`POST /users/me/password`, "forgotten password" reset)
deletes the sessions (`UserSession`, therefore the refresh tokens). However, an access
token JWT already issued would otherwise remain valid until its natural `exp`
(15 minutes), because it is stateless. `User.credentials_changed_at` closes this
window: `get_current_user`/`get_current_user_optional` compare the token's `iat`
with this field and reject (401) every token issued before the last
password change.

## User authentication (password, MFA, SSO)

### Registration and password login

**`POST /api/v1/auth/register`**

```json
{"username": "mario", "email": "mario@example.com", "password": "..."}
```

→ `201`, user created (`platform_role=utente`, MFA disabled). No automatic
login: it must be done separately. `409` if the username or email is already in use.
`400` if the username does not respect the format: lowercase letters,
digits, `-` and `_` as internal separators (never at the start or end, nor repeated),
3–32 characters, and not in the blacklist. The username is
the identifier that can be cited as `@username` in the content. `400` also if the
password is shorter than 10 characters (the only
requirement of the policy, with no constraints on character classes). The same
check is applied by the "forgotten password" flow below.

If `NOCT_DEPLOYMENT_MODE=solo` (single-owner installation, e.g. a personal blog;
see `.env.example`), the **first** registered user automatically becomes
`super_admin` (no manual intervention on the DB). Every later registration returns `409`.
With `NOCT_DEPLOYMENT_MODE=platform` (default), the behavior is the multi-user one
described above, with no limits.

**`POST /api/v1/auth/login`**

```json
{"email": "mario@example.com", "password": "..."}
```

→ `200`. Two possible forms:

- MFA not active: direct session. The refresh token is not in the response
  body: the backend sets it as an `httpOnly` cookie (see above).
  ```json
  {"access_token": "...", "token_type": "bearer"}
  ```
- MFA active: requires a second step.
  ```json
  {"mfa_required": true, "method": "totp", "challenge": "..."}
  ```
  If `method` is `"email"`, the sending of the
  code has already been queued at this point (see Email OTP details below).

`401` for wrong credentials or a deactivated user. `429` beyond 20 attempts per 5
minutes from the same IP, or beyond 5 attempts per 5 minutes on the same email
(brute-force protection via Redis. If Redis is not
reachable, the limit is simply disabled: fail open).

**`POST /api/v1/auth/mfa/verify`** — completes the login after an
`mfa_required` response.

```json
{"challenge": "...", "code": "123456"}
```

→ `200` with `access_token` as above (refresh token in the cookie). `401` if the
code is wrong or expired, or if the challenge is no longer valid (it lasts 5 minutes).
`429` beyond 20 attempts per 5 minutes from the same IP, or beyond 8 attempts per 5 minutes
on the same subject of the challenge (same protection as the login, also applied
to `mfa/totp/confirm` and `mfa/email/confirm` below. A 6-digit code is
otherwise guessable within a manageable number of attempts).

**`POST /api/v1/auth/refresh`** — no body. The refresh token is read from the
`noct_refresh_token` cookie. It requires the `X-CSRF-Token` header (see above).

→ `200`, new `access_token` and new refresh cookie (rotation: the
previous token is revoked, and reusing it gives `401`). `401` without a session cookie
or with an expired or already rotated refresh token. `403` without a valid `X-CSRF-Token`.

**`POST /api/v1/auth/logout`** — no body, same cookie and header as above.

→ `204`. Revokes the session and clears the cookies. Idempotent: no error
if the session was already revoked or the cookie is absent. `403` without a
valid `X-CSRF-Token`.

**`GET /api/v1/auth/me`** — requires a session. Returns
`{id, username, email, mfa_enabled}`.

**`GET /api/v1/auth/username-available?username=...`** — public, no
session required. Rate limit 30 requests/minute per IP. Checks the format
(length, allowed characters, reserved words) and uniqueness with the same query as
`register_user`:

```json
{"available": true, "reason": null}
{"available": false, "reason": "invalid_format"}
{"available": false, "reason": "taken"}
```

It is meant for the live check while the user types during
registration. It does not replace the uniqueness validation done anyway at
`POST /auth/register` (a race condition between the check and the submit is always
possible, and is handled there).

### Password change while logged in

**`POST /api/v1/users/me/password`** — requires a session.

```json
{"current_password": "...", "new_password": "..."}
```

→ `200 {"status": "ok"}`. Checks `current_password` against the existing hash
(same function as the login), applies the minimum policy of 10
characters, and **revokes all active sessions** of the user. This follows
the same principle as `POST /auth/password/reset` above,
applied here explicitly because until now there was no way to
change the password while staying logged in, only the reset via email (without the
old password). The current session is also closed: the client
must log in again with the new password. `400` if `current_password` is
wrong, if `new_password` does not respect the minimum policy, or if the account
has no password set (user linked only via SSO).

### Forgotten password

**`POST /api/v1/auth/password/forgot`**

```json
{"email": "mario@example.com"}
```

→ **always** `202`, whether the email exists or not. The response must never
allow enumerating which addresses have an account. If the email matches an
active user, it queues a 6-digit code by email (same mechanism as the
MFA/email change OTP: a job on the NATS JetStream message bus,
TTL 10 minutes). `429` beyond 20 attempts per 5 minutes from the same IP, or 5 per 5
minutes on the same email.

**`POST /api/v1/auth/password/reset`**

```json
{"email": "mario@example.com", "code": "123456", "new_password": "..."}
```

→ `204`. Sets the new password (same minimum policy of 10 characters as
above) and **revokes all active sessions** of the user (every refresh token
already issued stops working, as in account deletion).
A password reset is typically a response to a
compromised account, not only to a forgotten password. `400` if the
code is wrong, expired, or already used, or if `new_password` does not respect the
minimum policy. The same generic message is returned for an unknown email and for a
wrong code. `429` with the same limits as `password/forgot`.

### MFA — management (requires an active session, i.e. a login already done)

**`POST /api/v1/auth/mfa/totp/setup`** → generates a TOTP secret (not yet
active) and returns it together with a `provisioning_uri` (`otpauth://...`) and a
QR code ready to show (`qr_code_data_uri`, an SVG generated entirely on the
backend with the `qrcode` crate, never by a
third-party service: the secret must not leave the backend):

```json
{
  "secret": "BASE32...",
  "provisioning_uri": "otpauth://totp/Notturni:mario%40example.com?...",
  "qr_code_data_uri": "data:image/svg+xml;base64,..."
}
```

**`POST /api/v1/auth/mfa/totp/confirm`** — `{"code": "123456"}` → `204` and
activates the MFA (`mfa_enabled=true`, `mfa_method="totp"`). `400` if the code does not
match the secret generated by the setup.

**`POST /api/v1/auth/mfa/email/setup`** → `202`, generates and queues (job on the NATS bus)
the sending of a code to the user's email (see Email OTP details below).

**`POST /api/v1/auth/mfa/email/confirm`** — `{"code": "123456"}` → `204` and
activates the MFA (`mfa_method="email"`). `400` if the code does not match or has
expired (validity 10 minutes).

**`POST /api/v1/auth/mfa/disable`** `{"code"?}` — to disable the MFA, a
valid code is required (a stolen access token is not enough to remove the second factor).
Without `code` it returns `202 {"code_required": true, "method": "totp"|"email"}`
(with the email method it also sends the code). With a valid `code` (from the app or
received by email; a TOTP code already used does not count) it returns `204`, disables the MFA and deletes the
stored TOTP secret. `400` for a wrong or expired code. With
`NOCT_MFA_DISABLE_REQUIRES_CODE=false` the v1 behavior returns (`204`
immediately, no code). TOTP secrets are encrypted at rest (`NOCT_SECRETS_KEY`).

### SSO (Google, Microsoft, GitHub, LinkedIn)

**`GET /api/v1/auth/sso/{provider}/login`** — redirect to the provider's authorize URL.
`503` if the provider has no configured credentials (see
`.env.example`, variables `NOCT_OAUTH_*`).

**`GET /api/v1/auth/sso/{provider}/callback`** — receives the redirect from the
provider, exchanges the code, fetches the userinfo and applies the account linking:

- no user with that linked identity and no user with that email
  → creates a new one (username derived from the local part of the email);
- identity already linked → direct login on the associated user;
- existing user with the same email, **without** MFA → immediate linking
  and login;
- existing user with the same email, **with** MFA active → does not link
  right away, and requires verification of the code. The linking is completed
  inside `/auth/mfa/verify`, which recognizes the
  challenge as a "pending SSO linking" and finalizes it after the
  code is verified.

**Response: always a redirect (`303`) to the frontend, never JSON.** Unlike every other endpoint in this section, this one is reached by a
real browser navigation (OAuth redirect), not by a fetch. No JS code is there to read
a JSON body, so it must end on a real page (bug fixed: it previously returned raw JSON, shown on screen instead of
being handled by the app). Successful login → `{frontend}/dashboard` (session
cookies already set on the redirect response; the frontend's silent refresh on mount
retrieves the access token). MFA to verify →
`{frontend}/login?mfa_challenge=...&mfa_method=...` (same OTP form as the
password login). Error (provider not configured, OAuth failed, email
not available from the provider) → `{frontend}/login?sso_error=1`. `{frontend}`
is the first origin of `NOCT_CORS_ORIGINS`.

**Known limitation:** without real OAuth credentials (client id/secret for
each provider), the flow cannot be tested end-to-end in this development environment.
The account linking logic is tested directly instead. The `/login` and `/callback` endpoints must be verified
with real credentials before production use. In particular for
LinkedIn, the exact details of the userinfo endpoint may require
adjustments.

**Email OTP:** the code is generated, stored (hashed) and queued as a job
on the NATS JetStream bus (transactional outbox). The worker
sends it via SMTP (crate `noct-mail`, `lettre`:
`NOCT_SMTP_HOST`/`_PORT`/`_USER`/`_PASSWORD`/`_USE_TLS`/`_FROM_EMAIL`, see
`.env.example`). Locally it points to the `mailhog` service of `compose.yaml`
(no real email is sent out; it can be inspected at `http://localhost:8025` or via
`GET http://localhost:8025/api/v2/messages`). In production it must always
point to a real SMTP/transactional provider. Without `NOCT_SMTP_HOST`
set, the OTP is only logged by the consumer (convenient for development without
an SMTP server available, never the production case). A genuine SMTP error
(host unreachable, wrong credentials) nacks/requeues the message.

## Blog

**`POST /api/v1/blogs`** — requires a session.

```json
{
  "slug": "il-mio-blog",
  "title": "Il mio blog",
  "default_locale": "it",
  "subtitle": "Appunti sparsi",
  "description": "Descrizione breve del blog, max 256 caratteri.",
  "kind": "blog",
  "display_mode": "feed",
  "visibility": "public",
  "default_author_display_name": "il-mio-username"
}
```

`kind` (`blog` | `wiki` | `docs`, default `blog`) is the type of the **Space** and
cannot be changed after creation (it is not accepted by `PATCH`). In the
product terminology "blog" and "Space" are the same in the routes: `/blogs/...`
is the same contract as `/spaces/...`, an alias rewritten before routing (it does not
appear in `openapi.json`).

`display_mode` (`feed` | `wiki` | `docs`) is the structure of the public pages and depends on the type. A
`blog` uses only `feed`. `wiki` and `docs` choose between `wiki` (page tree, breadcrumbs, backlinks) and
`docs` (sidebar, page table of contents on the right, previous/next). If absent, the default of the
type applies. It can be changed with `PATCH /blogs/{slug}`. A value not allowed for the type → `400`. It is distinct from
`layout` in the presentation configuration (`/blogs/{slug}/config`), which only concerns the arrangement
of the cards in the feed. For the tree, the revisions and the internal links of wiki/docs Spaces, see
the section "Wiki and documentation spaces".

→ `201`. Applies the domain rules (see
[ROADMAP.md](../ROADMAP.md#1-product-and-domain-rules)): slug of at least 4
alphanumeric characters or hyphens, not in the blacklist (names reserved by the platform
and by the administrator), at most 5 Spaces per user (a single limit for blog, wiki and docs, `max_blogs_per_user`; in `solo` the primary blog is not counted, see [Solo mode](#solo-mode-primary-blog-and-personal-site-routes)). `default_locale` is
optional (default `it`). `subtitle` (max 64) and `description` (max 256) are
optional. `default_author_display_name` is optional (the frontend pre-fills it
with the username of the person creating the blog, but it remains a free field,
editable later with `PATCH`). If absent or empty, the public name
falls back to the profile preference of the author (`username` by default).
`visibility` is optional (default `public`). Values:

- `public`: reachable by anyone, appears in the homepage feed;
- `members`: public pages of the blog readable only by a user authenticated on the platform;
  excluded from the feed;
- `private`: diary. Readable only by the owner and collaborators,
  **writable only by the owner** regardless of memberships.

`400` if a rule is not respected, `409` if the slug is already in use. In `solo` the slug rules are
different (see "Solo mode" above): only the format and the collisions with site routes are checked.

`BlogOut` (response of all blog endpoints) also includes
`mentions_enabled` (bool, default `true`). If active, the frontend turns
the `@username` mentions in post content into links to the mentioned profile. See
"Mentions `@username`" in the Post section.

**`GET /api/v1/blogs`** — public, no session required. Directory of the
public blogs that can be indexed: `visibility=public`, not suspended
(`is_suspended=false`) and `search_indexing_enabled=true`. This is the same criterion as the
generated `robots.txt`, more restrictive than the plain "public" used by the post feed
(which also includes public blogs that are not indexable). Parameters
`limit` (default 30, max 100) and `offset` for pagination. `kind` (`blog`|`wiki`|`docs`) filters by Space type, otherwise all are listed. `q` searches in
slug/title/subtitle (substring), `locale` filters by main
language, `sort` is `active` (default: last published post, then
creation), `new` (creation) or `followers`. Each entry is a blog plus
`post_count` (posts actually published), `follower_count` and
`last_published_at`, the counts of the directory cards (mockups 4a/4c).
`owner_id` is always `null` in this list (no authenticated viewer).

**`GET /api/v1/blogs/{slug}/overview`** — requires a session: owner or
member with any role (`403` otherwise, `404` if the blog does not exist).
Counts for the "Panoramica" (Overview) tab of the blog:

```json
{
  "posts_total": 12, "posts_published": 9, "posts_scheduled": 1, "posts_draft": 1, "posts_in_review": 1,
  "followers": 214, "members": 2, "pending_comments": 3, "approved_comments": 40, "media": 18,
  "last_published_at": "2026-09-12T07:00:00Z"
}
```

`posts_scheduled` are the `published` posts with a future `published_at`. `media` is the
number of images cited in the posts (table `post_media`). In addition (B2):
`reads_30d`: 30 entries `{day, reads}` (UTC days, those without reads at 0),
`reads_total_30d`, `storage_bytes`: bytes used on storage by the media and
Markdown backups of the blog (prefixes `userdata/{utente}/{blog}/` of
the owner and collaborators; `null` if the storage does not respond, `0` if the
bucket was never created), and `storage_limit_mb`, the limit set
by a Super Admin (`platform_config.max_blog_storage_mb`), `null` if there is no
limit.

**`POST /api/v1/posts/{post_id}/read`** — public, `204`, no body.
Aggregated read count per day (table `post_reads_daily`, mockup
5a). No cookie, no session, no IP or reader identifier is persisted, only `+1` on (post, UTC day). Sent by the browser
(`navigator.sendBeacon`) after a few seconds on the public page of the
post. A limit per IP+post in Redis (1 every 6 hours, volatile) avoids counting
close reloads: beyond the limit the request still returns
`204` without counting. `404` for posts not publicly visible (draft,
scheduled, hidden, blog not public or suspended).

**`GET /api/v1/blogs/mine`** — requires a session. Lists the blogs owned by the
user.

**`GET /api/v1/blogs/member-of`** — requires a session. Blogs of others on which
the user has an active membership (after accepting an invitation). Each entry:
`{"blog": <BlogOut>, "role": "co_autore"|"mediatore"|..., "author_display_name": str|null}`.

**`GET /api/v1/blogs/{slug}`** — optional token. Blog detail. It follows
the `visibility`: a `members`/`private` blog not visible to the
current user (or anonymous) returns `404` as if it did not exist. The same applies to
`GET .../config`, `GET .../categories` and to the endpoints that read the posts
of the blog (see the Post section). The `owner_id` field of the response is
`null` for anyone who is not the owner: it is the only field that
would point to the real id of the person managing the blog, and a blog that presents
itself with a `default_author_display_name` different from the owner's username
must not allow tracing back to it (see "User profile and follow" below).

**`PATCH /api/v1/blogs/{slug}`** — requires a session, owner only
(`403` otherwise). Updatable fields: `title`, `subtitle` / `description`
(`""` clears, absent leaves unchanged; `400` if over 64 / 256 characters),
`visibility` (`public` | `members` | `private`), `comments_mode`
(`everyone` | `members` | `closed`: default who can comment, see the Comments
section. `everyone` requires `NOCT_TURNSTILE_SITE_KEY`/`_SECRET_KEY`
configured on the instance, otherwise `400`), `mentions_enabled` (bool:
turns the `@username` in posts into links, see the Post section),
`search_indexing_enabled` / `ai_crawling_enabled` (bool, default `true`:
opt-out for search engine crawlers and, separately, for AI/LLM
crawlers. See `GET /api/v1/seo/crawl-directives` above. Excluding the blog
also excludes all its posts, regardless of any per-post override),
`default_author_display_name`: public name of the authors on the posts of
this blog. When set, it is **enforced** (no override per individual
author or post), unless the collaborator has
their own membership alias, which takes precedence. An empty string `""` clears it,
absent leaves it unchanged.
`is_paused` (bool, B3, mockup 5c "Pause this blog"): blog voluntarily paused.
Readers no longer see posts and pages. The detail
`GET /blogs/{slug}` stays readable with `is_paused=true` to show the
"paused" page. The blog disappears from feed, directory and sitemap. The owner and
collaborators keep working on it from the dashboard. Nothing is deleted.
`extra_locales` (list of ISO 639-1 codes, B3): secondary languages of the blog
in addition to `default_locale`, informational only (duplicates and the main language
discarded; `400` if a code is not valid).

The response of `GET /blogs/{slug}` also includes `is_suspended`,
`is_paused`, `deleted_at` and `extra_locales`. For a **public** blog
suspended by an admin or paused, the detail returns `200` with the flags (and
only those: posts, pages, bibliography stay `404`), so the public page
can explain the status (mockup 3d). A blog awaiting
deletion is `404` for anyone except the owner.

**`POST /api/v1/blogs/{slug}/transfer`** — `{username}`, owner only.
Transfers ownership to a **current co-author** (`400` if the
user has no `co_autore` membership, or has already reached the blog
limit; `404` if it does not exist). In `solo` the primary blog cannot be transferred: `400` with "Il blog primario non può essere trasferito." and the owner stays unchanged. The transferring owner remains as co-author. The membership of the
new owner is removed. Recorded in the audit log
(`blog.ownership_transferred`).

**`GET /api/v1/blogs/{slug}/export`**: owner only. Downloads a
ZIP (`application/zip`) with all the content of the blog: `blog.json`, one
Markdown file per post at `posts/{locale}-{slug}.md` (front matter with title,
slug, language, status, date, author, tags), `media.json` (URL and alt of the
cited images. The binaries stay on the storage) and `links.json`. The active modules
add their files: `pages/{locale}-{slug}.md` (static pages),
`comments.json` (all states), `categories.json` and `notes.json` (library
and cited notes). Generated on the fly, no expiring link.

**`DELETE /api/v1/blogs/{slug}`**: `{confirm_slug}`, owner only.
`400` if the slug does not match. Deletion with a grace period (B3, mockup
5c/3d): it sets `deleted_at`. The blog disappears from public pages, feed,
directory and sitemap (it remains in `GET /blogs/mine` with `deleted_at`
set). Its posts are no longer writable. After 30 days the maintenance job (`blogs.purge_deleted`, one run per day)
deletes it permanently, with posts, comments, fragments, pages, categories,
collaborators, followers, configuration and objects on storage. Idempotent. Audit `blog.deleted`.

**`POST /api/v1/blogs/{slug}/restore`**: owner only. Cancels the
deletion within the grace period (`deleted_at` returns to `null`).
Audit `blog.restored`.

**`POST /api/v1/blogs/{slug}/follow`** / **`DELETE .../follow`**: requires
a session. Follow/unfollow a blog. Idempotent (`204` even if already
in the requested state).

**`GET /api/v1/blogs/{slug}/followers`**: public. Lists the `{username}` of those
who follow the blog.

**`GET /api/v1/blogs/{slug}/config`**: public. Presentation configuration of the blog
(palette/typography/layout. See
[ROADMAP.md](../ROADMAP.md#2-aesthetics) for the constraints), applied by the frontend
to all public pages of the blog (`BlogPageShell`). The palette becomes CSS variables. `typography.heading_font`/`body_font`/
`monospace_font` become `--font-heading`/`--font-body`/`--font-monospace` (fonts
self-hosted at build time, no request to Google at runtime. The latter is
used for code blocks, the "syntax highlighting" block).
`body_size`/`measure` set the body size and the width of the reading column
of the post. `layout` sets the arrangement of the blog's home feed.
Free JSON. If the owner has not saved anything yet, it returns the
platform default (identical to the non-customized shell):

```json
{
  "palette": {"background": "#fbf9f6", "foreground": "#2b2a28", "primary": "#3e6259", "muted": "#a8a29a", "border": "#e7e2da"},
  "typography": {"heading_font": "Lora", "body_font": "Source Sans 3", "monospace_font": "JetBrains Mono"},
  "layout": "standard"
}
```

**`PUT /api/v1/blogs/{slug}/config`**: requires a session, owner only
(`403` otherwise). Replaces the whole configuration (it is not a
merge). Enforced rules (`400` otherwise). The rest of the structure (`layout`
and any other key) is free:

- `palette`: at most 5 colors. Each hexadecimal color cannot exceed
  90% HLS saturation (a "calm" palette, see ROADMAP.md §2).
- `palette_dark` (optional): same constraints as `palette`. It is the dark variant
  applied to the public pages of the blog when the reader uses the dark theme. If absent, the
  platform dark palette applies in dark theme.
- `typography`: at most 3 distinct fonts among `heading_font`/`body_font`/
  `monospace_font` (`body_size`/`measure`, although they are also strings, do not
  count toward this limit). If present, `heading_font` must be one of the
  curated serif fonts (`Lora`, `Merriweather`, `Playfair Display`,
  `Source Serif 4`, `Crimson Pro`), `body_font` one of the curated sans-serif fonts
  (`Inter`, `Nunito Sans`, `Work Sans`, `Source Sans 3`, `Karla`) and
  `monospace_font` one of the curated monospace fonts (`JetBrains Mono`,
  `Fira Code`, `IBM Plex Mono`, `Source Code Pro`, `Space Mono`; platform default
  `JetBrains Mono`).
  `body_size` (`"17"`/`"18"`/`"19"`) and `measure` (`"narrow"`/`"normal"`) are not
  validated on the backend (only accepted). A value other than the expected ones is
  ignored by the frontend, which falls back to the default.
- `footer` (optional): per-blog override of only columns 1 and 2 of the
  platform footer (`GET /api/v1/footer`): `{"column1": "...",
  "column2": "..."}`, free Markdown, max 5000 characters each, no other
  key allowed (`400` otherwise: it is not possible to override `column3` nor
  `bottom_bar`, which are always platform-only). Absent or empty: the blog inherits
  the platform default for both.

Other keys remain free.

**`POST /api/v1/blogs/{slug}/cover-image`**: requires a session, owner only
(`403` otherwise). `multipart/form-data`, field `file`.
Cover image of the blog (banner of the public home, optional). It has the same formats and size limit, and the same automatic moderation as
`POST .../media` below (including the check on the maximum space per blog,
`413` if exceeded. See `platform_config.max_blog_storage_mb`). The upload
updates `cover_image_url` and
`cover_image_is_sensitive` (moderation result), and clears
`cover_image_categories`/`cover_image_alt_text`. Replacing an existing cover
does not delete the previous object on storage (same choice as
`Post.cover_image_url`). Returns the updated `Blog` (`BlogOut`).
It also creates a row in the blog's media library (`GET .../media` below,
`used_as_blog_cover=true` as long as it remains the current cover). Before, it never
ended up there, unlike the cover of a post (which goes through the same
endpoint as `POST .../media`).


**`PATCH /api/v1/blogs/{slug}/cover-image`**: owner only, `400`
if the blog does not have a cover yet. `{"categories": ["nudity", ...],
"alt_text"?}` (category vocabulary: `nudity`, `explicit`, `other`). It updates
the manual warning on the content without re-uploading the image, following the same
principle as `PATCH /posts/{id}` when only `cover_image_categories` changes.
Non-empty categories force `cover_image_is_sensitive=true`. An absent `alt_text`
leaves it unchanged. A present one (including `null`/`""`) clears it or
replaces it. Returns the updated `Blog`.

**`DELETE /api/v1/blogs/{slug}/cover-image`**: owner only. Clears
cover, warning, categories and alt text (the object on storage is not deleted,
same choice as above). Returns the updated `Blog`.

**`POST /api/v1/blogs/{slug}/favicon`**: requires a session, owner only.
`multipart/form-data`, field `file`. Dedicated favicon of the blog
(optional, shown in the browser tab on its public pages).
PNG/JPEG/WEBP, max 512 KiB (`400` otherwise). **No automatic
moderation**, unlike cover/media: it is an identity icon, not
content, same principle as the user avatar (`POST /users/me/avatar`).
Replacing an existing favicon **deletes** the previous object on
storage (unlike the cover, here the object is small and dedicated, like
the avatar). Public avatar bucket, prefix `favicons/{blog_id}/`.
Returns the updated `Blog`.

**`DELETE /api/v1/blogs/{slug}/favicon`**: owner only. Deletes the
object on storage and clears `favicon_url`. Returns the updated `Blog`.

**`POST /api/v1/blogs/{slug}/media`**: requires a session and write access to the
blog (owner/author/co-author). `multipart/form-data`, field `file`.
Allowed formats: PNG, JPEG, WEBP, GIF. Max 10 MiB (`400`
otherwise). Image to embed in the Markdown of a post (e.g.
`![alt](url)`). See "Media and backups" below for the S3 path. If a Super
Admin has set a maximum space per blog
(`platform_config.max_blog_storage_mb`, `PATCH /admin/config`), an upload
that would exceed it returns `413` instead of being accepted. No
check (unchanged behavior) if the limit is not set.

```json
{"url": "https://.../notturni/userdata/{user_uuid}/{blog_uuid}/media/{uuid}-2400x1600.png"}
```

**Responsive variants.** For PNG, JPEG and WEBP (not for GIFs, which may be
animated), the upload, both `media` and `cover-image`, also generates
**WebP** versions at 100% (max 2560 px wide), 75%, 50% and 25%
of the original, discarding those below 320 px. The original stays as it is. The sizes
are in the name (`{uuid}-{W}x{H}.{ext}`), and a variant is
`{uuid}-{W}x{H}-{w}.webp`. The frontend computes the widths `w` from `W`
(`frontend/src/lib/responsive-image.ts`, same rule as
`crates/noct-media/src/variants.rs`) and derives `srcset`, `width` and `height`
without extra calls. Images uploaded before this feature, GIFs and
images from other sites have no variants and are served as before. Decoding
applies the EXIF orientation. The variants carry no metadata (the original does:
it is kept as uploaded). Images over 40 megapixels → `400`. The space
used by the variants counts toward the blog quota, and `DELETE
.../media/{id}` deletes them together with the original.

**`GET /api/v1/blogs/{slug}/mentionable-users?q=<prefix>&limit=8`**:
requires a session and write access to the blog. Suggestions for
the `@mention` autocomplete in the editor: the owner, collaborators and
followers of the blog whose username starts with `q` or whose public name
contains it (`q` empty → first results by username). `limit` 1–25 (default
8). Returns `[{"username": "...", "display_name": str|null, "avatar_url":
str|null}]`. If the blog has `mentions_enabled=false`, it always returns `[]`.

**`GET /api/v1/blogs/{slug}/bibliography`**: optional token, follows the
`visibility` of the blog (`404` if not visible). Automatic bibliography:
all the footnotes of the **published** posts,
grouped by identical text (comparison ignoring case and spacing) and sorted by the recency of the first post that cites them:

```json
[
  {
    "content": "Text of the note, inline Markdown.",
    "citations": [
      {"post_title": "...", "post_slug": "...", "permalink": "/{blog}/{slug}", "locale": "it", "idx": 1}
    ]
  }
]
```

**`GET /api/v1/blogs/{slug}/media-bibliography`**: same authorization and
same principle as the bibliography above, for the media (today only images)
cited in the body of the **published** posts, grouped by identical URL:

```json
[
  {
    "url": "https://.../media/....jpg",
    "alt_text": "Description of the image",
    "categories": ["nudity", "explicit"],
    "is_sensitive": true,
    "citations": [
      {"post_title": "...", "post_slug": "...", "permalink": "/{blog}/{slug}", "locale": "it", "used_at": "2026-01-01T00:00:00Z"}
    ]
  }
]
```

`categories` is the subset of `suggestive`/`nudity`/`explicit`/`other`
chosen by the author for that image (see "Content warning" in the
Post section). It is empty if not flagged, *or* if flagged (by automoderation alone, or by the modal without a specific
category) without a category. For this reason `is_sensitive` is a separate field, not derivable from
`categories.length > 0`. It decides whether the image is shown blurred
in the grid, using the same flag as the rendering of the post. `used_at` is the publication date
of the post that cites it.

**`GET /api/v1/blogs/{slug}/links-bibliography`**: same principle, for the
links cited in the body of the published posts:

```json
[
  {
    "url": "https://esempio.org/articolo",
    "link_text": "testo del link",
    "citations": [
      {"post_title": "...", "post_slug": "...", "permalink": "/{blog}/{slug}", "locale": "it", "used_at": "2026-01-01T00:00:00Z"}
    ]
  }
]
```

The data of both endpoints come from the same source of truth as the
Markdown content of the post (tables `post_media`/`post_links`, rewritten
in full on every save, like `post_tags`/`post_notes`. Not a
list supplied separately by the client).

### Categories

Blog taxonomy. Unlike tags (free, up to 5 per
post, see the "Tags" section), categories are defined in advance by the
owner/authors, and a post has **at most one**.

**`GET /api/v1/blogs/{slug}/categories`**: public. List, sorted by
name.

**`POST /api/v1/blogs/{slug}/categories`**: requires a session and write access
to the blog. `{"name": "Viaggi", "slug": "viaggi"}` → `201`. `slug`:
lowercase, letters/digits/single hyphens, max 60 characters. `400` if the
format is not valid, `409` if the slug is already in use on that blog.

**`PATCH /api/v1/blogs/{slug}/categories/{category_id}`**: same
authorization. Updates `name`/`slug` (both optional). Same
validation and uniqueness rules as creation.

**`DELETE /api/v1/blogs/{slug}/categories/{category_id}`**: same
authorization. The posts with this category are not deleted. They remain
with no category.

### Static pages of the blog

Opt-in feature per blog (`Blog.static_pages_enabled`, **off by
default**. Unlike platform pages, which are always active, see the
section "Static pages (main site)"), intended for pages such as "About me"/"Contact me" of the individual blog. It uses the same translation schema as posts
(`translation_group_id` + `locale` + `slug`), but **no tags, categories or
workflow publication status**: only the boolean `is_published`. Public permalink
`/{blog_slug}/pagina/{slug}` (no date, unlike posts).

**`GET /api/v1/blogs/{slug}/pages?locale=it`**: list. Public: only
pages with `is_published=true`. Those with write access to the blog also see
the drafts.

**`GET /api/v1/blogs/{slug}/pages/{page_slug}?locale=it`**: resolution of the
public permalink. Same published/draft distinction as above. `404` if
not found.

**`GET /api/v1/blogs/{slug}/pages/by-id/{page_id}`**: requires write access
to the blog. Retrieves a page by id (draft included), for the
dashboard editor. Unlike the slug route above, which is meant for
resolving the public permalink.

All responses (`PageOut`) include `permalink` (computed:
`/{blog_slug}/pagina/{slug}` for blog pages or `/p/{slug}` for
platform ones) and `mentions_enabled` (mirror of `Blog.mentions_enabled`,
always `true` for platform pages), useful for public rendering
on the frontend without a separate fetch of the blog.

**`POST /api/v1/blogs/{slug}/pages`**: requires write access to the
blog (owner or `autore`/`co_autore` membership) **and**
`static_pages_enabled=true` on the blog (`403` otherwise, with an explicit message).
Creates a page, the root of a new translation family:

```json
{"slug": "contattami", "locale": "it", "title": "Contattami", "content": "...", "is_published": true}
```

`400` if the slug is not in the valid format (lowercase, letters/digits/single hyphens,
max 80 characters) or the language is not supported. `409` if a page
with that slug already exists in that language on this blog.

**`POST /api/v1/blogs/{slug}/pages/{page_id}/translations`**: same
authorization (including `static_pages_enabled`). Adds a translation to the
same family (same body as `POST .../pages`, `locale` always explicit).

**`GET /api/v1/blogs/{slug}/pages/{page_id}/translations`**: public.
Lists `{id, locale, slug, is_published}` of only the published translations
of the same family (same pattern as `GET /api/v1/posts/{post_id}/translations`
for posts). It is for the language selector on the frontend, which always shows
the current page separately.

**`PATCH /api/v1/blogs/{slug}/pages/{page_id}`**: requires write access
to the blog (it does **not** require `static_pages_enabled`: editing or
disabling pages that already exist is always possible, even with the feature
turned off). Updates `slug`/`title`/`content`/`is_published`, all
optional.

**`DELETE /api/v1/blogs/{slug}/pages/{page_id}`**: same authorization as
`PATCH` (no check on `static_pages_enabled`). `204`.

### Collaborators and invitations

The owner can invite other registered users as **co-author** or
**moderator**. The invitation stays `pending` until the invitee
accepts it from their own dashboard. The `BlogMembership` is created only on acceptance. There is one invitation row per (blog, user). A new invitation
after a refusal/revocation reuses the same row.

Owner side (all `403` if you are not the owner of the blog):

- **`GET /api/v1/blogs/{slug}/members`**: collaborators of the blog:
  `[{user_id, username, avatar_url, role, author_display_name, created_at}]`.
- **`PATCH /api/v1/blogs/{slug}/members/{user_id}`**: `{"role": "co_autore"|"mediatore"}`
  (`400` for other roles). `404` if not a collaborator.
- **`DELETE /api/v1/blogs/{slug}/members/{user_id}`**: removes the membership
  (`204`, idempotent).
- **`GET /api/v1/blogs/{slug}/invitations`**: all invitations of the blog
  (any status).
- **`POST /api/v1/blogs/{slug}/invitations`**: `{"username": "...", "role": "co_autore"|"mediatore"}`
  → `201`. `400` if the role is not allowed or you invite the owner yourself. `404`
  if the user does not exist. `409` if already a collaborator or already has a `pending` invitation.
- **`DELETE /api/v1/blogs/{slug}/invitations/{invitation_id}`**: revokes a
  `pending` invitation (`204`).

Collaborator side:

- **`PATCH /api/v1/blogs/{slug}/my-membership`**: `{"author_display_name": "..."}`
  (`""` clears it): the alias with which the collaborator signs the posts on this blog.
  `404` if you are not a collaborator.

Invitee side (on the current user, path without slug to avoid a clash with
`GET /blogs/{slug}`):

- **`GET /api/v1/blogs/received-invitations`**: received invitations still
  `pending`: `[{id, blog_slug, blog_title, role, status, invited_username,
  invited_by_username, created_at, responded_at}]`.
- **`POST /api/v1/blogs/received-invitations/{invitation_id}/accept`**: creates
  the membership with the role of the invitation, and marks it `accepted`. `404` if it is not yours,
  `409` if it is no longer `pending`.
- **`POST /api/v1/blogs/received-invitations/{invitation_id}/decline`**:
  marks it `declined`.

## Solo mode: primary blog and personal-site routes

In `solo` mode (`NOCT_DEPLOYMENT_MODE=solo`) the instance is one personal site. The
behaviour below applies only in `solo`; `platform` is unchanged.

**Primary blog.** The site has one primary blog, marked in `platform_config.primary_blog_id`
(no foreign key: the platform migration runs before the blog tables exist). The backend
creates it in an idempotent way:

- at the first registration (the first user becomes Super Admin and owns the blog), in the
  same transaction as the account;
- at startup, for the oldest active Super Admin, which also covers `NOCT_SUPER_ADMIN_*` and
  installations that have no blog yet.

Title: `NOCT_SITE_NAME` if set and not blank, otherwise the owner's username. Slug: the title
slugified (lowercase ASCII letters, digits and hyphens, at most 55 characters before the
suffix), then `-2`, `-3`, ... on a collision with an existing slug or a site route (see the
slug rules below). The creation writes the audit event `blog.primary_created` (actor
`system`). The primary blog is public and uses the default locale of the platform.

**Space limit.** In `solo` the primary blog does not count toward `max_blogs_per_user` (default 5): the owner can create that many other Spaces in addition to it. The exclusion applies to creation and to ownership transfer. In `platform` every Space counts, also when a `primary_blog_id` value is left over from a switch from `solo`.

The marker counts only while the blog row exists. A blog in the deletion grace period
(`deleted_at` set) still exists and is restored with `POST /blogs/{slug}/restore`; a
blog removed by the daily `blogs.purge_deleted` job is recreated at the next backend
restart (a running instance does not recreate it).

**`GET /api/v1/config`** exposes `primary_blog_slug`: the slug of the primary blog, or `null`
when there is none or while it is in the grace period. Only `solo` creates a primary blog; an
instance that was switched from `solo` to `platform` keeps the marker and the field.

**Routes (frontend).** Posts of the primary blog have the permalink `/post/<post-slug>`.
Posts of every other blog keep `/<blog-slug>/<post-slug>`. The `permalink` field of
`PostOut`, the sitemap entries and the crawl directives follow this rule. Other paths:

| Path | Result |
|---|---|
| `/` | Home of the primary blog (same output as its blog home) |
| `/post/<post-slug>` | Post of the primary blog (404 in `platform`) |
| `/<primary-slug>` | `308` to `/` (the query string is kept; the page does this, not `proxy.ts`) |
| `/<primary-slug>/<post-slug>` | `308` to `/post/<post-slug>` |
| `/<primary-slug>/<sub-page>` and `/<primary-slug>/feed.xml`, `/<primary-slug>/atom.xml` | `308` to the same path without `/<primary-slug>` (for example `/<primary-slug>/pagina/x` to `/pagina/x`). The query string is kept. |
| `/pagina/<slug>`, `/pub/<name>` | Primary blog page or publication, served at the root (the URL does not change) |
| `/bibliografia`, `/media`, `/link`, `/search` | Primary blog page, served at the root (the URL does not change) |
| `/feed.xml`, `/atom.xml` | Feed of the primary blog |

The sub-pages listed in the last two rows are root paths. `proxy.ts` rewrites them to the
primary blog's route, so the address stays the root one. A `308` target is always one of
these fixed root paths, so a redirect cannot leave the site.

**Scope of the root routing.** With `NOCT_SUBDOMAINS_ENABLED=true` the root routing applies to
the apex host (`NOCT_INSTANCE_FQDN` / site host) only. A `<slug>.<domain>` host is served as a
normal Space. With `NOCT_SUBDOMAINS_ENABLED=false` it applies to every host.

**Upgrade note.** Links and bookmarks to the old `/<primary-slug>/...` sub-page addresses still
work: they answer `308` to the root address. Search engines and feed readers follow the
redirect. Other modules still build `/<blog>/<post>` links by hand (see the known limit above).
Route collisions are only warned, never fixed automatically: if an existing Space has a slug
that is now a site route (see the list below), its address `/<slug>` is shadowed by the site
route, the backend logs one warning line with the colliding slugs at startup, and the Space is
neither renamed nor deleted. The owner has to rename it. Creating a new Space with such a slug
returns `400` (see the slug rules below).

Spaces other than the primary one are reached by path only, `/<slug>/`. In `solo` the
subdomain addresses are off (`NOCT_SUBDOMAINS_ENABLED=false`).

**Robots and sitemaps.** When the primary blog opts out of search or AI crawling, its
`Disallow` lines list its root routes one by one: the home (`/$`, `/?`), `/post/`, `/pagina/`,
`/pub/`, and the exact pages `/pub`, `/bibliografia`, `/link`, `/media`, `/search`, `/feed.xml`,
`/atom.xml`. The `$` and `?` suffixes keep other Spaces with a similar slug, such as `/media-lab`,
unblocked. It never emits `Disallow: /`, which would close the whole site. Sitemaps (global and per-Space) list the primary blog's
`/pagina/...` and `/pub/...` entries at their root paths, for example `/pagina/<slug>`.
Known limits: the global sitemap still lists `/` when the primary blog disallows search, and
the `Disallow` rules of other Spaces and of single posts are plain prefixes.

**Known limit.** Other modules (notes, fragments, publications, comments, newsletter, pages,
`noct-media`) still build `/{blog}/{post}` links by hand. Those links reach the post only
through the `308` redirect. See the postponed requests in [ROADMAP.md](../ROADMAP.md#8-postponed-requests).

**Slug rules for `POST /api/v1/blogs` in `solo`.** The slug must match the usual format
(lowercase letters, digits and hyphens, at most 63 characters) and must not equal a
site route: `admin`, `api`, `bibliografia`, `blogs`, `dashboard`, `forgot-password`, `link`, `login`,
`media`, `newsletter`, `p`, `pagina`, `post`, `pub`, `register`, `search`, `sitemaps`, `tag`, `u`, `users`,
`_next`, `storage`. The primary blog's sub-page names (`bibliografia`, `link`, `media`, `pagina`, `pub`,
`search`) are in this list, so a Space cannot take them.
There is no minimum length and no list of reserved names. A failed check returns `400`.
In `platform`, the rules are the ones described in the Blog section (minimum of 4 characters,
platform blacklist).

## Post

The content (`content`) is **Markdown**. The backend does not render it. The
frontend converts it to HTML (with sanitization) when the post is read.

**`@username` mentions**: in the text, an `@` at the start of a line, or
preceded by a space, followed by a valid username (lowercase letters/digits,
inner `-`/`_`), is a mention. The backend does not process it. It stays
`@username` in the Markdown. The frontend turns it into a link to the profile
`/u/{username}` at render time, **only if** the blog has
`mentions_enabled=true` (default; see the Blog section). `PostOut` returns the
blog's `mentions_enabled` for the client's convenience. The endpoint
`GET /blogs/{slug}/mentionable-users` feeds the editor's autocomplete.

**Footnotes**: these are a structured list of the post (`notes`:
`[{"idx": 1, "content": "inline Markdown text"}]`), **not in the body**. `idx`
is the note number (1-999). In the post `content`, the reference is the link
marker `[idx](#nota-idx)`. The editor inserts it as a real link node, so it
survives the round trip of its serializer. The text form `[^idx]` is also
accepted for API writers. Duplicate `idx`, empty notes, or notes longer than
2000 characters return `400`. `PostOut.notes` returns them sorted by `idx`.
Rendering (numbered footnote list plus a tooltip on the marker) is up to the
frontend. The blog-level aggregate is `GET /blogs/{slug}/bibliography` (see
the Blog section).

Each note also accepts 9 optional fields for structured bibliographies that
are compatible with BibTeX: `title`, `author`, `kind` (`"book" | "article" |
"web" | "note"`; any other value returns `400`), `source` (max 300;
publisher, journal, or site), `issued` (max 32; year or date as a free
string, not a date type, because not all sources have a year or a full ISO
date), `isbn` (max 32), `doi` (max 255), `url` (max 2000), `page` (max 32).
All are `string | null`. Missing or empty values are equivalent to `null`. In
the editor, these fields sit behind a toggle labeled "Aggiungi dettagli bibliografici"
(Add bibliographic details) in the "Nota" (Note) modal. The toggle uses the same
style as the sensitive-content warning for images.
`title` and `author` are max 300 characters. None of these is ever required;
only `content` is. On **creation only**, the fields also propagate to the blog
library note that this note links to (B8, below). If the library already has a
note with the same normalized text, its fields are never overwritten from
here. This preserves any change made in the library itself. `kind` and `url`
set directly here take precedence over the `guess_kind()` and `extract_url()`
heuristics. Those heuristics are the fallback when the fields are absent.
`GET /blogs/{slug}/bibliography` returns these same 9 fields for each
aggregated entry.

States: `draft` → (optional) `pending_review` → `published`. `published_at` is
also used for scheduling. A post with `status=published` and a `published_at`
in the future is not yet publicly visible. See `is_publicly_visible` below.

`is_hidden` (`PostOut`): moderation by a platform admin (`admin/moderazione`,
`PATCH /api/v1/admin/posts/{id}`). If `true`, the post is unreachable
publicly, whatever its `status`. This includes the author. Anyone without
write access gets the same `404` as for a nonexistent post.
The author can never set it.

Each post has a **readable permalink** without the UUID. `blog_slug` and
`permalink` are computed on every response (they are not model columns) and
are included in every `PostOut`:

```text
permalink = /{blog_slug}/{slug}
```

The date is the publication date (`published_at`) if the post is published.
Otherwise it is the creation date (`created_at`). This still allows a preview
link for a draft. Only users with write access to the blog can resolve that
link. The date does not guarantee uniqueness. Uniqueness is already given by
`blog_id` + `slug` + `locale` (see above). The date is only a readability
convention, in WordPress style.

**`POST /api/v1/blogs/{blog_slug}/posts`**: requires a session and either
ownership of the blog or a membership with the `autore` or `co_autore` role
(otherwise `403`). Creates a post in `draft` state. The post is the root of a
new translation "family" (`translation_group_id` = a new UUID). It also
queues a backup to S3 (see the "Media and backup" section below).

```json
{"slug": "primo-post", "title": "...", "content": "# Markdown...", "locale": null, "cover_image_url": null, "tags": null, "category_id": null, "notes": null}
```

The client does **not** set the public author name. It is computed as the
first applicable value in this order:

1. The collaborator's alias on *this* blog (`PATCH /blogs/{slug}/my-membership`).
2. The blog's `default_author_display_name`.
   If either of these two exists, it is **enforced**, with no possible
   override.
3. Otherwise, the profile preference `post_author_name_style` (see the Users
   section): `full_name` (first and last name), `display_name` (global alias),
   `verified_domain` (verified custom domain), or `username` (default).

`PostOut.author_avatar_url` is always the avatar of the real author (`null`
if not set). It is independent of the name shown above, which can be an
alias. A post has no replacement "blog avatar".

The value is saved in `PostOut.author_display_name` when the post is written.
It is **recomputed on every read**. Changing the blog or membership alias, or
renaming the user, applies at once to all existing posts, not only to posts
saved again. If the post is saved again (`PATCH /posts/{id}`), the author
also realigns the saved column. Another user who edits the post (for example
a reviewer) does not change it. This does not affect the response, which is
recomputed anyway.

`locale` is optional. If omitted, the blog's `default_locale` is used.
`cover_image_url` is optional. It is the URL returned by a previous upload to
`POST /blogs/{slug}/media` (see below). The field accepts any string. It does
not check that the URL points to a media item uploaded to this blog.
`cover_image_is_sensitive` (default `false`) takes the result of the automatic
moderation received in that same upload response. See the "Automatic image
moderation" section below. It is not recomputed here.
`cover_image_categories` (default `[]`) are the content-warning categories
chosen manually by the author (see "Content warning" below). A non-empty list
also forces `cover_image_is_sensitive` to `true`, whatever value was sent for
that field.
`cover_image_alt_text` (default `""`) is the alternative text of the cover
(accessibility). It is independent of any alt text on the same image in the
media library. `tags` is optional (see "Tags" below).
`category_id` is optional. It is the UUID of an existing category of the blog
(see the "Categories" section above). It returns `404` if the category does
not belong to this blog. It returns `409` if the slug is already used on that
blog for that language. `notes` is optional (see "Footnotes" above).

### Tags

A post can have at most **5 tags**. Tags come from the dedicated field (`tags`
in the create/update payload) or from `#hashtag`s written in the post text.
Both sources are merged into the same set, with duplicates removed. A tag is
normalized: lowercase, no `#`, only letters/digits and single hyphens, max 30
characters. `"#Poesia"` and `" poesia "` both become `"poesia"`. A malformed
tag in the dedicated field is an explicit error (`400`). A malformed hashtag in
the free text is silently ignored and does not make the save fail. Markdown
link anchors (`[1](#nota-1)`, the footnote markers) are not hashtags either.
Exceeding the limit of 5 (dedicated and text combined) is always an explicit
error (`400`), never a silent truncation.

Each `PostOut` exposes two fields:

- `manual_tags`: only the tags entered in the dedicated field (to show them
  again in an edit form).
- `tags`: the effective set (`manual_tags` + hashtags in the text). This is the
  set to show when reading. It is also used by the `?tag=` feed filter and for
  the trending section (see the "Feed" section below).

Each `PostOut` also exposes `category`: `null`, or
`{"id", "name", "slug"}` of the blog category assigned to the post (see the
"Categories" section above). Unlike tags, a post has at most one category.

In `PATCH /posts/{post_id}` (see below): an absent `tags` field leaves the
dedicated tags unchanged. A list (even an empty `[]`) replaces them. Hashtags
in the text are instead recomputed **on every content change**, whatever this
field contains.

**`POST /api/v1/posts/{post_id}/translations`**: same authorization as
creation. Adds a translation to the same family as the given post
(shared `translation_group_id`). The translation is an independent `draft`
post with its own slug:

```json
{"slug": "my-post", "locale": "en", "title": "...", "content": "...", "category_id": null, "notes": null}
```

It returns `409` if a translation for that language already exists in the
family, or if the slug is already used on that blog for that language.
`category_id` is optional. If omitted, the translation inherits the category
of the original post. If passed explicitly (even `null`, to assign none), it
overrides the inheritance. `notes` is optional and is **not** inherited from
the original. A translation has its own notes.

**`GET /api/v1/posts/{post_id}/translations`**: public. Lists `{id, locale,
slug, status}` for all **publicly visible** translations in the same family.
It is meant for building a language selector in the frontend.

**`GET /api/v1/blogs/{blog_slug}/posts`**: public (optional token). Without a
session, or without write access to the blog, it returns only publicly
visible posts (published and not scheduled in the future). With a session and
write access (owner, author, or co-author), it returns all posts, including
drafts, posts under review, and scheduled posts. The author dashboard needs
this. Optional query parameter `?locale=it` filters a single language. If
omitted, all languages are returned. It also respects the blog's `visibility`:
`404` if the caller cannot see a `members` or `private` blog (see the Blog
section). The same rule applies to `GET /posts/{post_id}` and to the permalink
below.

**`GET /api/v1/posts/{post_id}`**: public if the post is publicly visible.
Otherwise it requires write access to the blog (same rule as creation). If not
authorized, it returns `404` (not `403`, so the existence of the post is not
revealed). Internal use (dashboard and editor): it identifies the post by
UUID, not by its public permalink.

**`GET /api/v1/blogs/{blog_slug}/posts/{post_slug}`**: public (optional
token), with the same visibility rules as `GET /posts/{post_id}` above. It
resolves the readable permalink (see above) to the post. It is the endpoint
for the public post page (for example
`https://notturni.eu/{blog_slug}/{post_slug}`), so the UUID never has to appear
in the URL. It returns `404` if the blog and slug match no post, or a post not
visible to the caller.

**`PATCH /api/v1/posts/{post_id}`**: same authorization as creation. Updates
`title`/`content`/`cover_image_url`/`cover_image_is_sensitive`/`cover_image_categories`/`cover_image_alt_text`/`tags`/`category_id`/`notes`
(all optional).

- `cover_image_alt_text` is independent of `cover_image_url`, the same
  principle as `cover_image_categories` below. An absent field leaves the alt
  text unchanged. A present field (even `null` or `""`) clears it or replaces
  it.
- If `content` changes, the S3 backup is queued again. The cited media and
  links are also recomputed (see "Content warning" and "Media and cited links"
  below).
- `notes`: an absent field leaves the notes unchanged. A list (even an empty
  `[]`) replaces them.
- `cover_image_url`: an absent value (`null` or omitted field) leaves the cover
  unchanged. An empty string `""` removes it. This also clears
  `cover_image_is_sensitive` and `cover_image_categories`, whatever is sent for
  those fields. Any other value replaces the cover together with
  `cover_image_categories` in the same request. If that field is absent in this
  case, it becomes `[]`. This matches `cover_image_is_sensitive`: a new cover
  always starts without a warning unless the author states one explicitly.
- `cover_image_categories` **without** a new `cover_image_url` in the same
  request: unlike `cover_image_is_sensitive`, which stays unchanged in this
  case, an absent field leaves the categories of an existing cover unchanged.
  A list (even an empty one) replaces them. A non-empty list also forces
  `cover_image_is_sensitive` to `true`. This lets the content-warning modal
  update a cover that is already uploaded, without uploading it again.
- `tags`: see the "Tags" section above.
- `category_id`: an absent field leaves the category unchanged. An explicit
  `null` removes it. A valid UUID replaces it (`404` if it does not belong to
  this blog). Unlike `cover_image_url`, a UUID has no "empty" value. That is why
  absent, `null`, and value are distinct cases.

It also accepts `search_indexing_enabled` and `ai_crawling_enabled` (booleans
with three states, like `comments_mode`). An absent field changes nothing. An
explicit `null` returns to inheriting from `Blog.search_indexing_enabled` or
`Blog.ai_crawling_enabled`. A value sets an override for this post only. The
override cannot reopen a crawler that the blog has already excluded. It can
only restrict it further. `PostOut` returns both the raw value (`null` means
inherit) and `effective_search_indexing_enabled` and
`effective_ai_crawling_enabled`. The effective fields are always set. They
already account for the cascading block.

### Content warning

In addition to the automatic flag (see "Automatic image moderation" below),
the author can set a warning on images manually, using a modal in the style of
Bluesky. The categories are **Suggestivo** (Suggestive, `suggestive`),
**Nudità** (Nudity, `nudity`), **Esplicito** (Explicit, `explicit`), and
**Contenuto sensibile** (Sensitive content, `other`). These labels are UI strings. For
images in the post body, the choice travels in the same Markdown `title` already
used for "sensitive" (see below). Use `![alt](url "sensitive")` for the automatic
flag alone (category unknown). Use `![alt](url "sensitive:nudity,explicit")`
for an explicit choice. There is no dedicated field. The editor rewrites the
image `title` directly. Selecting zero categories in the modal removes the
warning completely, even if automatic moderation set it. The author always has
the final choice. For the cover, see `cover_image_categories` above.

### Media and cited links

Like tags, media (currently images only) and links in the post body are
extracted from the Markdown and stored in read-only tables (`post_media` and
`post_links`) on every save. The client does not send them separately. They feed
the blog's aggregated bibliography. See `GET /api/v1/blogs/{slug}/media-bibliography`
and `.../links-bibliography` in the Blog section.

### Image gallery (`gallery` fence)

A gallery groups up to 10 images in one block of the post `content`. It is a
fenced block whose info string is exactly `gallery`, with one standard
image line per item:

````markdown
```gallery
![alt one](https://.../a.jpg)
![alt two](https://.../b.jpg "sensitive:nudity")
```
````

Rules:

- The info string is `gallery` (trailing spaces allowed). Any other info
  string is an ordinary code block. The block closes at the next line that
  starts with three backticks. A block without a closing line runs to the end
  of the content.
- Each image line uses the same grammar as any Markdown image in a post
  (see "Content warning" above for the `sensitive` titles).
- Lines that are not image lines are ignored by the frontend renderer. The
  backend counts only image lines.
- **More than 10 image lines in one `gallery` block → `400`** on `POST
  /api/v1/blogs/{slug}/posts` and on `PATCH /api/v1/posts/{post_id}` (when
  `content` is sent). The message is in Italian and names the limit. The
  rule applies to drafts too. Other fences are not checked.
- A gallery with one image is stored and rendered as a plain image line,
  without the fence. The editor does this on save.
- The editor drops `[` and `]` from alt text on save, because the image
  grammar does not accept them in alt text.
- Images inside a gallery are media like any other image: they feed
  `post_media` and the usage check of `DELETE` on media.
- Upload: there is no gallery endpoint. The client sends one
  `POST /api/v1/blogs/{slug}/media` per file (see "Media and backups"). Quota,
  moderation and the responsive variants apply per image.

Layout (frontend only): 1 image is shown full width; 2 side by side; 3 with
one large image on top and two below; 4 or more with one large image on top
and three small ones below. With more than 4 images, the fourth small tile
shows "+N", where N is the number of images beyond four. All images stay
reachable in the lightbox.

**`POST /api/v1/posts/{post_id}/submit-for-review`**: owner, author, or
co-author. Moves a post from `draft` to `pending_review`. Returns `400` if the
post was not in `draft`.

**`POST /api/v1/posts/{post_id}/return-to-draft`**: owner or **reviewer** (a
blog role; see [ROADMAP.md](../ROADMAP.md#1-product-and-domain-rules)). Sends a
post back from `pending_review` to `draft` (rejects the review). Returns `400`
if the post was not in `pending_review`, and `403` if the caller is neither
owner nor reviewer.

**`POST /api/v1/posts/{post_id}/unpublish`**: owner, author, or co-author.
Takes a `published` post offline, including a scheduled one. It returns to
`draft` with `published_at = null`. Returns `400` if the post was not
published, and `403` without write access to the blog. Publishes the event
`posts.post.unpublished` and invalidates the public cache.

**`DELETE /api/v1/posts/{post_id}`**: same authorization. Deletion is
**permanent** and returns `204`. Comments, notes, fragments, tags, cited
media and links, and publication records of the post are removed by cascade
(FK `on_delete = cascade`). The other translations in the family remain.
Publishes `posts.post.deleted`. The Markdown copy already saved to S3 by the
backup is not touched by this call.

**`POST /api/v1/posts/{post_id}/publish`**: publishes now, or schedules the
post if a future date is passed:

```json
{"published_at": "2026-09-01T10:00:00Z"}
```

The body is entirely optional. Without a payload, the post is published
immediately. Allowed for owner, author, or co-author from **any** state. A
**reviewer** can approve only from `pending_review` (otherwise `403`). This is
how a reviewer approves a review, instead of rejecting it with
`return-to-draft`. It is idempotent if the post is already published and no new
`published_at` is passed. An existing `published_at` is not touched. Passing a
new date always overwrites it, including to reschedule an already published
post.

## Wiki and documentation spaces (`wiki` module)

**Experimental feature, opt-in.** Wiki and docs are off by default. Enable them
with `NOCT_EXPERIMENTAL_FEATURES=wiki,docs` (ConfigMap in Kubernetes). When they
are off, `POST /blogs` with `kind` `wiki` or `docs`, and `PATCH` with
`display_mode` `wiki` or `docs`, return `400` ("La funzione sperimentale … non è
attiva": "The experimental feature … is not enabled"). `GET /config` exposes `experimental_features`, and the frontend
offers only the active types. Turning them off later does not delete or hide
the spaces already created.

These rules apply to Spaces with `kind` = `wiki` or `docs` (see `POST /blogs`).
For `blog` Spaces the module does nothing. A non-null `parent_id` or a
`position` on posts is rejected (`400`). No revision and no internal link is
recorded.

**Post fields** (extension of `POST /blogs/{slug}/posts`, `PATCH /posts/{id}`, and
of the responses):

- Input: `parent_id` (UUID of a post in the same Space; `null` means root; absent
  means unchanged), `position` (integer ≥ 0 among siblings; if missing, a new or
  moved page goes to the end), and `revision_summary` (note of max 300 characters
  about the revision created by the save).
- Output: `parent_id`, `position`, and `revisions` (number of stored revisions).

A cycle, a parent from another Space, or more than 8 levels of subpages returns
`400`. When a post is deleted, its children become roots.

**`GET /api/v1/blogs/{slug}/outline`**: public (`?locale=` optional). A flat list
in reading order (depth-first): `{id, slug, title, locale, status, parent_id,
position, depth, is_visible}`. Users who can write see all posts. Others see only
published posts. A page whose parent is not visible moves up to the root. Each
translation is a node of its own.

**`PUT /api/v1/blogs/{slug}/outline`**: requires a session with write access. Body:
`{"items": [{"id": "...", "parent_id": null}, ...]}`. The order of the list sets
the position among siblings. The list must contain **all** posts of the Space,
each exactly once. Otherwise it returns `409` (the index has changed; reload).
Cycles or excessive depth return `400`. On a `blog` Space it returns `400`. It
returns the updated index and records `wiki.outline_updated` in the audit log.

**Revisions**: only for users who can write in the Space (otherwise `401` or
`403`). Every save that changes the title or the content (including creation)
stores a snapshot. If the text is identical to the last revision, no new revision
is created. At most 200 revisions are kept per post. A restore is a normal
`PATCH` with the title and content of the chosen revision.

- `GET /api/v1/posts/{id}/revisions`: newest first. Returns `{revision, title,
  editor, summary, size, created_at}`. `editor` is the membership alias of the
  author of the change, otherwise the username.
- `GET /api/v1/posts/{id}/revisions/{n}`: full revision, with `content`.
- `GET /api/v1/posts/{id}/revisions/{n}/diff?against=m`: line-based differences.
  `against` defaults to `n-1`. For revision 1, the comparison is with an empty
  text. Returns `{from, to, hunks: [{op: "equal"|"insert"|"delete", text}]}`.

**Internal links**: `[[page]]` and `[[page|text]]` in the Markdown (ignored in code)
point to a post in the same Space by its slug, normalized (lowercase, no accents,
hyphens). They are resolved at read time. A link to a page created later works
without any further step. The frontend renders them as links.

- `GET /api/v1/posts/{id}/backlinks`: public. Returns `{id, slug, title, locale}`
  of the pages that link to this one (`404` if the post is not visible). Readers
  see only published pages.

## Git sync (`git_sync` module)

Exports the texts of a Space, as Markdown, to a branch of a remote Git repository
(GitHub, GitLab, Forgejo, Gitea, and others). Only the **owner** of the Space can
manage it (otherwise `401` or `403`). It is export only. No change made in the
repository flows back to the platform.

- **`GET /api/v1/blogs/{slug}/git-sync`** → `{configured, remote_url, branch,
  username, auth_method, has_secret, ssh_public_key, ssh_host_fingerprints,
  include_drafts, enabled, last_status, last_run_at, last_error, last_commit,
  last_pages}`. A token or a private key is never returned. `has_secret` only
  says whether a credential exists. `last_status` values: `running`, `ok` (commit
  pushed), `unchanged` (nothing new, no network access), and `error` (details in
  `last_error`).
- **`PUT /api/v1/blogs/{slug}/git-sync`** → `{remote_url, auth_method?, branch?,
  username?, secret?, regenerate_key?, host_fingerprint?, include_drafts?,
  enabled?}`. There are two access methods: `auth_method` = `https` (default) or
  `ssh`. If absent, the value is unchanged. `branch` (default `notturni-backup`)
  follows the Git branch-name rules. If `branch` or `username` is absent, it stays
  unchanged. Changing the repository, access method, branch, user, credential,
  host key, or `include_drafts` resets the last result and makes everything run
  again. The repository host must resolve only to public addresses. Loopback,
  private networks, link-local addresses, and cloud metadata addresses return
  `400`.
  - **`https`**: `remote_url` is `https://` with no embedded credentials. `secret`
    is the **personal token** of the provider (GitHub, GitLab, Forgejo, and
    others) with write access to that repository only. It is required on the first
    configuration and when switching from `ssh` to `https`. Later, if absent, the
    saved one is kept. It is encrypted at rest.
  - **`ssh`**: `remote_url` is `git@host:user/repo.git` or
    `ssh://git@host[:port]/user/repo.git`. It is normalized to the second form. The
    path is limited to harmless characters. `secret` is **not allowed**. The
    platform generates the key (ed25519; the private key is encrypted at rest, like
    the TOTP secrets). The response returns `ssh_public_key`. Register it at the
    provider as a *deploy key with write access*. `regenerate_key: true` creates a
    new key, and the old one stops working. On configuration, the platform reads the
    **host key** of the host (`ssh-keyscan`), pins it, and returns its fingerprints
    in `ssh_host_fingerprints`. From then on, the connection is verified against
    that key (`StrictHostKeyChecking=yes`). A different or unknown key makes the run
    fail. It is never accepted automatically. `host_fingerprint` (`SHA256:…`, from
    the provider's documentation) is optional. If the detected key does not match
    it, the configuration is rejected (`400`). Changing the host or the port runs
    the detection again. Because the first detection is trust on first use, for
    known providers always set the expected fingerprint.
- **`DELETE /api/v1/blogs/{slug}/git-sync`**: `204`. It deletes the configuration,
  token, or key. It does not touch the remote repository. The deploy key must be
  revoked at the provider.
- **`POST /api/v1/blogs/{slug}/git-sync/run`**: `202`. Queues a sync (`409` if one
  is running, `400` if disabled). The result appears in `GET`. Also, an hourly run
  queues all active configurations.

**What is written.** For a blog: `{language}/{slug}.md`. For wiki and docs: the
page tree, with an `index.md` for pages that have subpages (for example
`{language}/intro/index.md` and `{language}/intro/install.md`). Each file has a
YAML front matter (title, slug, language, status, dates, author with the public
name, tags; for wiki and docs also `parent` and `position`), followed by the
Markdown. Images stay as URLs. By default only published content is written. With
`include_drafts`, drafts and revisions under review are included too, but never
posts hidden by moderation. The root contains `.notturni.json`. An existing branch
without this file is never overwritten.

**Guarantees.** The push is always a normal push (never forced), and never to the
default branch of the repository. A repository with no commits at all is rejected.
If the generated files are identical to the last result, the run does not touch
the network. Every configuration, removal, and run is recorded in the audit log
(`git_sync.configured`, `git_sync.removed`, `git_sync.run`).

**Known SSH host fingerprints (Super Admin only).** The platform keeps a list of
hosts with the expected fingerprints of their keys. GitHub and GitLab.com are
included, entered by the migration with the fingerprints the providers publish.
If the host of an SSH repository is in the list, the configuration pins **only**
the detected keys that match an expected fingerprint. If none matches, the
configuration is rejected (`400`, with the fingerprints received). For hosts not in
the list, trust on first use and the optional user fingerprint still apply.

- **`GET /api/v1/admin/git-sync/known-hosts`** → `[{host, port, fingerprints,
  note, updated_at}]`.
- **`PUT /api/v1/admin/git-sync/known-hosts`** → `{host, port?, fingerprints,
  note?}`. It takes 1 to 10 `SHA256:…` fingerprints and replaces the existing ones.
  Use it when a provider rotates its keys. Configurations already pinned do not
  change by themselves. The sync fails until the owner saves the configuration
  again. That configuration is then verified against the updated list.
- **`DELETE /api/v1/admin/git-sync/known-hosts/{host}/{port}`**: `204`.

All of these are recorded in the audit log (`git_sync.known_host_saved`,
`git_sync.known_host_removed`).

**Not done in this version:** import from the repository, export of media files,
user-supplied SSH keys (the platform generates them), and SSH CAs
(`@cert-authority`). Deleting a content item or the account (GDPR Art. 17) does
not delete it from the remote repository. The user owns that repository, and the
UI states this.

## Portal identity and SEO (`branding` module)

An icon, a sharing image, search engine texts, and verification codes, managed by
administrators at `/admin/seo`. Reading is public, because the frontend uses it
for the meta tags of every page. If the module is off, the defaults apply.

- **`GET /api/v1/seo/site?locale=`** (public) → `{locale, site_name, title,
  description, keywords, noindex, google_verification, bing_verification,
  yandex_verification, twitter_site, theme_color, extra_meta[], icons[],
  apple_touch_icon, social_image_url}`. The title and description come from the
  requested language. If missing, they fall back to the default language, then
  `it`, then `en`, and finally to the first available one.
- **`GET /api/v1/admin/seo`** → `{branding, locales}`, with images already
  resolved to URLs.
- **`PUT /api/v1/admin/seo`** → `{site_name, noindex, google_verification,
  bing_verification, yandex_verification, twitter_site, theme_color, extra_meta,
  locales}`. It replaces the whole set. Absent fields are cleared, and languages
  not sent are removed. It does not touch the images. Validation: text on one line
  without `< > "`. Verification values are the code only, not the whole tag.
  `twitter_site` is `@name` (max 15 characters). `theme_color` is `#rrggbb`.
  `extra_meta` allows up to 10 `{name, content}` pairs, without the names managed
  by the portal (`robots`, `viewport`, `description`, `keywords`, `refresh`,
  `http-equiv*`, and others) and without duplicates. `locales` has the form
  `{it: {title, description, keywords}}`. Audit: `branding.settings_updated`.
- **`POST /api/v1/admin/branding/icon`** (multipart `file`, PNG/JPEG/WEBP, max 2
  MiB): from **one nearly square image** (short side ≥ 128 px, aspect ratio within
  1.1; it is cropped at the center), it generates PNGs of 16, 32, 48, 180, 192, and
  512 px. None is ever larger than the original. They serve as favicon, iOS and
  Android shortcuts, and manifest. It replaces and deletes the previous set. It
  returns `400` with the reason if the image is too small, not square, or
  unreadable. `DELETE` removes it.
- **`POST|DELETE /api/v1/admin/branding/social`**: image for the sharing previews
  (Open Graph and Twitter card), width ≥ 600 px, stored as is. The frontend uses it
  as a fallback when a page has no image of its own. The cover of a post, if it
  exists and is not sensitive, takes precedence.
- The images are stored in the public avatar bucket under `branding/`. Audit:
  `branding.icon_updated|icon_removed|social_updated|social_removed`.

**Effects on the frontend.** Title and description, keywords, Open Graph and
Twitter card, icons (`<link rel="icon">`, `apple-touch-icon`),
`/manifest.webmanifest`, theme color, verification codes (`google-site-verification`,
`msvalidate.01`, Yandex), and extra meta tags. With `noindex`, the portal adds
`noindex, nofollow` to every page, `robots.txt` becomes `Disallow: /`, and the
sitemaps are emptied. This does not remove what search engines have already
indexed.

## Sitemap

Generated on the fly from Postgres, never by hand, and declared in `robots.txt`.
They contain only public, indexable content. Drafts, scheduled posts, private,
suspended, paused, or deleting Spaces are excluded, and so is anything the author
has excluded from search engines (blog or post).

- **`GET /sitemap.xml`** (frontend) → one `<urlset>`. Above 5000 URLs, it becomes a
  `<sitemapindex>` that points to `/sitemaps/{n}.xml`. It contains the home page,
  the site pages (`/p/…`), the Spaces, and the posts. Translations declare all
  languages as `xhtml:link rel="alternate" hreflang` (posts: translation group;
  site pages: `?locale=`).
- **`GET /{slug}/sitemap.xml`**: one Space only. It contains the home page, posts,
  static pages, and publications. It also works on `{slug}.{domain}/sitemap.xml`
  (rewritten by `proxy.ts`). The listed addresses are always the canonical portal
  addresses. `404` if the Space is not indexable.
- **`GET /{slug}/robots.txt`** (and `{slug}.{domain}/robots.txt`): the Space's
  exclusions for search engines and AI crawlers, with the paths rewritten for the
  subdomain, and a reference to its sitemap. A Space that is not indexable blocks
  everything.
- Backend: **`GET /api/v1/seo/sitemap?offset=&limit=`** (max 5000) → `{total,
  noindex, entries[{path, updated_at, alternates[]}]}` in stable order (home page,
  site pages, Spaces, posts). And **`GET /api/v1/blogs/{slug}/sitemap`** → `{slug,
  entries[]}`.
- In `solo`, both sitemaps (global and per-Space) use the primary-blog addresses: the
  primary blog is listed as `/` and its posts as `/post/<slug>`. Other Spaces and their
  posts keep `/<slug>` and `/<slug>/<post>`. The redirect URLs are not listed. See
  [Solo mode](#solo-mode-primary-blog-and-personal-site-routes).

## Privacy policy and cookie policy (`legal` module)

Two documents, `privacy` and `cookie`, are already written (in Italian and English)
for the platform components and adapted to the installation. The source text is
Markdown with **variables** and **conditions on active features**. On publication
it is resolved and becomes a site page (`/p/privacy`, `/p/cookie-policy`, read with
the `/pages` routes), with the translations as linked pages. Everything is reserved
to `amministratore`/`super_admin` (Administrator and Super Admin roles), except the
last route.

- **Template syntax**: `{{name}}` (value), `{{name|fallback text}}`,
  `{{#if feature}}…{{/if}}`, and `{{#unless feature}}…{{/unless}}`. Blocks can be
  nested. An empty value with no fallback is **missing**. An unknown name or an
  unclosed block is an **error**. In both cases, the document is not published.
- **Features** (detected from the installation): `newsletter`, `comments`,
  `turnstile`, `sso`, `sso_google|microsoft|github|linkedin`, `mfa`,
  `custom_domains`, `git_sync`, `link_preview`, `image_moderation`, `smtp`,
  `backup`, `audit_archive`, `reports`, `gdpr_tools`, `discovery`, `solo`, and
  `transfers`. The text describes only what is active. If a feature changes, the
  document must be published again. Until then it is marked `stale`.
- **Variables**: controller data (`controller_name`, `controller_address`,
  `controller_email`, `controller_registry`, `dpo_contact`, `hosting_description`,
  `processors`, `backup_retention`, `audit_archive_retention`, `minimum_age`) and
  derived values (`instance_fqdn`, `session_days`, `audit_days`, `deletion_days`,
  `grace_days`, `last_update`).
- **`GET /api/v1/admin/legal`** → `{settings, values, flags, documents,
  template_locales}`. `documents` lists each language with `published`, `stale`,
  `customized`, `missing[]`, and `issues[]`. The bundled templates (it, en) are
  created automatically. Until they are customized, they follow the platform
  updates.
- **`PUT /api/v1/admin/legal/settings`** → controller data (single-line text;
  `minimum_age` 10–18; `controller_email` must contain `@`). Absent fields are
  cleared. Pages already published are rewritten with the new values. If clearing a
  field would leave a page incomplete, the request returns `400`.
- **`GET|PUT /api/v1/admin/legal/{doc}/{locale}`**: one document (`LegalDocOut`:
  source, `default_content`, resolved text `rendered`, and status). `PUT {title,
  content}` saves it and creates the language if missing (any two-letter code).
  It does not publish.
- **`POST …/{doc}/{locale}/reset`** `{from?}`: restores the template of the
  language (it or en), or copies the text of another language (`from`).
- **`POST …/{doc}/{locale}/publish`** `{replace_existing?}`: returns `400` if data
  is missing or there are errors. Returns `409` if a page with the same address
  already exists and was not created here. It replaces such a page only with
  `replace_existing: true`. `POST …/unpublish` withdraws the page.
  `DELETE …/{doc}/{locale}` deletes an added language (not `it` or `en`).
- **`GET /api/v1/legal/public?locale=`** (public) → `[{doc, slug, locale, title}]`:
  the published documents in the requested language, or in the best available one.
  The site footer uses it.
- Audit: `legal.settings_updated`, `legal.document_saved`, `legal.document_reset`,
  `legal.document_published`, `legal.document_unpublished`,
  `legal.document_deleted`.

**Stated limits.** The texts are a template, not legal advice. Providers,
transfers outside the EU, and retention periods must be checked by whoever runs
the instance. The platform **does not delete audit-log archives from storage on its
own**. The period set in `audit_archive_retention` must be enforced manually until
an automatic expiry exists.

## Publications

A publication collects a blog's posts as chapters under `/{blog}/pub/{name}`. The
order is chronological, from the oldest, or explicit (`Post.chapter_order`). A post
belongs to at most one publication. `POST /blogs/{slug}/posts`,
`POST /posts/{id}/translations`, and `PATCH /posts/{id}` accept `publication_id`
(three states, like `category_id`). It returns `400` if the publication does not
belong to the blog. Changing the publication resets the order. `PostOut` includes
`publication: {id, name, title} | null` and `chapter_order`.

**`GET /api/v1/blogs/{slug}/publications`**: follows the blog's visibility.
Returns `[{id, name, title, description, chapters_total, chapters_published,
created_at}]`. Readers see only publications with at least one published chapter.
Users with write access see all of them.

**`GET /api/v1/blogs/{slug}/publications/{name|id}`**: the index. Same fields as
above, plus `chapters: [{n, post_id, slug, title, locale, status, published_at,
permalink, reading_minutes, is_public}]`. Readers see only published chapters
(`404` if there are none). Users who write also see drafts and scheduled chapters,
with `is_public=false`.

**`POST /api/v1/blogs/{slug}/publications`**: `{name, title, description?}`
(requires write access). `name` is a URL segment: lowercase letters, digits, and
hyphens. Returns `409` if the name is already used. **`PATCH .../{name|id}`**:
same fields. **`DELETE .../{name|id}`**: `204`. The posts remain without a
publication.

**`PUT /api/v1/blogs/{slug}/publications/{name|id}/order`**: `{post_ids}` in the
desired order (mockup 3g, drag-to-order). Posts not listed follow at the end, in
chronological order. Returns `400` if an id does not belong to the publication.

## Blog notes library

The post footnotes (`post_notes`) are linked on save to a **blog note**
(`blog_notes`) with the same normalized text (lowercase, no punctuation or
apostrophes). The blog note is created if missing. Its `kind` and `url` come from
the post note if it sets them. Otherwise they are estimated: `guess_kind()` gives
`book`, `article`, `web`, or `note`, and `extract_url()` turns a DOI into
`https://doi.org/…`. `GET /blogs/{slug}/bibliography` exposes `kind` and `url`,
plus the same bibliographic fields as `post_notes` (see above), for each entry.

**`GET /api/v1/blogs/{slug}/notes?q=`**: owner and collaborators. Returns
`[{id, content, kind, url, title, author, source, issued, isbn, doi, page,
created_at, updated_at, used_in: [{post_id, post_slug, post_title, idx}],
possible_duplicates: [id, …]}]`, newest first. `possible_duplicates` lists notes of
the same blog whose first 30 normalized characters are equal. `title`, `author`,
`source`, `issued`, `isbn`, `doi`, and `page` can be written through this API (see
`POST` and `PATCH` below). The library UI (`NotesTab.tsx`) cannot write them yet. It
edits only `kind`, `url`, and `content`.

**`POST /api/v1/blogs/{slug}/notes`**: `{content, kind?, url?, title?, author?,
source?, issued?, isbn?, doi?, page?}` (requires write access). Returns `400` if the
kind is not one of the allowed values or the URL is not http or https.

**`PATCH /api/v1/blogs/{slug}/notes/{id}`**: same fields as `POST`, all optional.
An absent field is not touched. An empty string clears it. Changing the text also
updates it in the posts that cite the note.

**`DELETE /api/v1/blogs/{slug}/notes/{id}`**: `204`. Returns `409` if a post cites
the note.

**`POST /api/v1/blogs/{slug}/notes/{id}/merge`**: `{into_id}`. The citations of the
note move to the destination, including the text. The source note is deleted.
Returns the destination note.

**`GET /api/v1/blogs/{slug}/notes/export.bib`**: BibTeX
(`application/x-bibtex`). One entry per note: `@book`, `@article`, or `@misc`. The
structured fields are included when present: `title`, `author`, `publisher` or
`journal` depending on the type (for `source`), `year` (for `issued`, estimated from
the text if absent, for compatibility with notes created before this field), `isbn`,
`doi`, and `url`. It always includes `note` with the full text.

**`POST /api/v1/blogs/{slug}/notes/import`**: `{bibtex}`. A minimal parser that
reads the same structured fields as above (`author`, `title`, `journal` or
`publisher`, `year`, `isbn`, `doi`, or `note` for free text; `url`, `doi`). Entries
already present (same normalized text) are skipped. Returns `201` with the created
notes. Returns `400` if no entry is recognized.

## Media and backup

The storage backend is selected with `NOCT_STORAGE_BACKEND`. The values are `s3`
(default; Garage, AWS S3, or any S3-compatible endpoint) or `localstorage` (local
filesystem, served by the backend itself on `/storage`; meant for `solo`
installations without S3 storage). Everything lives in a single bucket or
namespace (`NOCT_S3_BUCKET_CONTENT`), under the prefix
`{NOCT_SITE_SLUG}/userdata/{user_uuid}/{blog_uuid}/...`. This lets several
installations or purposes share the same physical bucket (or the same directory,
with the `localstorage` backend) without collisions.

- **`.../media/{uuid}.{ext}`**: images uploaded via `POST /blogs/{slug}/media` (see
  the Blog section above). The same endpoint serves images embedded in the Markdown
  content (`![alt](url)`) and the cover image of a post (`Post.cover_image_url`,
  set later with `PATCH /posts/{post_id}`). The upload does not distinguish the two
  uses. The caller decides where to use the returned URL. **Public read**: the images
  are meant to be embedded in posts and served to visitors. The bucket policy is
  scoped to this prefix only (`.../media/*`), not to the whole bucket.
- **`.../posts/{post_uuid}.md`**: a backup or fallback copy of each post's
  Markdown. It is written by the `backup` module (a consumer of
  `posts.post.published`) every time a post is published. The event travels on the
  NATS bus. A bus or S3 failure never makes a post save fail. The database remains
  the source of truth, and this file is only a safety copy. **Private**: it is not
  meant to be served directly to visitors.

The worker must be started separately (`noct-worker`, already present in
`compose.yaml` as the `worker` service). Without it, messages stay queued until the
worker starts.

### Automatic image moderation

Every upload via `POST /blogs/{slug}/media` passes, synchronously and before the
response, through a self-hosted NSFW classifier. It is a separate containerized
service in `moderation/`. No image ever leaves the infrastructure. This matches the
EU-focused, GDPR-oriented design (see `docs/ABSTRACT.md`). The backend calls it
internally through `NOCT_MODERATION_SERVICE_URL` (set automatically by
`compose.yaml`). The response includes `is_sensitive: bool`:

```json
{"url": "https://.../media/{uuid}.png", "is_sensitive": false}
```

The caller decides what to do with it. For an image in the content, the editor
inserts it as `![alt](url "sensitive")`. The `title` is the convention that carries
this information inside the Markdown itself, with no dedicated table. See
`frontend/src/lib/markdown.ts`, which renders such an image blurred and clickable to
reveal it. For a post cover, the flag must be passed explicitly as
`cover_image_is_sensitive` on create or update (see the Post section). The server
does not recompute it at that point.

**Fail open**: if the moderation service is unreachable, does not respond in time,
or `NOCT_MODERATION_SERVICE_URL` is not set, `is_sensitive` is always `false`. A
problem in this auxiliary service must never make an otherwise successful upload
fail. The same principle already applies to the post backup to S3. This is not
meant as a legal safety barrier. It is only an automatic aid for the author.

## Blog media library

`POST /api/v1/blogs/{slug}/media` (upload, see above) now records every image in
`media_files` and also returns `media_id`.

**`GET /api/v1/blogs/{slug}/media`**: owner and collaborators (`403` otherwise).
Returns `{items: [{id, url, content_type, size_bytes, alt_text, caption,
categories, is_sensitive, uploader_username, created_at, used_in: [{post_id,
post_slug, post_title, permalink}], used_as_blog_cover}], total_bytes}`, newest
first. `used_in` covers both images cited in the content (`post_media`) and images
used as a post cover (`Post.cover_image_url`). Earlier versions tracked only the
first kind. As a result, an image used only as a cover was reported as "used by
nobody" and could be deleted while it was still the live cover of the post. This is
fixed. `used_as_blog_cover` is `true` if the image is the current blog cover
(`Blog.cover_image_url`, see the cover image section above). That is now also
always recorded here at upload.

**`POST /api/v1/blogs/{slug}/media/sync`**: requires write access. Imports into the
library the images cited in posts that do not have a row yet (blogs created before
the library existed): `size_bytes=0`, no uploader. Idempotent. Returns the same as
`GET`.

**`PATCH /api/v1/blogs/{slug}/media/{media_id}`**: `{alt_text?, caption?,
categories?}`. Categories must be among `SENSITIVITY_CATEGORIES`. Otherwise
`400`. Posts already written keep their alt text and warning in their own Markdown.
They are not rewritten. The library values are the default for future uses.

**`DELETE /api/v1/blogs/{slug}/media/{media_id}`**: `204`. Returns `409` if the image
is still cited in a post (content or cover), or if it is the current blog cover. It
removes the row and, if it was uploaded through the library, the object in storage.

## Link preview

**`GET /api/v1/link-preview?url=<url>`** — public, no authentication (it also serves the rendering of the public post page, not only the editor). Fetches the Open Graph title, description and image of an external page:

```json
{"url": "https://esempio.org/articolo", "title": "...", "description": "...", "image": "https://.../cover.jpg"}
```

`title`/`description`/`image` are `null` if they are not found in the page HTML, or if the fetch fails (unreachable site, timeout, redirect — redirects are not followed automatically). In that case the response is still `200` with only `url` set, never an error, so the caller can always show a plain link. `400` only for a URL that is not eligible from the start: a scheme other than `http`/`https`, or a hostname that resolves to a private, loopback, link-local or reserved address (SSRF mitigation; not an absolute barrier, same "best-effort help" principle already applied to the automatic image moderation above). `429` above 30 requests/minute from the same IP (rate limiting via Redis — it limits the use of this endpoint as a proxy/scanner toward third parties, since the endpoint has no authentication; fails open if Redis is unreachable).

For YouTube hosts (`youtube.com`, `youtu.be`, ...), which without a consent cookie serve the same generic page for every video, the title, channel (in `description`) and thumbnail come from YouTube's public oEmbed endpoint; if that fails, the endpoint falls back to the normal HTML fetch.

**Two-level cache** (Redis and database), shared and deduplicated by URL — two posts, users or blogs that cite the same link make one real fetch, not one each: Redis as the hot cache (TTL 6h), and the table `link_preview_cache` (one row per URL, unique on `url_hash` = sha256 of the URL) as the persistent source that survives a Redis restart or flush. A preview with real Open Graph data stays valid for one week before it is checked live again. A failure (unreachable host, not HTML, no meta tags, ...) is cached for 6 hours only, so the endpoint does not stay blocked longer than needed but also does not keep hitting a host that never responds. The live fetch works as before (timeout 8s, body truncated at about 3 MB or at the closing `</head>`, redirects not followed).

Used by the editor (`frontend/src/components/editor/LinkPreviewCard.tsx`) when a URL is pasted on its own: the link stays plain, deletable text, and the card is a separate block just below it, saved in the Markdown as a link with `title="card"` (`[url](url "card")`, same convention as "sensitive" on images) — without saving a snapshot of the title, description or image, which are requested again each time the editor is opened or the public page is rendered.

## Comments

Who can comment is governed by `comments_mode` (`everyone` | `members` | `closed`), default `members`: at blog level (`PATCH /api/v1/blogs/{slug}`), with an optional override per post (`PATCH /api/v1/posts/{post_id}`; `comments_mode: null` reverts to the blog setting). A thread has one level only: `Comment.parent_id` points to a *top-level* comment of the same post (`400` if it points to a reply or to a comment of another post). This keeps conversations readable without limiting the depth at schema level.

**`POST /api/v1/posts/{post_id}/comments`** — optional authentication. The effective `comments_mode` is the post's value if set, otherwise the blog's. `closed`: `403` for everyone, including registered users.

- **with a valid session** (any `comments_mode` except `closed`): the comment is attributed to the user, with status `approved` automatically (no moderation for registered users).
  ```json
  {"content": "...", "parent_id": "..."}
  ```
  `parent_id` is optional (a reply to a top-level comment of the same post). `author_display_name` in the response follows the profile preference `post_author_name_style` (username / first and last name / global alias / verified domain — not the blog alias, which applies only to posts: a comment always carries the person's name, not the blog's). It is **recalculated on each read**, not only on creation: a change of username or preference is reflected at once, including on past comments. `author_avatar_url` (always the author's real avatar, never an alias) follows the same logic: `null` for anonymous comments.
- **without a session, `comments_mode="members"`:** `401`.
- **without a session, `comments_mode="everyone"`:** requires `author_display_name`/`author_email` (otherwise `400`) and a valid `captcha_token` from the Cloudflare Turnstile widget, verified server-side — `400` if missing or invalid (fail closed: an unreachable Turnstile service blocks the comment). The comment is still created with status `pending`.
  ```json
  {
    "content": "...",
    "author_display_name": "...",
    "author_email": "...",
    "captcha_token": "..."
  }
  ```

**`GET /api/v1/posts/{post_id}/comments`** — public. Only `approved` comments.

**`GET /api/v1/posts/{post_id}/comments/pending`** — requires a session and either blog ownership, a membership with role `mediatore` (Mediator), or a `platform_role` of Amministratore (Administrator), Super Admin or Moderatore (Moderator) (`403` otherwise). The moderation queue of the single post.

**`GET /api/v1/blogs/{blog_slug}/comments`** — same authorization as `pending`. Comments of *all* posts of the blog with the status given by the optional `status` parameter (`pending` by default, or `approved` / `rejected`), most recent first. Each item also has `post_title` and `post_slug`. It serves per-blog moderation in the dashboard without one request per post; for the cross-blog version see `GET /api/v1/admin/comments` below.

**`POST /api/v1/comments/{comment_id}/approve`** / **`.../reject`** — same authorization as `pending` (so Amministratore/Super Admin/Moderatore can also use them on a comment of a blog where they have no membership). They change the status of the comment.

### Extended queue

`GET /api/v1/blogs/{blog_slug}/comments` also accepts `reported=true` (comments reported to the platform, in any status). Each comment exposes `reported_to_platform` and `report_note`. The `rejected` status is the "hidden" state of the queue (no new status).

**`POST /api/v1/comments/{comment_id}/report`** — `{note}` is required (`400` if empty), same authorization as `approve`. Reports the comment to the platform moderators: it appears in `GET /api/v1/admin/comments?reported=true` with `report_note`/`reported_at`. Audit `comment.reported`.

**`POST /api/v1/comments/{comment_id}/block-author`** — `{note?}`, same authorization. Adds the author to the blog's list of blocked authors and sets the comment to `rejected`. Registered user → `user_id`; anonymous comment → sha256 of the email (never the plain email or the IP). `400` if the author is the blog owner or cannot be identified. Audit `comment.author_blocked`. A blocked author receives `403` on `POST /posts/{id}/comments` for that blog.

**`GET /api/v1/blogs/{blog_slug}/blocked`** / **`DELETE .../blocked/{block_id}`** — same authorization. List of `{id, label, is_anonymous, note, created_at}` (`label` is `@username` or the free-text name of the anonymous author) and unblock (`204`).

**Automatic closing**: `PATCH /api/v1/blogs/{slug}` accepts `comments_auto_close_days` (integer ≥ 0, `0`/`null` = never). After N days from `published_at`, the post's `effective_comments_mode` becomes `closed` (`403` for new comments; existing ones stay visible).

## Reports

**`POST /api/v1/blogs/{slug}/report`** / **`POST /api/v1/posts/{post_id}/report`** — requires a session. `{reason: spam | abuse | illegal | other, note?}` (note max 500 characters). Reports a publicly visible blog or post to the platform moderators: `201` with `{id, target_type, target_id, reason, note, status, created_at}`. One report per reader and target (a second request returns the first one, `201`); `400` if the user reports their own content; `404` for non-public content; `429` above 10 reports per hour per user. Open reports appear in the admin panel (`GET /api/v1/admin/blogs/{id}/reports`) and are closed by admin actions.

## Fragments

A portion of text that a reader highlights (with the mouse) on a published post is saved in a unified personal collection. It is not tied to the post author, who does not need to do anything to enable it. It always requires a session: a fragment belongs to the user who saved it. Visibility is set at **user** level (`is_public`, chosen when saving and editable later), not at blog level: public = visible to other registered users of the platform, never to anonymous visitors; private (default) = visible only to the user who saved it. The text is saved as it is (not an offset in the post). The frontend searches for it again in the rendered text at each read to highlight it again (`lib/highlight-fragments.ts`), tolerating small whitespace differences but not a post that was rewritten in the meantime. In that case the fragment stays saved but is no longer highlighted on the page. Selecting again (even partly) a fragment that is already highlighted offers to remove it (`DELETE`) instead of saving a new one. This is handled entirely in the frontend (`Range.intersectsNode` on the existing `<mark>` elements), with no dedicated endpoint beyond the `DELETE`/`PATCH` described below.

**`POST /api/v1/posts/{post_id}/fragments`** — requires a session. `404` if the post is not published or is not visible to the user (same access rule as the public permalink). `400` if the text is empty or exceeds **15%** of the length of the post's raw Markdown — a proxy for the length of the rendered text, which the backend never computes (there is no server-side Markdown rendering, see Post). Saving the same text twice on the same post is idempotent: it returns the same fragment (with its current visibility, not overwritten by the `is_public` of the second request), not an error.

```json
{"text": "il pezzo di testo evidenziato", "is_public": false}
```

`is_public` is optional, default `false`.

```json
{"id": "...", "post_id": "...", "text": "...", "is_public": false, "created_at": "..."}
```

**`GET /api/v1/posts/{post_id}/fragments`** — requires a session. Only the fragments saved by the current user on that post (never those of other users, regardless of `is_public`: this endpoint is the one for highlighting the user's *own* fragments). The frontend uses it to highlight them at each read, whether the user reached the post from the collection page or from a direct link.

**`PATCH /api/v1/fragments/{fragment_id}`** — requires a session and ownership of the fragment (`404` otherwise, as for `DELETE` below). Changes the visibility after saving.

```json
{"is_public": true}
```

**`GET /api/v1/users/me/fragments`** — requires a session. Unified collection of all fragments saved by the user, most recent first: text, visibility, capture date, post title and public permalink, author name. The author name is the value already resolved when the post was saved or last edited (`Post.author_display_name`). It is not recalculated from an alias changed later, unlike `PostOut`. This simplification is accepted for this derived view.

```json
[{"id": "...", "text": "...", "is_public": false, "created_at": "...", "post_title": "...", "author_display_name": "...", "permalink": "/blog/post"}]
```

**`DELETE /api/v1/fragments/{fragment_id}`** — requires a session and ownership of the fragment (`404` otherwise, not `403`: it does not reveal the existence of the fragment to users who do not own it). `204` if removed.

### Aggregate views across all of the user's blogs

Five endpoints, all `GET /api/v1/users/me/...`, require a session and have no pagination (as the per-blog lists they generalize). Common scope: **all blogs the user owns or collaborates on** (owned blogs plus blogs where the user is a member; deleted blogs excluded). They are not one blog at a time like the matching tabs in `dashboard/blogs/{slug}`, which remain the only way to edit, upload or delete these contents (these views are read-only). Each item carries `blog_slug`/`blog_title` to show which blog it comes from.

**`GET /api/v1/users/me/posts`** — all posts (any status: draft, review, published, scheduled) of all the user's blogs, most recent first. Same response schema as `GET /blogs/{slug}/posts` (`PostOut`, which already includes `blog_slug`/`permalink`), generalized to several blogs.

**`GET /api/v1/users/me/media`** — generalizes `GET /blogs/{slug}/media` (section "Blog media library" below), same schema per row (alt, caption, categories, sensitivity, "used in") plus `blog_slug`/`blog_title`:

```json
[{
  "id": "...", "url": "...", "content_type": "image/jpeg", "size_bytes": 12345,
  "alt_text": "...", "caption": null, "categories": [], "is_sensitive": false,
  "uploader_username": "mario", "created_at": "...",
  "used_in": [{"post_id": "...", "post_slug": "...", "post_title": "...", "permalink": "/blog/post"}],
  "blog_slug": "...", "blog_title": "..."
}]
```

**`GET /api/v1/users/me/publications`** — generalizes `GET
/blogs/{slug}/publications` (section "Publications" below), same total/published chapter counts per publication, plus `blog_slug`/`blog_title`.

**`GET /api/v1/users/me/links`** — generalizes `GET
/blogs/{slug}/links-bibliography` (section "Link preview" / bibliography below) to several blogs, grouped by identical URL across all posts. **Author view**: unlike the public endpoint it generalizes, it does not apply the public visibility filter — an author also sees here the links in their own drafts. Each citation carries `blog_slug`/`blog_title`.

```json
[{"url": "...", "link_text": "...", "citations": [
  {"post_title": "...", "post_slug": "...", "permalink": "...", "locale": "it", "used_at": "...", "blog_slug": "...", "blog_title": "..."}
]}]
```

**`GET /api/v1/users/me/bibliography`** — generalizes the bibliography of footnotes (section "Blog notes library" below) to several blogs, grouped by identical note text. Same author view (no public visibility filter), and the same caution toward non-http(s) URLs in notes created before schema validation. Each citation carries `blog_slug`/`blog_title`.

## Static pages (main site)

> `/p/{slug}` chooses the language from `?locale=`, then from the visitor's interface language, then `it` and `en`, and finally the first published translation. If the page has more than one language, it shows a language selector. `privacy` and `cookie-policy` are handled by the `legal` module (see above).

Pages such as About, Contact, Privacy are not tied to a user blog (`blog_id: null` in the response). The platform team manages them, and they are always active (unlike blog pages, which are opt-in — see "Blog static pages" below). They use the same translation scheme as posts (see the Multilingual section above). Public permalink `/p/{slug}` (a dedicated prefix, so that it does not collide with blog slugs reachable without a subdomain at `/{blog_slug}/...`), also returned in the `permalink` field of the response.

**`POST /api/v1/pages`** — requires a session with `platform_role` `amministratore`/`super_admin` (`403` otherwise). Creates a page, the root of a new translation family.

```json
{"slug": "chi-siamo", "locale": "it", "title": "Chi siamo", "content": "...", "is_published": true}
```

`409` if a page with that slug already exists in that language.

**`POST /api/v1/pages/{page_id}/translations`** — same authorization. Adds a translation to the same family (same body as `POST /pages`, without an implicit `locale`: it must always be given explicitly).

**`GET /api/v1/pages/{page_id}/translations`** — public. Lists `{id, locale, slug, is_published}` of the published translations only, of the same family. Same pattern as the blog pages above and as posts.

**`GET /api/v1/p/{slug}?locale=it`** — public (optional token): without an admin session, only pages with `is_published=true` (`404` otherwise, whether draft or non-existent). With an admin session, drafts too, so they can be reviewed before publication.

**`GET /api/v1/pages?locale=it`** — public (optional token): the same published/all distinction based on the caller's role. Optional query parameter `q`: filters by title or slug (`ilike`, substring), used by the search in the Pages section of the dashboard (`frontend/src/app/admin/pagine`).

**`PATCH /api/v1/pages/{page_id}`** — requires an admin role. Updates a single translation (`slug`, `title`, `content`, `is_published`, all optional).

## User interests

**`GET /api/v1/interests`** — public, no authentication. Current list of selectable interests (the "user interests" block: fixed multilingual tags — a canonical non-linguistic key, never free text):

```json
[{"key": "music", "translations": {"it": "Musica", "en": "Music"}}, ...]
```

In `solo` mode no interests are offered: `GET /api/v1/interests` returns `[]`, the directory
filter `?interest=` is ignored, and public profiles return `"interests": []`. `PATCH /api/v1/users/me`
with a non-empty `interests` list answers `400` (the list is empty for the platform), while an
empty array is accepted. Values already stored in `users.interests` stay in the database and in
the GDPR export; the interests module and the admin editor of `platform_config.interests` are not
changed by `solo`.

The only source of truth is `platform_config.interests`. It is seeded from `NOCT_DEFAULT_INTERESTS` (JSON, same schema) or from a curated built-in list at first installation, and then a Super Admin can change it at any time (`PATCH /api/v1/admin/config`, field `interests` — see the Platform administration section). The frontend resolves the translation in the current language itself (fallback `en`, then the first available, then `key`).

## User profile and follow

**`GET /api/v1/users`** — public, no authentication. Public user directory (the "user directory" block, same schema as `GET /blogs`): only active accounts (not anonymized or deleted) that have not opted out (`User.directory_listed`, see `PATCH /users/me` below). The Super Admin is **always** excluded, regardless of their own `directory_listed`. This is not a user option, for security reasons: the account with the broadest privileges must not be discoverable in the public directory. `q` searches username, public alias and bio (`ILIKE`). `locale` filters by native language (`native_language`). `interest` filters by canonical interest key (to find people to share an interest with and follow them). `sort` is `new` (registration, default) or `followers`. `limit` (default 30, maximum 100), `offset`. Entry:

```json
{
  "username": "...", "display_name": "...", "bio": "...",
  "avatar_url": "...", "verification_tier": "none", "custom_domain": null,
  "interests": ["music", "cinema"], "follower_count": 3
}
```

**`GET /api/v1/users/{username}`** — public. Public profile:

```json
{
  "username": "...", "bio": "...",
  "first_name": "...", "last_name": "...", "display_name": "...",
  "post_author_name_style": "username",
  "country": "IT", "native_language": "it", "fallback_languages": ["en", "fr"],
  "interests": ["music", "cinema"],
  "avatar_url": "...", "social_links": [...], "created_at": "...",
  "verification_tier": "none", "custom_domain": null,
  "atproto_did": "did:web:notturni.eu:users:<uuid>",
  "activitypub_actor_id": "https://notturni.eu/ap/actors/<uuid>"
}
```

`verification_tier` (`none`|`bronze`|`silver`|`gold`|`blue`) is the profile verification badge, in the style of Bluesky, Instagram or Twitter. `bronze` comes from a custom domain verified via DNS (see below). `gold`, `silver` and `blue` come from lists or domains managed by hand by a Super Admin (`PATCH /admin/config`, see the Platform administration section). It is recalculated on each read. `custom_domain` is set only if a custom domain was verified successfully (never for a `pending` or `failed` state). The platform username always remains usable for citations and resolution: the domain is an addition, not a replacement at the routing/permalink level. `atproto_did`/`activitypub_actor_id` are **placeholder** identifiers for a possible future federation (AT Protocol/Bluesky, then ActivityPub/Mastodon, see ROADMAP.md §5). They are computed on the fly from `NOCT_INSTANCE_FQDN` and the user's UUID (never from the username, so that they stay stable if the username changes). They are not stored, and they are not really federated: no `/ap/...` or WebFinger endpoint is served. The string is only shown in the profile.

`{username}` in this endpoint (and in all the other `GET /api/v1/users/{username}/...` endpoints below, including follow/unfollow) also accepts a verified custom domain in place of the username. If there is no exact match on `username`, it retries on `custom_domains.domain` (only status `verified`) before returning 404. The username always remains usable: the domain is an additional identifier, not an exclusive one.

**`GET /api/v1/users/me`** — requires a session. Same as above, but with the owner's private fields, never exposed on anyone's public profile:

```json
{
  "...": "all fields of GET /users/{username}",
  "email": "...",
  "username_changed_at": "2026-09-10T12:00:00Z",
  "next_username_change_allowed_at": "2026-09-15T12:00:00Z",
  "pending_email_change": {"new_email": "...", "stage": "awaiting_old_confirmation"},
  "domain_pending_verification": "...",
  "domain_verification_instructions": {"txt_record_name": "...", "txt_record_value": "..."},
  "directory_listed": true, "interests": ["music", "cinema"]
}
```

`username_changed_at`/`next_username_change_allowed_at` are `null` if the username has never been changed. `pending_email_change.stage` is `awaiting_old_confirmation` (waiting for the code sent to the old mailbox) or `awaiting_new_confirmation` (waiting for the code sent to the new one) — see the email change flow below. `domain_pending_verification`/`domain_verification_instructions` are set only if a custom domain is not yet verified (`pending`/`failed`), so that the user can resume the flow without calling `POST .../domain` again. `directory_listed` (private, never exposed on `GET /{username}`) is the opt-out from the public directory (`GET /users` below), active by default. The profile always remains reachable through the direct `@username` link. This flag only excludes the user from the list and from search.

`display_name` is a global public alias. When set, it is the heading of the public profile, in place of the username or first and last name. `post_author_name_style` is the user's preference for the author name shown on their posts: `username` (default), `full_name` (first and last name), `display_name` (global alias) or `verified_domain` (verified custom domain, see below). It applies only when the blog does not impose a public name (see the Posts section). As with `display_name`, if the chosen value is not available (domain not verified), it falls back to the username. `first_name`/`last_name`/`country`/`native_language` are free or optional. `country` is only checked for format (ISO 3166-1 alpha-2, e.g. `IT`); it is not checked against an official list of countries. `native_language`/`fallback_languages` are two-letter ISO 639-1 codes, in the same format as the `locale` of posts and pages (see the Multilingual section). `fallback_languages` are also meant as the languages into which the user may later translate their own content. Maximum 5.

**`PATCH /api/v1/users/me`** (also accepts `ui_locale`: `it`|`en`; `""` reverts to the platform default — the interface language, returned by `GET /auth/me`) — requires a session and returns the same schema as `GET /api/v1/users/me`. Updates `username`, `bio`, `first_name`, `last_name`, `display_name`, `post_author_name_style`, `country`, `native_language`, `fallback_languages` (all optional). For `first_name`/`last_name`/`display_name`/`country`/`native_language`: an empty string `""` clears the field, an absent field leaves it unchanged, any other value replaces it (`400` if the format of `country`/`native_language` is invalid). `post_author_name_style`: one of `username` | `full_name` | `display_name` | `verified_domain` (`422` otherwise); an absent field leaves it unchanged. It is accepted even without an active verified domain: it falls back to the username until one is verified, the same behavior as an unset `display_name`. For `fallback_languages`: an absent field leaves the list unchanged; a list (even an empty one) replaces it (`400` if it has more than 5 entries or an invalid code). `directory_listed`: boolean; an absent field leaves it unchanged. It is the opt-out from the public directory (`GET /users` below). `interests`: array of canonical keys (see `GET /api/v1/interests`); an absent field leaves it unchanged, and a list (even an empty one) replaces it. Maximum 5; `400` if the limit is exceeded or if it contains a key that is not among the current platform keys. `username`: an absent field leaves it unchanged. Otherwise the same format and blacklist rules as registration apply (`400` if invalid, `409` if already in use), **plus a cooldown of 5 days** between two consecutive changes. If the cooldown is violated, the response is `409` with a message that gives the date of the next allowed change. The first change is never blocked (`username_changed_at` starts as `null`). The ID remains the real key by which the rest of the system refers to the user, so an allowed change is visible at once everywhere (posts, comments, `@mention` autocomplete). The exception is `@mentions` already written in the text of existing posts and pages: they are saved as plain text and are not rewritten. Audit event `user.username_changed` (`payload.old_username`/`new_username`).

### Verified email change

There is no `email` field in `ProfileUpdateRequest`/`PATCH /users/me`. The email change goes through a dedicated flow in **two steps**, so that the person requesting it controls both the old and the new mailbox. It uses the same OTP mechanism as email MFA: it is queued as a job on the NATS bus and sent by the worker. It uses a dedicated table (`email_change_requests`) instead of reusing `mfa_email_codes`, so that the two kinds of code for the same user are not confused. This applies only to the online product: it requires `NOCT_SMTP_HOST` to be set for real delivery, as for email MFA. Without it, the code is only logged in development.

**`POST /api/v1/users/me/email/request`** — requires a session. `{"new_email": "..."}` → `202`. Sends a 6-digit code (TTL 10 minutes) to the user's **current** mailbox. `400` if `new_email` is the same as the current one or is already used by another account. A previous pending request that was not completed is replaced.

**`POST /api/v1/users/me/email/verify-current`** — requires a session. `{"code": "..."}` → `202`. Verifies the code sent to the old mailbox, then sends a new one to the **new** mailbox. `400` if the code is wrong or expired, or if there is no pending request.

**`POST /api/v1/users/me/email/verify-new`** — requires a session. `{"code": "..."}` → `200`, same schema as `GET /users/me` with the email already updated. Verifies the code sent to the new mailbox and applies the change (checking uniqueness again to avoid a race condition). `400` if the code is wrong or expired, or if the previous step was not completed. Audit event `user.email_changed` (`payload.old_email`/`new_email`).

**`DELETE /api/v1/users/me/email/request`** — requires a session. `204`, idempotent (also without a pending request). Cancels the pending request at whatever step it is. It deletes the row in `email_change_requests`, not only the client-side state. Otherwise the OTP already sent would remain valid until it expires.

### Custom domain verified via DNS (Bluesky style)

One domain per user (`custom_domains`, unique `user_id`), verified by proving possession: the user publishes a TXT record in their own DNS. There is no HTTP dependency and no SSRF surface, only DNS lookups (`hickory-resolver`). A successful verification assigns the `bronze` badge (it never downgrades a higher tier already assigned by other logic in the future) and copies the domain to `users.verified_domain` (a denormalized column, cleared when the domain is removed). The platform username always remains the fallback identifier, citable and resolvable. The domain is an addition, not hard-wired into routing or permalinks (the per-blog subdomain remains `⚪` in ROADMAP.md §3). However, it **can be resolved** in place of the username in `GET /api/v1/users/{username}` and in the related public endpoints (see above), and it can be selected as `post_author_name_style` to sign posts and comments (`verified_domain`, falling back to the username if the domain is not verified).

**`POST /api/v1/users/me/domain`** — requires a session. `{"domain": "iltuodominio.it"}` → `200`. Creates or replaces the domain, in `pending` status:

```json
{
  "domain": "iltuodominio.it", "status": "pending",
  "txt_record_name": "_notturni-challenge.iltuodominio.it",
  "txt_record_value": "notturni-verify=<token>"
}
```

`400` if the format is not a valid hostname or if it is a (sub)domain of the platform itself (`NOCT_INSTANCE_FQDN`). `409` if it is already claimed and verified by another account, or if two users claim the same still-free domain in parallel (the first one to commit wins, and the other receives `409` instead of a generic error).

**`POST /api/v1/users/me/domain/verify`** — requires a session. Queries DNS for the expected TXT record (timeout 5s; a failure affects the single verification, not the feature). If it matches, `200` with `status: "verified"`, and the `bronze` badge is assigned. Otherwise `400` with `status: "failed"`, and it can be retried. Rate-limited (5 attempts per 10 minutes per user, with the same fail-open behavior as the rest). Audit event `user.domain_verified`.

**`DELETE /api/v1/users/me/domain`** — requires a session, `204`, idempotent. If the domain was verified and the tier was `bronze` (assigned only by this verification), it resets `verification_tier` to `none`.

**`GET /api/v1/users/{username}/blogs`**, **`.../posts`**, **`.../comments`** — public, `404` if the user does not exist. Tabs of the public profile (mockup 3e): public, non-suspended blogs owned by the user (same schema as `GET /blogs/{slug}`, with `owner_id` set to `null`), posts published on public blogs (same schema as the feed; `limit` default 20, maximum 50; `offset`), and approved comments on public posts (`{id, content, created_at, post_title, permalink}`, same pagination). **Privacy rule**: only content signed publicly with the username is listed. A blog whose `default_author_display_name` differs from the username, the posts signed with that alias, and the comments left with an alias are excluded. Otherwise these endpoints would link the alias to the real identity.

**`GET /api/v1/users/me/follow-stats`** — requires a session. Sums the user's followers (`UserFollow`) with those of all their blogs (`BlogFollow`), including the blogs that present themselves with an alias different from the username:

```json
{
  "user_followers": 12,
  "blogs": [{"blog_slug": "il-mio-blog", "blog_title": "...", "alias": "Anonimo Curioso", "followers": 40}],
  "total_followers": 52
}
```

Reserved for the owner: this is the only endpoint where the real identity and the blog aliases appear together. Publicly, `GET /api/v1/users/{username}/followers` and `GET /api/v1/blogs/{slug}/followers` continue to show only the count or list of the single entity, without ever linking them to each other.

**`POST /api/v1/users/me/avatar`** — requires a session, `multipart/form-data` with field `file`. Accepted formats: PNG, JPEG, WEBP; maximum 2 MiB (`400` otherwise). Uploads to the configured storage backend (S3/Garage with a custom endpoint always injected, or the local filesystem; the `avatars` bucket/namespace is publicly readable). Replaces and deletes the previous avatar, if any. Returns `{"avatar_url": "..."}`.

**`DELETE /api/v1/users/me/avatar`** — requires a session. Removes the current avatar (idempotent).

**`POST /api/v1/users/me/social-links`** — requires a session. `{"label": "mastodon", "url": "https://..."}` → `201`. Maximum 5 links per profile (`400` above the limit). `label` is no longer meant as a free-form label but as a platform key (e.g. `mastodon`, `bluesky`, `github`, `website`). The list of known platforms, with their monochrome icons, exists only in the frontend (`frontend/src/lib/social-platforms.tsx`, easy to extend). The backend still does not validate the value against a closed list: it remains a free string.

**`DELETE /api/v1/users/me/social-links/{link_id}`** — requires a session. Only the user's own link (`404` otherwise, so that its existence is not revealed).

**`POST /api/v1/users/{username}/follow`** / **`DELETE .../follow`** — requires a session. Follow / unfollow a user. Idempotent. `400` if the user tries to follow themselves.

**`GET /api/v1/users/{username}/followers`** / **`.../following`** — public. Lists of `{username}`.

### GDPR (ROADMAP.md §1)

**`GET /api/v1/users/me/export-data`** — requires a session. Right of access and portability (Art. 20): a JSON snapshot of all data linked to the account. This covers the profile, owned blogs, posts and comments written (anywhere, not only on the user's own blogs, since they remain words written by the user), saved fragments, follows (in both directions, IDs only), API tokens (name, prefix and dates, never the secret or the hash), audit events where the user is the actor (up to 1000, most recent), and any custom domain claim (also if still `pending`/`failed`, not only a verified one). The `verification_token` is never included: it is an operational secret, not personal data. The structure is free-form, not a typed `response_model`: the object changes with the active modules (each module adds its own sections).

**`DELETE /api/v1/users/me`** — requires a session. `{"confirm_username": "il-proprio-username"}` → `204`. `400` if it does not match the current username exactly. There is no confirmation by password: the flow is meant to be started from a confirmation form already authenticated in the dashboard, not by an automated client.

**Important**: it does not delete the `users` row — it **anonymizes** it. A user may own public blogs with collaborators, or may have commented on other users' blogs. Deleting the row would require either blocking the operation until no linked blog, post or comment exists (there is no blog transfer or deletion feature today), or deleting that content in cascade as well, harming third parties who asked for nothing. This approach is still allowed by GDPR when strict erasure conflicts with the rights of third parties (Art. 17.3).

Deleted in full (purely personal data): sessions (logout everywhere), API tokens, pending email MFA codes, SSO identities, social links, saved fragments, follows (in both directions), and the memberships as collaborator on other users' blogs. `username`/`email` are replaced with unique anonymous values, and `is_active=false` (blocks login and reuse of existing tokens/sessions at once). `display_name="Utente eliminato"` (Deleted user) with `post_author_name_style="display_name"`: owned blogs, posts and comments remain, but at once with this author everywhere (the same dynamic alias resolution already used for blog privacy). Event `user.account_deleted` in the audit log, recorded **before** the anonymization (so the `actor_label` keeps the original username/email).

## API tokens (core engine / direct user access)

A token is an opaque value (not a decodable JWT), generated with the prefix
`noct_`. Only its sha256 hash is stored in the database (table
`api_tokens`). Each token has an
`owner_type`:

- **`core`** — token of the engine or a service, not tied to a user. Meant for
  machine-to-machine calls: internal workers, maintenance scripts, scheduled
  tasks.
- **`user`** — token tied to a specific `User`, so that the user can work with
  the API without going through the frontend editor or the admin of their own
  blog. It can be managed from the dashboard
  (`frontend/src/app/dashboard/token`).

The `/api/v1/tokens` endpoints accept **two different authentication schemes**
on the same `Authorization: Bearer` header:

- an **opaque ApiToken** (`noct_...`) — inherits the `owner_type` (and the user,
  if applicable) of the token itself; this is the bootstrap and rotation
  mechanism for `core` tokens.
- a **session JWT access token** (user login) — needed to issue the *first*
  `user` token from the dashboard, where by definition no ApiToken exists yet to
  authenticate the request. With this authentication `owner_type` is always
  `user`, tied to the user of the session.

Authentication tells the two cases apart by the prefix of the received value
(`noct_` vs JWT). It then normalizes the actor for the endpoints below and for
the audit log.

### How to get the first core token

```bash
cd backend && source .venv/bin/activate
python -m scripts.create_api_token --name "core-engine"
```

The plain-text value is printed only once. A user does not need the script: they
create their own first `user` token directly from the dashboard (session JWT,
see above).

### `POST /api/v1/tokens`

Creates a new token with the same `owner_type` (and user, if applicable) as the
actor that authenticated the request (ApiToken or session JWT).

```json
{"name": "descrizione-libera"}
```

→ `201`. The `token` field is the plain-text value, shown only here.

### `GET /api/v1/tokens`

Lists the tokens with the same `owner_type` as the caller (for `owner_type=user`,
only the user's own tokens). It never exposes the plain-text value or the
hash, only `token_prefix`.

### `DELETE /api/v1/tokens/{token_id}`

Revokes a token (sets `revoked_at`, does not delete it). `403` if it is not one
of the caller's own tokens, `404` if the id does not exist.

## Editable site texts (`/site-texts`, `/admin/site-texts`)

In `solo` mode `PUT /api/v1/admin/site-texts/{locale}` answers `403` ("Site texts are fixed in solo
mode."), after the authentication and role checks. `GET /api/v1/site-texts` keeps working and returns
the overrides already stored. Footer texts stay editable through `PATCH /api/v1/admin/config`.

Administrators can rewrite, language by language, the texts of the home page
(manifesto, pillars, buttons) and of the newsletter sign-up form. The defaults
stay in the frontend translation files; only the overrides are stored here.

- **`GET /api/v1/site-texts?locale=it`** (public) → `{locale, texts}` with only
  the overridden keys (`{"Home.eyebrow": "…"}`). The frontend merges them with
  the defaults on every request (time-based cache of 60 s, invalidated by the
  `site-texts` tag).
- **`PUT /api/v1/admin/site-texts/{locale}`** (`amministratore`/`super_admin`) → `{"texts": {key: text | null}}`: sets a key or,
  with `null`/empty string, restores the default. Atomic (one error → nothing is applied); allowed locales `it`, `en`; at most 200
  keys per request; audit `site_texts.updated`.
  - Allowed keys: only the groups `Home.` and `NewsletterSignup.` (`400` for others).
  - Value: plain text on one line, up to 2000 characters, **without `{ } < >`** (messages go through an ICU renderer that
    would otherwise break). The only exception is `NewsletterSignup.consent`, which accepts the `<privacy>…</privacy>` tag (the link
    to the privacy notice), once and well-formed. No apostrophe immediately before `<` or `>`.
  - Texts with placeholders (`{count}`) cannot be rewritten from the interface.

## Map of links (`graph` module)

**`GET /api/v1/graph`** (user) → `{scope, nodes, edges, truncated}`. The graph is computed by Postgres at request time (no own
table, no graph database: see ROADMAP).
- **Nodes**: `blog` (Space), `post`, `tag`, `category`, with `id` (`post:{uuid}`, `blog:{slug}`, `tag:{name}`, `cat:{uuid}`),
  `label` and the fields specific to the type (for `blog` posts: `slug`, `path`, `status`, `published_at`).
- **Edges**: `in` (post → Space), `tag`, `category` and `link` (post → post, with `via` = `url` for a link in the text that points to a
  post on the site, in the form `https://{fqdn}/{blog}/{post}`, `https://{blog}.{fqdn}/{post}` or `/{blog}/{post}`, or `wiki` for a
  `[[page]]` of the same Space). The domain of internal links is `NOCT_INSTANCE_FQDN`: it must match the public domain (and
  `NOCT_SITE_URL`, which the editor uses for links inserted from the search). Duplicate links, self-links, external links and links
  to posts outside the set are discarded.
- **What is visible**: with `scope=mine` (default), the posts of the user's own Spaces (owner or member), published only;
  `drafts=true` adds drafts and posts under review. With `scope=platform` (only `amministratore`/`super_admin`, otherwise `403`) it
  returns everything that is public and visible in the lists (never drafts, private Spaces, suspended or paused ones).
- **Filters**: `blog` (slug; `404` if it is not in the set), `kind` (`blog`|`wiki`|`docs`), `limit` (1–800, default 300: the most
  recent posts; `truncated` says whether there are more). Invalid values → `400`.

## Platform administration

All endpoints require a session with `platform_role` in
`amministratore`/`super_admin` (`403` otherwise), **except**
`GET /api/v1/admin/comments` below, which also accepts `moderatore`.
They are used by the frontend sections
`frontend/src/app/admin/{utenti,blog,moderazione,moderazione-commenti,registro}`
— under the `/admin/*` prefix, separate from `/dashboard/*` (personal
sections), not a separate app — see ROADMAP.md.

**`GET /api/v1/admin/overview`** — also accepts `moderatore`. Platform overview:
`users_total`,
`users_new_7d`, `blogs_total`, `blogs_suspended`, `posts_published`, the queues
`queue_pending_comments`/`queue_posts_in_review`/`queue_hidden_posts`,
`queue_open_reports`, `audit_today` (audit log entries since midnight UTC), `deployment_mode` and
`services`: a list `{name, status, detail?}` with `status` in `ok`/`down`/
`unconfigured` for `postgres` (`SELECT 1`), cache, bus and
`storage` (TCP connection with a 2 s timeout; `storage` is always `ok` with
`localstorage`) and `moderation` (`GET /health` of the service, `unconfigured`
if `NOCT_MODERATION_SERVICE_URL` is absent). Aggregates only, no personal data.

**Required note (B5, mockup 5e)**: `PATCH /admin/users/{id}` (role change or
activation), `PATCH /admin/blogs/{id}` (suspension) and
`PATCH /admin/posts/{id}` (hide/show) require `note` (at least 3
characters, `400` otherwise) when they actually change the state; the note
ends up in `payload.note` of the audit entry. No note is required if the
request changes nothing.

### Platform settings

In `solo` mode `reserved_blog_names` is not available: `GET` returns `[]`, a `PATCH` with a non-empty
list answers `400` ("I nomi riservati non sono disponibili in modalità solo."), and an empty list is
accepted without writing anything. A value saved earlier stays in the database and is not shown. The
footer fields stay editable in `solo`.

**`GET /api/v1/admin/config`** / **`PATCH /api/v1/admin/config`** — `super_admin`
only (`403` otherwise). Single row `platform_config`, created on first access with the defaults (`NOCT_DEFAULT_LOCALE` for the language, 5 blogs,
open registration). Fields: `default_locale` (`it`|`en`),
`registration_mode` (`open`|`invite`|`closed` — `invite`/`closed` make
`POST /auth/register` answer `403`, with different messages),
`sso_providers` (subset of the providers configured via env; empty = all
configured ones; an excluded provider answers `403` on
`/auth/sso/{provider}/login`), `mfa_required_for_admins` (if enabled, an
Administrator/Super Admin without MFA gets `403` on the whole admin area
until they enable it), `reserved_blog_names` (in addition to the code blacklist
`reserved_builtin`, read-only), `moderation_threshold` (0–1,
passed to the moderation service on every upload), `max_blogs_per_user`
(1–100), `anonymous_comments_allowed` (if `false`, `comments_mode=everyone`
can no longer be set), `audit_retention_days` (7–3650, default seeded from
`NOCT_AUDIT_RETENTION_DAYS`: retention days for events in
`audit_log` before periodic deletion —
job `audit.maintenance`, read from the `platform_config` row
on each run, no longer from the env after the row is created). The response
also includes `infrastructure`: a read-only summary of the `NOCT_*`
environment (never secrets; besides the deployment, storage and service flags it carries `subdomains_enabled`, `worker_inline`, `local_storage_public_url` (only with `localstorage`), `modules` (active modules), `bus` (`memory`|`nats`) and `cache` (`memory`|`redis`); it no longer includes `audit_retention_days`, which is now an editable field of its own, not an env value),
`footer_column1_markdown`/`footer_column2_markdown`/`footer_column3_markdown`/`footer_bottom_bar_markdown`
(free Markdown, max 5000 characters each, `""` clears it — footer shown on
every public platform page and on every blog; see `GET /api/v1/footer`
below; columns 1/2 are only the default, which can be overridden per blog
in `PUT /blogs/{slug}/config` — never column 3 nor `bottom_bar`, which are always platform-wide only),
`interests` (the "user interests" block: full list — unlike
`reserved_blog_names`, this field **replaces** rather than adds to the list — of `{"key": "...", "translations": {"it": "...",
"en": "..."}}`; canonical key in slug format `[a-z0-9_-]{1,40}`, unique, at least one non-empty translation per entry, at most 200 entries; seeded when the row is created from `NOCT_DEFAULT_INTERESTS` (JSON, same schema) if set, otherwise from a
curated built-in list — see `GET /api/v1/interests`
below for the public list and `PATCH /users/me` for the user's choice;
removing a key here also cleans `User.interests` of every user who had selected it, not only the platform list), `max_blog_storage_mb`
(maximum space per blog — media + Markdown backups, same count as
`GET /blogs/{slug}/overview::storage_bytes` — in MB; `null`/`0` = no limit, default; exceeding it answers `413` on
`POST /blogs/{slug}/media` and `POST /blogs/{slug}/cover-image`),
`verification_gold_identifiers`/`verification_silver_identifiers`
(arrays of emails/usernames, case-insensitive match: they assign,
respectively, the `gold` verification badge — entities verified manually by
the platform, known publications/agencies/organizations/public figures — and
`silver` — financial supporters of the project; `verification_gold_identifiers`
also accepts a bare email domain, e.g. `"testata.it"`, not only whole emails/
usernames) and `verification_blue_domains` (array of email domains,
hostname format validated: they assign the `blue` badge, in addition to the
platform's own domain — `NOCT_INSTANCE_FQDN` — which is already included
automatically). `User.verification_tier` (see `GET /users/{username}`)
is computed on every read from the platform lists and from the verified domain; it changes with every event that can
change it (registration, email/username change, verification or removal of the custom
domain, edit of any of these three lists — which recomputes **all** active users in one go). Priority when several
criteria are met: `gold` > `silver` > `blue` > `bronze` (custom domain
verified via DNS) > `none`. A tier is never lost "by mistake": if a user is removed from a higher list, they fall back to the
next lower tier that is still valid, not straight to `none`.
Every change is written to the audit log (`platform.config_updated`, with
`changes: {field: {from, to}}`). If a `footer_*` field changes, the frontend
cache is invalidated on the shared tag `platform-footer` (all public pages, not only those of one blog).
`GET /api/v1/config` (public) exposes the effective `default_locale`, `registration_mode`
and `sso_providers`.

**`GET /api/v1/footer`** — public, no auth. Platform footer,
raw Markdown not yet rendered (the frontend renders it at read time, same principle as posts):

```json
{"column1": "...", "column2": "...", "column3": "...", "bottom_bar": "..."}
```

Each key is `null` if not configured (no default except for
`bottom_bar`, seeded when the `platform_config` row is created with a
link to the repository — in any case editable/clearable at any time).

### Modules (hot activation)

`super_admin` only. Optional modules are turned on and off without restarting anything: the API
writes the **desired** state to the database, and every replica (API and worker) applies it within a few
seconds (the one that received the request right away): it migrates the module schema, mounts the routes
and starts the consumers, or unmounts them. Disabling never deletes data or tables. Every change
goes to the audit log (`module.enabled`/`module.disabled`).

**`GET /api/v1/admin/modules`** — all modules compiled into the binary: `{name, version, kind:
core|optional, description, requires[], default_enabled, enabled, active, pending, error,
required_by[]}`. `enabled` is the desired state, `active` the one running in the replica that
answers, `pending = enabled != active`; `error` explains why a change was not applied
(e.g. failed migration; retried every 30 seconds).

**`POST /api/v1/admin/modules/{name}/enable`** / **`.../disable`** — returns the module as above.
`404` for an unknown module, `400` for a core module (always active), for an activation whose
dependency is missing, and for a deactivation of a module required by another active module.
Idempotent. `NOCT_MODULES` is only used to seed the first installation.

### GDPR requests (B6, mockup 5f)

**`GET /api/v1/admin/gdpr?status=`** — admin. Register of requests:
`{id, username, type: export|deletion, status: open|approved|completed|
rejected, deadline_at, note, created_by_username, approved_by_username,
approved_at, completed_at, created_at}`, open ones first and sorted by deadline (30
days from receipt, Art. 12). Self-service actions (`GET
/users/me/export-data`, `DELETE /users/me`) leave a row already
`completed` with the note `self-service`.

**`POST /api/v1/admin/gdpr`** — `{username, type, note}` (note required: who asked and how).
Records a request that arrived out of band.

**`POST /api/v1/admin/gdpr/{id}/approve`** — second approval: for a
`deletion`, the approver must be an admin **different** from the one who recorded it
(`403` otherwise). An `export` does not need it.

**`POST /api/v1/admin/gdpr/{id}/execute`** — `export` (open or approved):
responds with the user's data as JSON (same format as
`/users/me/export-data`) and closes the request; `deletion`: only if
`approved`, anonymizes the account as `DELETE /users/me` does (`400` for a Super Admin). **`.../reject`** — `{note}` required. Everything goes to the audit log
(`gdpr.request_created|approved|rejected`, `gdpr.export_executed`,
`gdpr.deletion_executed`).

**`GET /api/v1/admin/users`** — lists all platform users
(id, username, email, `platform_role`, `is_active`, `mfa_enabled`,
`blogs_count` — blogs owned — and `last_seen_at`, last use of a refresh
session, `null` if never used). Optional query param `q`: filters by
username or email (`ilike`, substring).

**`POST /api/v1/admin/users`** — `Super Admin` only, `201`, rate limited (20 per 5 minutes per
admin). Body `{username, email, password?}`: creates an account with platform role `utente`,
**active at once** (there is no email-verification step: login only checks `is_active`), without registration and without
sending any email. It works in `solo` and `platform` mode and does not change `POST /auth/register`, which stays closed in
`solo` after the first user. Same validations and Argon2 hashing as `register` (`400` on invalid
username, email or password; `409` "Username o email già in uso."). If `password` is omitted the server generates a random one and
returns it **once** in `generated_password` (`null` otherwise; at least 20 characters, CSPRNG, never stored in clear nor written to the audit log); the admin hands it over out of band and the user
changes it with the usual change-password route. Response: `{id, username, email, platform_role, is_active,
generated_password}` (never the hash). The audit event `admin.user_created` (actor, target user, no password) is written in the same
transaction. The created user can then be invited by username to a blog (`POST /blogs/{slug}/invitations`). The same operation
is available from the shell without SMTP: `noct-ctl user create` (see BACKEND.md).

**`PATCH /api/v1/admin/users/{user_id}`** — `{platform_role?, is_active?}`.
- Assigning or removing the `amministratore`/`super_admin` roles (both as
  starting and ending value) requires being `super_admin`: an `amministratore` can only manage `utente`/`moderatore` (`403` otherwise).
- You cannot deactivate your own account (`400`) — this prevents locking out the last remaining Super Admin.
- A deactivated user (`is_active=false`) can no longer log in.

**`POST /api/v1/admin/users/{user_id}/reset-password`** — requires
`Administrator`/`Super Admin`, always `202`, no body. Forced password reset:
it triggers for the user the same email cycle as
`POST /auth/password/forgot` (same mechanism — 6-digit code,
10-minute TTL, sent as a job on the NATS bus), but started by the admin instead of
by the user: the admin neither sets nor sees any password, they only trigger the sending. `400` if the target user is not active. No rate limit
per IP/email (the one on `/auth/password/forgot` is meant for an anonymous
user enumerating addresses): only a more permissive limit per admin actor
(20/5 min), against a compromised admin account spamming resets to many
users. Audit event `user.password_reset_triggered` (no note
required, unlike role change/activation — it is not a persistent state change on the account).

There is no endpoint to create the first Super Admin (there is no session to authenticate the request), but it no longer has to be promoted by hand in the database: if `NOCT_SUPER_ADMIN_USERNAME`/`NOCT_SUPER_ADMIN_EMAIL`/
`NOCT_SUPER_ADMIN_PASSWORD` are all set (`.env`/`k8s/secret.yaml`),
the account is created automatically when the backend starts, if it does not already
exist (startup hook of the `auth` module) — idempotent: a later restart neither recreates nor touches it. As alternatives, the first user's
auto-promotion in `solo` mode and a manual `UPDATE` on the database in `platform` mode remain available.

**`GET /api/v1/admin/blogs`** — lists all platform blogs (id,
slug, title, `owner_username`, `visibility`, `is_suspended`, `is_paused`,
`deleted_at`, `posts_count`, `reports_open`). Optional query params: `q`
(slug, title or owner), `visibility`, `state` (`active` |
`suspended` | `paused` | `deleted` | `reported` — the last one: only blogs
with open reports, sorted by count).

**`PATCH /api/v1/admin/blogs/{blog_id}`** — `{is_suspended: bool, note}`. A suspended blog
is publicly unreachable (the detail stays readable with `is_suspended=true` for the warning page, see the Blogs section) and not
writable, owner included, until it is reactivated.
Recorded in the audit log with the note.

**`GET /api/v1/admin/blogs/{blog_id}/reports`** — reports panel
(mockup 5e): `{blog, owner_mfa_enabled, owner_email_domain, reports: [{id,
target_type, target_id, post_slug, post_title, reason, note,
reporter_username, created_at}]}` — only the **open** reports on the blog and its posts.

**`POST /api/v1/admin/blogs/{blog_id}/action`** — `{action, note}` (note
required). `action`: `suspend`, `restore`, `hide_reported_posts` (only the
posts with open reports), `deactivate_owner` (deactivates the owner's account and suspends the blog; `403` for an administrator unless they are a
super admin), `dismiss` (archives). Closes all open reports of the
blog (`actioned`, or `dismissed` for `dismiss`) and records the action with the note. Returns the updated blog.

**`GET /api/v1/admin/posts`** — lists all platform posts (with `reports_open`), newest first
(id, title, slug, `blog_slug`, `blog_title`, `author_username`,
`status`, `is_hidden`, `published_at`, `created_at`) — drafts/
under review/scheduled included, not only published ones. Optional query param
`q`: filters by post title or slug, blog slug, or author username
(`ilike`, substring).

**`PATCH /api/v1/admin/posts/{post_id}`** — `{is_hidden: bool}`. See
`Post.is_hidden` in the Posts section: hides/shows a post regardless of
`status`, including for the author. No notification to the author and no field
for the reason; the state change is however recorded in the audit log
(`post.hidden`/`post.unhidden`, see below).

**`GET /api/v1/admin/comments`** (also accepts `reported=true`: only comments reported from the blogs, with `report_note`/`reported_at`, most recently reported first) — requires `platform_role` in
`amministratore`/`super_admin`/**`moderatore`** (the only endpoint in this
section that is also open to the Moderator role, ROADMAP.md §1). Comments from *all*
platform blogs in the state given by the optional parameter
`status` (`pending` by default, or `approved`/`rejected`), newest first — same shape as
`GET /api/v1/blogs/{slug}/comments` but without restriction to a single blog, with `blog_id`/`blog_slug`/`blog_title` added.
Optional query param `q`: filters by comment content, author name,
post title or blog slug (`ilike`, substring).
Approve/reject remain the `POST /api/v1/comments/{comment_id}/approve`/
`reject` already described above (same authorization).

### Audit log

Every sensitive action leaves a row in `audit_log` (append-only), written
in the same transaction as the action: authentication (`auth.login` with
`payload.method` `password`/`mfa_*`/`sso_*`, `auth.login_failed`),
administration (`user.role_change` with `payload {from,to}`,
`user.activated`/`user.deactivated`, `blog.suspended`/`blog.unsuspended`,
`post.hidden`/`post.unhidden`, `comment.approved`/`comment.rejected`), API
tokens (`api_token.created`/`api_token.revoked`), account (`user.
account_deleted`, GDPR), blog invitations and membership
(`blog.invitation_created`/`_accepted`/`_declined`/`_revoked`,
`blog.member_role_changed` with `payload {from,to}`, `blog.member_removed`),
static pages (`page.created`/`page.updated`/`page.deleted`, both platform
and blog pages — `blog_id` is present only for the latter). Each
row carries:

- `actor_type`/`actor_id`/`actor_label` — who: actor type, its id (if
  applicable) and a readable snapshot `username <email>` at the time of the
  event (it stays valid even if the account is later renamed or deleted).
- `ip`/`user_agent` — source IP address of the request (the **last** hop of
  `X-Forwarded-For` behind Traefik — the only one written by the trusted proxy, not by the
  client, which could otherwise spoof a first hop at will — otherwise
  the direct connection IP) and user agent.
- `target_type`/`target_id`/`blog_id` — on what: type and id of the object
  involved, plus the context blog when applicable (denormalized to filter
  without joins).
- `payload` (JSON) — details specific to the action; for actions with a
  context blog it also includes `blog_alias`, the public alias under which that blog
  appears (`Blog.default_author_display_name`, `null` if the blog
  has none) — in addition to the real identity in `actor_label`, never instead of it.
- `occurred_at`.

**Channel** (web/api/system): not a column of its own, but a field computed from
`actor_type`, both in the response of `GET /api/v1/admin/audit-log` and in its
filter (see below). `user`/`anonymous` always come from an authenticated browser/app
session, or from a not-yet-authenticated request (login), never from an
ApiToken; `core_token`/`user_token` are always direct access via an
opaque token; `system` is an internal process with no HTTP request (bootstrap,
scheduled jobs). The channel is derived from `actor_type`.

Events older than `NOCT_AUDIT_RETENTION_DAYS` (default 105) are downloaded
to storage as gzipped NDJSON, one file per ISO week, and then removed from the
database (job `audit.maintenance`); deletion never touches events
not yet archived.

**`GET /api/v1/admin/audit-log`** — rows, newest first. Optional query params:
`action`, `actor_id`, `target_id`, `blog_id` (exact match),
`channel` (`web`/`api`/`system`, see above), `since`/`until` (ISO 8601,
range `[since, until)` on `occurred_at`), `limit` (1–500, default 100),
`offset` (default 0). Only events still in the database: those already
archived are on storage, and can be re-imported with
`noct-ctl audit restore AAAAwSS`.

## Feed (multi-blog home page)

**`GET /api/v1/feed/posts`** — public, no authentication. Published posts (with `published_at` reached) from **`public`** blogs only (blogs with
`members`/`private` visibility are excluded — see `visibility` in the Blogs section), newest first. It is meant for the platform home page
(product rule: the home page collects articles in the user's language, dev.to style; see `docs/ABSTRACT.md`).

Optional query params: `kind` (`blog`|`wiki`|`docs`; **by default only `blog` Spaces**, wiki and documentation pages are not feed articles; unknown value → `422`), `locale` (filters one language, otherwise all
together), `tag` (filters by normalized tag, e.g. `poesia`, not `#Poesia` —
see the "Tags" section above), `category` (filters by category slug — see the
"Categories" section above; since categories are per blog, different blogs with
a category of the same name appear together, as already happens for tags),
`limit` (default 20, maximum 50), `offset` (pagination, default 0),
`following=true` (requires a session, `401` otherwise — mockup 1c
"Following"): only posts from the blogs followed or written by followed users,
with the same visibility constraints as the rest of the feed. It is a
separate router from `/blogs/{slug}/posts` on purpose: here posts span
different blogs and are not scoped to a specific slug/id.

**`GET /api/v1/feed/trending`** — public, no authentication. Most used tags among
published posts (from **`public`** blogs only) in the last `days`
days (default 7, maximum 90), most frequent first:
`[{"tag": "poesia", "post_count": 12}, ...]`. Optional query
params: `days`, `limit` (default 10, maximum 30). There are no like/share counters
on the platform yet (see ROADMAP.md): this is the only basis available today for a "trending" section.

**`GET /api/v1/feed/locales`** — public, no authentication. Post count
per language, same visibility filters as `GET /feed/posts`, most used first,
**at most 5 results**, only languages with at least one post:
`[{"locale": "it", "count": 42}, {"locale": "en", "count": 7}]`. Meant
for the language filter pills on the home page — it replaces a static list
of hardcoded languages that could show languages with no posts.

## Search

Two distinct endpoints, not one with an optional scope: search on the portal spans all public blogs, while search on a single blog (also
reached from its own subdomain) stays limited to that blog's posts.
Both search title and content with `ILIKE` (no stemming or Postgres full
text search today, same pragmatic approach already used by
`GET /blogs?q=`), so matching is case-insensitive and does not need whole words.

**`GET /api/v1/search/posts`** — public, no authentication. `q`
(required; empty string or spaces only → `[]` without error) searches title/content among the published posts of **`public`** blogs only, with the same
visibility constraints as `GET /feed/posts`, newest first. Optional query params: `limit` (default 20, maximum 50), `offset` (pagination, default
0). To search blogs themselves by name see `GET /blogs?q=` (Blogs section).
`q` longer than 200 characters (after trimming, counted in Unicode characters) → `422`.
Rate limit per IP: 120 requests per minute; above it → `429` with `Retry-After`.

**`GET /blogs/{slug}/search`** — public. As above but limited to the posts
of blog `{slug}` (`404` if the blog is not visible to the caller, same
behavior as `GET /blogs/{slug}/posts`); always only actually published posts, even for
users with write access to the blog (unlike
`GET /blogs/{slug}/posts`, which also shows drafts/under review to them: search is a public box, not an editing tool). Same
`limit`/`offset` as above, the same 200-character cap on `q` (`422`) and the same per-IP rate limit (120 per minute, `429`).

Notes for both routes:

- `limit` and `offset` are clamped, not rejected: out-of-range values become the nearest bound (`/search/posts`: `limit` 1–50;
  `/blogs/{slug}/search`: `limit` 1–50, `offset` ≥ 0). A non-numeric value → `422`.
- The per-IP key is the client IP derived from `X-Forwarded-For` according to `NOCT_TRUSTED_PROXIES` (default `1`, i.e. one
  trusted proxy in front of the API). With `0`, or without `X-Forwarded-For`, the TCP peer IP is used. Clients behind the same
  NAT share the same bucket.
- Browser access: the calls come from the frontend, which runs on the same origin as the API behind the reverse proxy. A
  browser on a per-blog subdomain calling the API directly needs that origin in `NOCT_CORS_ORIGINS` or `NOCT_CORS_ORIGIN_REGEX`
  (see [CONFIGURATION.md](CONFIGURATION.md#browser-cors-and-frontend)). Search needs no cookie or `Authorization` header.
- Pagination in the frontend pages (not an API limit): 20 results per page, `?page=` from 1 to 1000. The last reachable page
  therefore covers offset 19980.

## Newsletter (ROADMAP.md §3)

One list per blog (`blog_slug`) plus one platform list (digest,
no `blog_slug`), double opt-in via email, self-service unsubscribe/deletion
without login.

**`POST /api/v1/newsletter/subscribe`** — public. Body `{email, blog_slug?,
locale?}`. Rate-limited (5/hour per IP, 3/hour per email). Always answers
`202 {"status": "ok"}`, including when the email is already subscribed and confirmed:
no address enumeration through a different response. If new or not
yet confirmed, it generates an opaque confirmation token (sha256 hash in the
table, like API tokens) and queues the email sending as a job on the NATS bus.

**`GET /api/v1/newsletter/confirm?token=...`** — public. `200
{"status": "confirmed"|"already_confirmed"|"invalid"}`, idempotent (a second click on the same valid link answers `already_confirmed`, not an error). Token expired after 48 hours → `invalid`.

**`POST /api/v1/newsletter/unsubscribe`** — public. Body `{token,
reason?}`. The `token` is an HMAC-signed link (not a hash in the table: it must
be rebuildable for every send, not only verifiable once), included
at the bottom of every email sent. `400` if the token is not valid.

**`POST /api/v1/newsletter/unsubscribe/delete`** — public. Body
`{token}`. Permanent deletion of the row (right to erasure,
GDPR Art. 17), self-service with no login needed: the recipient of the email
is already identified by the signed token.

**Management per blog** (owner or `autore`/`co_autore` membership,
same logic as `can_write_posts`):

- `GET /blogs/{slug}/newsletter/stats` → `{pending, confirmed,
  unsubscribed}`.
- `GET /blogs/{slug}/newsletter/campaigns` → list of campaigns (automatic and
  manual), most recent first.
- `POST /blogs/{slug}/newsletter/campaigns` — body `{subject, body_markdown,
  scheduled_at?}`. Without `scheduled_at` (or with one in the past): immediate
  sending (`status=sending`, job on the NATS bus). With a future `scheduled_at`:
  it stays `status=scheduled` — **no scheduler sends it automatically yet**, it is only the persisted state. `scheduled_at` must
  include a time zone (e.g. suffix `Z` or `+00:00`): a naive value
  is rejected with 422, since it cannot be compared with the current instant. The sending
  (job of the `newsletter` module) is idempotent on message redelivery
  (table `newsletter_deliveries`): a subscriber already
  notified does not receive a second email if the worker is interrupted
  midway through sending and the message goes back to the queue.
- `PATCH /blogs/{slug}/newsletter/campaigns/{id}` / `DELETE .../campaigns/{id}`
  — edits (`{subject?, body_markdown?, scheduled_at?}`) or cancels a
  campaign, **only while `status=scheduled`** (409 otherwise: a
  campaign already `sending` may have already been picked up by the worker,
  and there is no safe window to intercept it; automatic
  `post_notification` campaigns are created already `sending`, so they are never in this
  state). `DELETE` does not delete the row; it sets `status=canceled` (the worker skips it if the message is already queued). `scheduled_at` is
  tri-state like the settings below: omitted leaves it unchanged, `null` or
  an instant in the past immediately converts the campaign to immediate
  sending (same logic as creation).
- `GET /blogs/{slug}/newsletter/settings` / `PATCH .../newsletter/settings`
  — `{newsletter_auto_notify_enabled, newsletter_sender_name,
  newsletter_banner_url, newsletter_banner_alt_text}`. The first field
  turns the automatic notification on every published post off or on for
  this blog (on by default, `Blog.newsletter_auto_notify_enabled`),
  with one automatic send per post even if it is republished
  (unique constraint on `post_id`). The other three customize the campaign email:
  display name in the `From` header (the address is always `NOCT_SMTP_FROM_EMAIL`, never an arbitrary domain) and a banner shown at the top
  of the HTML body (`newsletter_banner_url` must start with `http://`
  or `https://`, as the links in the notes/bibliography do). **PATCH is tri-state**:
  a field omitted from the body leaves the current value unchanged, `null` clears it explicitly — this also applies to `newsletter_auto_notify_enabled`,
  made optional for this purpose. Defaults (all fields absent/empty): sender name = blog title, no banner.
- Campaign emails are
  `multipart/alternative`: a plain-text fallback (clients without HTML, screen
  readers) plus an HTML version with the banner and `body_markdown` rendered
  to HTML (`pulldown-cmark` + sanitization with
  `ammonia` — no `<script>`, no `on*` attributes, no scheme other than
  `http`/`https`/`mailto` in `href`/`src`, the same care as for notes/
  bibliography). It is the only point in the backend that renders Markdown server-side: posts do it only on the frontend. The other platform emails
  (OTP, password reset) remain plain text only.

**Platform digest** (Super Admin/Administrator,
`require_platform_admin`), same shapes as above with `blog_id=None`:
`GET /admin/newsletter/stats`, `GET /admin/newsletter/campaigns`,
`POST /admin/newsletter/campaigns`, `PATCH`/`DELETE
/admin/newsletter/campaigns/{id}` (same editability/cancellation rules as
above), and `GET`/`PATCH /admin/newsletter/settings` — same
tri-state scheme as above but without `newsletter_auto_notify_enabled` (not
applicable to a digest not tied to the publication of a single blog),
source `platform_config` instead of `Blog`.

## CORS

The backend accepts browser calls from the origins in `NOCT_CORS_ORIGINS`
(comma-separated; default `http://localhost:3000`, exact origins) plus, optionally,
`NOCT_CORS_ORIGIN_REGEX` (Python regex, Starlette's `allow_origin_regex`) — needed for the per-blog subdomains (`slug.notturni.eu`, see
`k8s/ingressroute.yaml`): an exact origin for each cannot be enumerated in
advance. Production example: `https://([a-z0-9-]+\.)?notturni\.eu`.

## Health

**`GET /api/v1/health`** — public, no authentication. Also runs
`SELECT 1` on the database. Meant for readiness/liveness probes (Kubernetes,
compose healthcheck).

**`GET /api/v1/config`** — public, no authentication. Exposes
`{"deployment_mode": "solo"|"platform", "subdomains_enabled": true|false, "turnstile_site_key": "..."|null, "primary_blog_slug": "..."|null}`
(`primary_blog_slug` is described in "Solo mode" above)
(`NOCT_DEPLOYMENT_MODE`/`NOCT_SUBDOMAINS_ENABLED`/`NOCT_TURNSTILE_SITE_KEY`; `subdomains_enabled` defaults to `true` when unset and only informs the frontend, the backend rules do not change), plus `default_locale`,
`registration_mode` and `sso_providers` (the enabled and configured providers; `[]` without the
`auth_sso` module). `deployment_mode` is
used by `frontend/src/app/admin/layout.tsx` to hide the Users entry in
`solo` mode; `turnstile_site_key` is used by `CommentsSection` to know whether
to show the captcha widget on comments open to everyone (`null` if the instance
has none configured) — both without needing an authenticated session already. Site key only, never the secret key: it is meant to be public.

**`GET /api/v1/seo/crawl-directives`** — public, no authentication.
Exposes `{"search_disallow": ["/blog", "/blog/post", ...], "ai_disallow":
[...], "ai_user_agents": ["GPTBot", "ClaudeBot", ...]}` — relative paths to
put in `Disallow` in robots.txt, computed from the opt-ins for
blog/post crawlers (see below). `ai_disallow` always also includes all of
`search_disallow`: a specific user-agent group in robots.txt fully replaces `User-agent: *` for that bot; it does not add to it. Only `public` blogs that are
not suspended and posts that are publicly visible (`is_publicly_visible`) — a
`members`/`private` blog is in any case unreachable by an anonymous crawler.
Used by `frontend/src/app/robots.ts` (special Next.js route, generates
`/robots.txt`).

**`GET /api/v1/seo/sitemap-entries`** — public, no authentication.
Exposes `{"blogs": [{"slug", "updated_at"}, ...], "posts": [{"permalink",
"updated_at"}, ...]}` — only content that is actually indexable (same criterion as
`search_disallow` above: a blog/post whose effective `search_indexing_enabled` is `false` does not appear). Used by
`frontend/src/app/sitemap.ts` to generate `/sitemap.xml`.

## Common errors

| Case                                         | Status |
| -------------------------------------------- | ------ |
| `Authorization` header missing/malformed     | 401    |
| Session/token not found, revoked or expired  | 401    |
| Wrong login credentials                      | 401    |
| Domain rule violated (slug, blog limit)      | 400    |
| Slug already in use                          | 409    |
| Insufficient permission (role/ownership)     | 403    |
| Resource not found                           | 404    |
