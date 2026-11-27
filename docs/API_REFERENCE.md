# API reference — endpoints by module

List of all endpoints exposed by the backend, derived from the `api.toml` manifests of each module (237 routes in 27 modules).
This page complements [API.md](API.md), which describes payloads, domain rules and examples of the main flows. Here the goal is to
know **which routes exist, with which verb, who can access them, and why they exist**. The complete machine-readable schema
(request/response types) is served by `GET /api/v1/openapi.json` of the running backend.

## Conventions

- **Prefix**: all paths are relative to `/api/v1` (e.g. `POST /auth/login` → `POST /api/v1/auth/login`), except `/storage` in the "Platform" section.
- **Alias**: `/spaces/…` is equivalent to `/blogs/…` (a "blog" in the contract is a *Space*: `blog`, `wiki` or `docs`).
- **Format**: JSON; errors have a `{"detail": "…"}` body with the appropriate HTTP status.
- **Authentication**: header `Authorization: Bearer <access token>` (login) or `Bearer noct_…` (API token, only where indicated).
  The refresh token is an `HttpOnly` cookie.
- **Verbs**: `GET` reads (idempotent, no side effects); `POST` creates or starts an action/job; `PUT` replaces a resource or a whole collection;
  `PATCH` partially modifies (absent fields = unchanged, `null`/`""` clear the value where documented); `DELETE` removes.
- **Status**: if not indicated, `200` for `GET`/`PUT`/`PATCH`, `201` for creations, `204` with no body; `202` = queued (the outcome arrives asynchronously).

### "Access" column

| Value | Meaning |
|---|---|
| public | No authentication. Space resources still respect the Space visibility (`public` · `members` · `private`). |
| user | Any authenticated user. |
| user or API token | Authenticated user, or API token with the appropriate scope. |
| write on the Space | User with write permission on the Space named in the path (owner, co-author, etc.); the route declares `scope = blog`. |
| role: … | Required platform role (super admin, administrator, moderator). |

The **Notes** column reports the status when it differs from the default, the **rate limit** (`maximum / window`, per `ip` or per `user`),
and any flags: *raw* = handler that manages the response directly (cookie, download, stream); *upload* = multipart.

## Platform (server routes, not a module)

| Verb | Path | Access | Description |
|---|---|---|---|
| GET | `/health` | public | Liveness: the process responds. Used by probes. |
| GET | `/health/ready` | public | Readiness: checks the database and infrastructure services before receiving traffic. |
| GET | `/openapi.json` | public | OpenAPI document generated from the manifests of the active modules. |
| GET | `/storage/{bucket}/…` (outside `/api/v1`) | public | Only with `NOCT_STORAGE_BACKEND=localstorage`: serves public buckets from the filesystem. |


## admin  (core)

Platform administration: overview, users, blogs, posts, audit log and settings. Optional modules add counters and settings to it with `AdminSection`.

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| GET | `/admin/overview` | role: super admin, administrator, moderator | KPIs, open queues, today's audit and service health |  |
| GET | `/admin/users` | role: super admin, administrator | Users with blog count and last login (`q` on username/email) |  |
| POST | `/admin/users` | role: super admin | Creates an active account (role `utente`) without registration or email; the password is chosen by the admin or generated and shown once | 201; rate limit 20/5 min per user |
| PATCH | `/admin/users/{user_id}` | role: super admin, administrator | Role and activation (note required; administration roles only by the Super Admin) |  |
| POST | `/admin/users/{user_id}/reset-password` | role: super admin, administrator | Starts for the user the same email cycle as forgotten password | 202; rate limit 20/5 min per user |
| GET | `/admin/blogs` | role: super admin, administrator | All blogs with counters; filters `q`, `visibility`, `state` (active\|suspended\|paused\|deleted\|reported) |  |
| PATCH | `/admin/blogs/{blog_id}` | role: super admin, administrator | Suspends or reactivates a blog (note required if it changes) |  |
| GET | `/admin/posts` | role: super admin, administrator | All posts with open reports (`q`) |  |
| PATCH | `/admin/posts/{post_id}` | role: super admin, administrator | Hides or shows a post (note required if it changes) |  |
| GET | `/admin/audit-log` | role: super admin, administrator | Audit log, newest first; filters action, actor_id, target_id, blog_id, channel, since, until, limit (max 500), offset |  |
| GET | `/admin/config` | role: super admin | Platform settings, including those of the active modules |  |
| PATCH | `/admin/config` | role: super admin | Updates the settings; every change goes to the audit log with the previous values. In solo, a non-empty `reserved_blog_names` is rejected (400) | |
| GET | `/admin/modules` | role: super admin | Modules compiled into the binary, with desired and running state |  |
| POST | `/admin/modules/{name}/enable` | role: super admin | Enables an optional module (hot: migrates the schema, mounts the routes, starts the consumers) |  |
| POST | `/admin/modules/{name}/disable` | role: super admin | Disables an optional module (routes unmounted, data and tables intact; rejected if another active module requires it) |  |
| PUT | `/admin/site-texts/{locale}` | role: super admin, administrator | Bulk-edits the UI texts of one language (atomic, audited); only keys of the allowed groups, plain text without braces or tags. In solo, always 403 | |

## auth  (core)

Registration, session login (access JWT + refresh with rotation), forgotten password, password change, API tokens.

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| POST | `/auth/register` | public | Registers a user | 201; rate limit 30/10 min per ip |
| GET | `/auth/username-available` | public | Checks whether a username is available (?username=) | rate limit 30/1 min per ip |
| POST | `/auth/login` | public | Login with email and password; the refresh token travels in an httpOnly cookie | raw |
| POST | `/auth/refresh` | public | Renews the session from the refresh cookie (rotation); requires X-CSRF-Token | raw |
| POST | `/auth/logout` | public | Revokes the current session; requires X-CSRF-Token | 204; raw |
| GET | `/auth/me` | user | The user of the current session |  |
| POST | `/auth/password/forgot` | public | Sends a reset code (always responds 202, so the email cannot be enumerated) | 202 |
| POST | `/auth/password/reset` | public | Sets a new password with the received code | 204 |
| POST | `/users/me/password` | user | Changes the password by providing the current one; revokes all sessions | rate limit 10/5 min per user |
| POST | `/tokens` | user or API token | Issues an API token (shown only once) | 201 |
| GET | `/tokens` | user or API token | The tokens of the same owner |  |
| DELETE | `/tokens/{token_id}` | user or API token | Revokes a token of one's own owner | 204 |

## blogs  (core)

Blogs, collaborators and invitations, lifecycle (pause, deletion with grace period, transfer), presentation configuration.

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| POST | `/blogs` | user | Creates a Space (`kind` = blog\|wiki\|docs; slug of at least 4 characters, not reserved; per-user limit across all Spaces). In solo: no minimum length and no reserved list, only format and collisions with site routes | 201 |
| GET | `/blogs/mine` | user | Blogs owned by the user |  |
| GET | `/blogs/member-of` | user | Blogs of other owners on which the user has a membership |  |
| GET | `/blogs/received-invitations` | user | Collaboration invitations awaiting a response |  |
| POST | `/blogs/received-invitations/{invitation_id}/accept` | user | Accepts an invitation (creates the membership) |  |
| POST | `/blogs/received-invitations/{invitation_id}/decline` | user | Declines an invitation |  |
| GET | `/blogs/{slug}` | public | Blog details (404 if the visibility does not allow access) |  |
| PATCH | `/blogs/{slug}` | user | Edits the blog (owner only) |  |
| DELETE | `/blogs/{slug}` | user | Deletion with grace period (requires retyping the slug) |  |
| POST | `/blogs/{slug}/restore` | user | Cancels the deletion within the grace period |  |
| POST | `/blogs/{slug}/transfer` | user | Transfers ownership to a co-author |  |
| GET | `/blogs/{slug}/members` | user | Blog collaborators (owner only) |  |
| PATCH | `/blogs/{slug}/members/{user_id}` | user | Changes the role of a collaborator (co_autore or mediatore) |  |
| DELETE | `/blogs/{slug}/members/{user_id}` | user | Removes a collaborator (idempotent) | 204 |
| PATCH | `/blogs/{slug}/my-membership` | user | The collaborator chooses the alias they sign posts with on this blog |  |
| GET | `/blogs/{slug}/invitations` | user | Invitations issued by the blog (owner only) |  |
| POST | `/blogs/{slug}/invitations` | user | Invites a user as co_autore or mediatore | 201 |
| DELETE | `/blogs/{slug}/invitations/{invitation_id}` | user | Revokes a pending invitation | 204 |
| GET | `/blogs/{slug}/config` | public | Presentation configuration (follows the blog visibility) |  |
| PUT | `/blogs/{slug}/config` | user | Replaces the whole configuration (owner only) |  |

## media  (core)

Blog images: upload for the editor with media library, blog cover and favicon, storage limit per blog, automatic moderation (fail open).

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| POST | `/blogs/{slug}/media` | write on the Space | Image to embed in the Markdown (PNG, JPEG, WEBP, GIF; max 10 MiB) | 201; upload |
| GET | `/blogs/{slug}/media` | user | Blog media library (owner and collaborators) |  |
| POST | `/blogs/{slug}/media/sync` | write on the Space | Imports into the library the images cited in posts that do not yet have a row |  |
| PATCH | `/blogs/{slug}/media/{media_id}` | write on the Space | Alt text, caption and content warning |  |
| DELETE | `/blogs/{slug}/media/{media_id}` | write on the Space | Deletes an image no longer in use (409 if it is in a post or is the cover) | 204 |
| POST | `/blogs/{slug}/cover-image` | user | Blog cover (owner only) | 201; upload |
| PATCH | `/blogs/{slug}/cover-image` | user | Content warning and alt text of the already uploaded cover |  |
| DELETE | `/blogs/{slug}/cover-image` | user | Removes the cover (the object in storage remains) |  |
| POST | `/blogs/{slug}/favicon` | user | Blog favicon (PNG, JPEG, WEBP; max 512 KiB; no moderation) | 201; upload |
| DELETE | `/blogs/{slug}/favicon` | user | Removes the favicon and deletes the object from storage |  |
| GET | `/users/me/media` | user | Media libraries of all my blogs (owner or collaborator), with the blog each belongs to |  |

## platform  (core)

Platform settings (single row): limits, reserved names, moderation threshold, footer. They are edited from `/admin/config` (core module `admin`).

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| GET | `/config` | public | Public instance configuration: language, registrations, captcha, access providers, deployment mode, `primary_blog_slug` (solo) | |
| GET | `/footer` | public | Platform footer in raw Markdown (columns 1-2 can be overridden per blog) |  |
| GET | `/site-texts` | public | UI texts overridden by administrators for the language `?locale=` (empty if none); the frontend merges them with the defaults |  |

## posts  (core)

Posts: drafts, review, publishing and scheduling, translations, tags, cited media and links, permalinks.

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| POST | `/blogs/{slug}/posts` | write on the Space | Creates a post (draft) | 201 |
| GET | `/blogs/{slug}/posts` | public | Blog posts: published ones for everyone, drafts and scheduled ones for users who can write (`?locale=`) |  |
| GET | `/blogs/{slug}/search` | public | Searches the blog's published posts (`?q=` max 200 characters → 422, `limit`, `offset`) | rate limit 120/1 min per ip |
| GET | `/blogs/{slug}/posts/{post_slug}` | public | Resolves the public permalink /{blog}/{post} |  |
| GET | `/blogs/{slug}/media-bibliography` | public | Images cited in published posts, grouped by URL |  |
| GET | `/blogs/{slug}/links-bibliography` | public | Links cited in published posts, grouped by URL |  |
| GET | `/posts/{post_id}` | public | Post details by id (404 if not visible) |  |
| PATCH | `/posts/{post_id}` | user | Edits a post |  |
| POST | `/posts/{post_id}/translations` | user | Adds a translation of a post | 201 |
| GET | `/posts/{post_id}/translations` | public | Published translations of a post (for the language selector) |  |
| POST | `/posts/{post_id}/submit-for-review` | user | Sends a draft for review |  |
| POST | `/posts/{post_id}/return-to-draft` | user | The reviewer or the owner returns a post under review to draft |  |
| POST | `/posts/{post_id}/publish` | user | Publishes (immediately or scheduled) |  |
| POST | `/posts/{post_id}/unpublish` | user | Takes a published (or scheduled) post out of publication: it returns to draft |  |
| DELETE | `/posts/{post_id}` | user | Permanently deletes a post (with its comments, notes, fragments and translations of that post only) | 204 |
| GET | `/users/me/posts` | user | All my posts (any status) across all my blogs, newest first |  |
| GET | `/users/me/links` | user | Links cited in posts (any status) across all my blogs, grouped by URL |  |
| GET | `/seo/crawl-directives` | public | Paths to exclude in robots.txt (opt-out per blog and per post; search engines and AI crawlers set separately) |  |
| GET | `/seo/sitemap-entries` | public | Blogs and posts that can be indexed, for sitemap.xml |  |
| GET | `/seo/sitemap` | public | Page (`offset`, `limit` max 5000) of the portal's indexable URLs: home, site pages, Spaces and posts, with translations as hreflang alternatives |  |
| GET | `/blogs/{slug}/sitemap` | public | Indexable URLs of a public Space: home, posts (translations as hreflang alternatives), static pages and publications; 404 if the Space is not public or has excluded search engines |  |
| GET | `/blogs/{slug}/export` | user | ZIP with all the blog content in open formats (owner only); image binaries stay in storage | raw |

## profiles  (core)

User profile: public and private personal details, avatar, social links, three-step email change, user directory, a user's public blogs and posts.

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| GET | `/users` | public | Public user directory (`q`, `locale`, `interest`, `sort` = new\|followers, `limit` max 100, `offset`) |  |
| GET | `/users/me` | user | Private profile of the owner (with the email) |  |
| PATCH | `/users/me` | user | Updates the profile; absent fields stay unchanged, "" clears text fields. In solo, a non-empty `interests` list is rejected (400) | |
| GET | `/users/{username}` | public | Public profile, by username or by verified personal domain |  |
| POST | `/users/me/avatar` | user | Avatar (PNG, JPEG, WEBP; max 2 MiB) | rate limit 20/10 min per user; upload |
| DELETE | `/users/me/avatar` | user | Removes the avatar | 204 |
| POST | `/users/me/social-links` | user | Adds a social link (max 5) | 201 |
| DELETE | `/users/me/social-links/{link_id}` | user | Removes a social link | 204 |
| POST | `/users/me/email/request` | user | Step 1/3 of the email change: code sent to the current mailbox | 202; rate limit 5/10 min per user |
| POST | `/users/me/email/verify-current` | user | Step 2/3: verifies the code from the current mailbox and sends one to the new mailbox | 202; rate limit 10/10 min per user |
| DELETE | `/users/me/email/request` | user | Cancels the ongoing email change, at any step | 204 |
| POST | `/users/me/email/verify-new` | user | Step 3/3: verifies the code from the new mailbox and applies the change | rate limit 10/10 min per user |
| GET | `/users/{username}/blogs` | public | The user's public blogs; blogs with an alias different from the username are not listed |  |
| GET | `/users/{username}/posts` | public | The user's published posts, only those signed with the username (`limit` max 50, `offset`) |  |

## auth_mfa  (optional)

Two-factor authentication: TOTP app or email code, challenge at login.

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| POST | `/auth/mfa/verify` | public | Completes the login with the MFA code; the refresh token travels in an httpOnly cookie | rate limit 20/5 min per ip; raw |
| POST | `/auth/mfa/totp/setup` | user | Generates a new TOTP secret (active only after confirmation) |  |
| POST | `/auth/mfa/totp/confirm` | user | Confirms the secret with a code and enables app-based MFA | 204; rate limit 20/5 min per ip |
| POST | `/auth/mfa/email/setup` | user | Sends a code to your email to confirm email-based MFA | 202; rate limit 5/5 min per user |
| POST | `/auth/mfa/email/confirm` | user | Confirms the received code and enables email-based MFA | 204; rate limit 20/5 min per ip |
| POST | `/auth/mfa/disable` | user | Disables MFA: without `code` 202 (with the email method it sends the code); with a valid `code` 204 | rate limit 20/5 min per ip; raw |

## auth_sso  (optional)

Sign-in with external providers (Google, Microsoft, GitHub, LinkedIn) and account linking by email.

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| GET | `/auth/sso/{provider}/login` | public | Starts sign-in with a provider: redirect to the authorization (state and PKCE in an httpOnly cookie) | rate limit 30/5 min per ip; raw |
| GET | `/auth/sso/{provider}/callback` | public | Completes sign-in: session and redirect to the frontend, or the MFA challenge to link an existing account | rate limit 30/5 min per ip; raw |

## branding  (optional)

Portal identity and SEO: icon and favicon (from a single image), share image, title and description per language, search engine verification, additional meta tags and indexing block.

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| GET | `/seo/site` | public | Portal identity and meta tags in the requested language (`?locale=`) or in the best available one: icons, share image, title, description, verifications, indexing block |  |
| GET | `/admin/seo` | role: super admin, administrator | Portal identity and SEO settings, with the uploaded images |  |
| PUT | `/admin/seo` | role: super admin, administrator | Sets the site name, texts per language, search engine verifications, additional meta tags and indexing block (replaces the whole set; absent fields are cleared) |  |
| POST | `/admin/branding/icon` | role: super admin, administrator | Portal icon and favicon from a single square image (PNG, JPEG, WEBP; max 2 MiB; at least 128 px): 16, 32, 48, 180, 192 and 512 px versions are derived, as far as the source size allows | 201; upload |
| DELETE | `/admin/branding/icon` | role: super admin, administrator | Removes the portal icon (defaults are restored) and deletes the files from storage |  |
| POST | `/admin/branding/social` | role: super admin, administrator | Image for share previews (PNG, JPEG, WEBP; max 2 MiB; at least 600 px wide; recommended 1200×630) | 201; upload |
| DELETE | `/admin/branding/social` | role: super admin, administrator | Removes the share image and deletes the file from storage |  |

## comments  (optional)

Post comments: registered users without moderation, anonymous users (captcha) with moderation, replies, reports, blocked authors; mode set per blog and per post.

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| POST | `/posts/{post_id}/comments` | public | Comments on a post (registered users: published immediately; anonymous: captcha and moderation) | 201; rate limit 30/1 min per ip |
| GET | `/users/{username}/comments` | public | The user's approved comments on public posts, only those signed with the username (`limit` max 50, `offset`) |  |
| GET | `/posts/{post_id}/comments` | public | Approved comments on a post |  |
| GET | `/posts/{post_id}/comments/pending` | user | Comments of a post awaiting moderation (owner, moderator, platform moderators) |  |
| GET | `/blogs/{slug}/comments` | user | Comments on all the blog's posts (`?status=` default pending, `?reported=true`) |  |
| POST | `/comments/{comment_id}/approve` | user | Approves a comment |  |
| POST | `/comments/{comment_id}/reject` | user | Rejects a comment |  |
| POST | `/comments/{comment_id}/report` | user | Reports the comment to the platform moderators, with a note |  |
| POST | `/comments/{comment_id}/block-author` | user | Blocks the comment's author in the blog and rejects the comment |  |
| GET | `/blogs/{slug}/blocked` | user | Authors blocked by the blog |  |
| DELETE | `/blogs/{slug}/blocked/{block_id}` | user | Unblocks an author | 204 |
| GET | `/admin/comments` | role: super admin, administrator, moderator | Comments on all blogs (`status`, default pending; `reported=true` for reported ones; `q`) |  |

## custom_domains  (optional)

The user's personal domain, verified with a TXT record in DNS: grants the "bronze" verification badge and makes the profile reachable from the domain too.

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| POST | `/users/me/domain` | user | Registers (or replaces) the domain, pending verification, and returns the TXT record to publish | rate limit 20/1 h per user |
| POST | `/users/me/domain/verify` | user | Queries DNS: if the TXT record matches, verifies the domain and grants the bronze badge | rate limit 5/10 min per user |
| DELETE | `/users/me/domain` | user | Removes the domain (also if verified) | 204 |

## discovery  (optional)

Discovery: portal feed, trends, global search, Space directory, following blogs and users, blog overview, reads, mentions.

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| GET | `/feed/posts` | public | Portal feed, newest first; by default only articles from `blog` Spaces (`kind` = blog\|wiki\|docs, `locale`, `tag`, `category`, `following=true`, `limit` max 50, `offset`) |  |
| GET | `/feed/locales` | public | The top 5 languages by number of posts in the feed |  |
| GET | `/feed/trending` | public | Most used tags in the last `days` days (max 90) |  |
| GET | `/search/posts` | public | Global search in title and content (`q` max 200 characters → 422, `limit` max 50, `offset`) | rate limit 120/1 min per ip |
| GET | `/blogs` | public | Public Space directory (`kind` = blog\|wiki\|docs, `q`, `locale`, `sort` = active\|new\|followers, `limit` max 100, `offset`) |  |
| POST | `/blogs/{slug}/follow` | user | Follows a blog (idempotent) | 204 |
| DELETE | `/blogs/{slug}/follow` | user | Stops following a blog, even if it is no longer visible | 204 |
| GET | `/blogs/{slug}/followers` | public | Usernames of the blog's followers |  |
| POST | `/users/{username}/follow` | user | Follows a user (idempotent) | 204 |
| DELETE | `/users/{username}/follow` | user | Stops following a user | 204 |
| GET | `/users/{username}/followers` | public | Who follows the user |  |
| GET | `/users/{username}/following` | public | Who the user follows |  |
| GET | `/users/me/follow-stats` | user | How many people follow me in total: users and followers of my blogs |  |
| GET | `/blogs/{slug}/overview` | user | Overview (counts, reads over the last 30 days, storage): owner and members |  |
| GET | `/blogs/{slug}/mentionable-users` | write on the Space | Suggestions for @mentions: owner, collaborators and followers (`q`, `limit` max 25) |  |
| POST | `/posts/{post_id}/read` | public | Counts a read (aggregated per day, no reader data) | 204 |

## fragments  (optional)

Fragments: portions of text highlighted and saved by readers on a published post, as a personal collection.

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| POST | `/posts/{post_id}/fragments` | user | Saves a fragment | 201 |
| GET | `/posts/{post_id}/fragments` | user | Fragments the user has already saved on this post |  |
| GET | `/users/me/fragments` | user | The user's fragment collection, newest first |  |
| PATCH | `/fragments/{fragment_id}` | user | Changes the visibility of a fragment |  |
| DELETE | `/fragments/{fragment_id}` | user | Deletes a fragment | 204 |

## gdpr  (optional)

GDPR rights: export of your own data (art. 20), account deletion by anonymization (art. 17), and a queue of requests with legal deadlines.

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| GET | `/users/me/export-data` | user | Copy of your data in a single JSON file (GDPR art. 20) | rate limit 5/1 h per user |
| DELETE | `/users/me` | user | Deletes the account (GDPR art. 17): personal data deleted, shared content anonymized | 204; rate limit 5/10 min per user |
| GET | `/admin/gdpr` | role: super admin, administrator | Request register (`status`): open and approved requests first, by deadline |  |
| POST | `/admin/gdpr` | role: super admin, administrator | Enters a request received outside the platform on behalf of a user | 201 |
| POST | `/admin/gdpr/{request_id}/approve` | role: super admin, administrator | Approves; a deletion must be approved by an administrator other than the one who entered it |  |
| POST | `/admin/gdpr/{request_id}/reject` | role: super admin, administrator | Reasoned rejection |  |
| POST | `/admin/gdpr/{request_id}/execute` | role: super admin, administrator | Executes: an export returns the data JSON; an approved deletion anonymizes the account |  |

## git_sync  (optional)

Sync of a Space's texts to a branch of a remote Git repository (GitHub, GitLab, Forgejo...): export only, via HTTPS with a personal token or via SSH with a deploy key.

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| GET | `/blogs/{slug}/git-sync` | user | Configuration and last outcome of the Git sync (owner only; the token is never returned) |  |
| PUT | `/blogs/{slug}/git-sync` | user | Sets repository, branch and access: personal token via HTTPS, or SSH deploy key generated by the platform with a pinned host key (branch other than the repository's default branch) |  |
| DELETE | `/blogs/{slug}/git-sync` | user | Removes the sync and deletes the saved token (the remote repository is not touched) | 204 |
| POST | `/blogs/{slug}/git-sync/run` | user | Queues a sync now; the outcome appears in `last_status` (poll with GET) | 202 |
| GET | `/admin/git-sync/known-hosts` | role: super admin | SSH hosts whose expected fingerprints the platform knows (GitHub, GitLab...): detected keys are pinned only if they match |  |
| PUT | `/admin/git-sync/known-hosts` | role: super admin | Adds or updates the expected fingerprints of a host (e.g. when the provider rotates its keys); configurations already pinned do not change until the user saves them again |  |
| DELETE | `/admin/git-sync/known-hosts/{host}/{port}` | role: super admin | Removes a host from the list: trust on first use applies again, and so does the user's optional fingerprint | 204 |

## graph  (optional)

Link map: graph of posts, Spaces, tags, categories and links between posts, computed by Postgres for the user's own Spaces only (or for the whole platform, for administrators).

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| GET | `/graph` | user | Graph of links between posts, Spaces, tags, categories and links: of your own Spaces (`scope=mine`, including drafts if `drafts=true`) or, for administrators, of everything public (`scope=platform`); filters `blog`, `kind`, `limit` (max 800) |  |

## interests  (optional)

Public list of platform interests, chosen by users in their profile and used to filter the user directory.

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| GET | `/interests` | public | Platform interests selectable in the profile. Returns `[]` in solo | |

## legal  (optional)

Privacy notice and cookie policy: multilingual templates that adapt to the active features of the instance, controller details, editing and publication as site pages (/p/privacy, /p/cookie-policy).

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| GET | `/admin/legal` | role: super admin, administrator | Legal documents: controller details, detected active features, and the status of each document per language (completeness, publication, to be republished) |  |
| PUT | `/admin/legal/settings` | role: super admin, administrator | Sets the controller details; documents already published are rewritten with the new values (rejected if it would leave empty a field they use) |  |
| GET | `/admin/legal/{doc}/{locale}` | role: super admin, administrator | One document (`privacy` or `cookie`) in one language: source, template, resolved text, and what is missing to publish it |  |
| PUT | `/admin/legal/{doc}/{locale}` | role: super admin, administrator | Saves the title and text of a document (creates the language if missing); does not publish |  |
| POST | `/admin/legal/{doc}/{locale}/reset` | role: super admin, administrator | Restores the supplied template (it/en) or copies the text of another language to start a translation |  |
| POST | `/admin/legal/{doc}/{locale}/publish` | role: super admin, administrator | Publishes the resolved text as a site page (/p/privacy, /p/cookie-policy); rejected if data is missing or the template has errors |  |
| POST | `/admin/legal/{doc}/{locale}/unpublish` | role: super admin, administrator | Withdraws the published page (the source text remains) |  |
| DELETE | `/admin/legal/{doc}/{locale}` | role: super admin, administrator | Deletes a language added by the administrator (the template languages, it and en, are restored but not deleted) | 204 |
| GET | `/legal/public` | public | Published legal documents, in the requested language (`?locale=`) or in the best available one: used by the links in the site footer |  |

## link_preview  (optional)

Open Graph preview of links pasted into the editor and of cards in posts (server-side fetch, protected against SSRF).

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| GET | `/link-preview` | public | Open Graph preview of a URL (`?url=`) | rate limit 30/1 min per ip |

## newsletter  (optional)

Newsletters per blog and platform digest: double opt-in subscription, unsubscription and deletion without login, automatic notification of new posts, and manual campaigns.

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| POST | `/newsletter/subscribe` | public | Subscription with double opt-in; the response is always the same (no email enumeration) | 202; rate limit 5/1 h per ip |
| GET | `/newsletter/confirm` | public | Confirms the subscription (`?token=`); idempotent | rate limit 30/5 min per ip |
| POST | `/newsletter/unsubscribe` | public | Unsubscribes with the signed link received by email, without login | rate limit 30/5 min per ip |
| POST | `/newsletter/unsubscribe/delete` | public | Deletes your data from the list (GDPR art. 17), without login | rate limit 30/5 min per ip |
| GET | `/blogs/{slug}/newsletter/stats` | write on the Space | Subscribers by status |  |
| GET | `/blogs/{slug}/newsletter/campaigns` | write on the Space | The blog's campaigns, newest first |  |
| POST | `/blogs/{slug}/newsletter/campaigns` | write on the Space | Creates a manual campaign (immediate or scheduled) | 201; rate limit 20/1 h per user |
| PATCH | `/blogs/{slug}/newsletter/campaigns/{campaign_id}` | write on the Space | Edits a campaign that is still scheduled |  |
| DELETE | `/blogs/{slug}/newsletter/campaigns/{campaign_id}` | write on the Space | Cancels a campaign that is still scheduled |  |
| GET | `/blogs/{slug}/newsletter/settings` | write on the Space | Automatic sending, sender name and email banner |  |
| PATCH | `/blogs/{slug}/newsletter/settings` | write on the Space | Updates the settings (absent field = unchanged, `null` = cleared) |  |
| GET | `/admin/newsletter/stats` | role: super admin, administrator | Digest subscribers by status |  |
| GET | `/admin/newsletter/settings` | role: super admin, administrator | Digest sender name and banner |  |
| PATCH | `/admin/newsletter/settings` | role: super admin, administrator | Updates the digest settings |  |
| GET | `/admin/newsletter/campaigns` | role: super admin, administrator | Digest campaigns |  |
| POST | `/admin/newsletter/campaigns` | role: super admin, administrator | Creates a digest campaign | 201 |
| PATCH | `/admin/newsletter/campaigns/{campaign_id}` | role: super admin, administrator | Edits a digest campaign that is still scheduled |  |
| DELETE | `/admin/newsletter/campaigns/{campaign_id}` | role: super admin, administrator | Cancels a digest campaign that is still scheduled |  |

## notes  (optional)

Footnotes of posts, the blog's notes library (duplicates, merge, BibTeX) and bibliographies (notes field of posts).

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| GET | `/blogs/{slug}/notes` | user | The blog's notes library with "cited in" and duplicates (`?q=` filters on the text); owner and collaborators |  |
| POST | `/blogs/{slug}/notes` | write on the Space | Creates a library note | 201 |
| GET | `/blogs/{slug}/notes/export.bib` | user | Exports the library to BibTeX | raw |
| POST | `/blogs/{slug}/notes/import` | write on the Space | Imports BibTeX (entries already present are skipped) | 201 |
| PATCH | `/blogs/{slug}/notes/{note_id}` | write on the Space | Edits a note; the text is also updated in the posts that cite it |  |
| DELETE | `/blogs/{slug}/notes/{note_id}` | write on the Space | Deletes a note not cited by any post (409 otherwise) | 204 |
| POST | `/blogs/{slug}/notes/{note_id}/merge` | write on the Space | Merges the note into another: citations move to the target |  |
| GET | `/blogs/{slug}/bibliography` | public | Bibliography of published posts, grouped by identical text |  |
| GET | `/users/me/bibliography` | user | Notes of all posts (any status) across all my blogs |  |

## pages  (optional)

Static pages: site pages (administrators only) and the pages of blogs that enable them (`static_pages_enabled` field of the blog).

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| GET | `/blogs/{slug}/pages` | public | Blog pages in one language |  |
| GET | `/blogs/{slug}/pages/by-id/{page_id}` | write on the Space | Page by id (editor, drafts included) |  |
| GET | `/blogs/{slug}/pages/{key}` | public | Page by slug and language |  |
| POST | `/blogs/{slug}/pages` | write on the Space | Creates a page | 201 |
| POST | `/blogs/{slug}/pages/{key}/translations` | write on the Space | Adds a translation | 201 |
| GET | `/blogs/{slug}/pages/{key}/translations` | public | Published translations of a page |  |
| PATCH | `/blogs/{slug}/pages/{key}` | write on the Space | Edits a page |  |
| DELETE | `/blogs/{slug}/pages/{key}` | write on the Space | Deletes a page | 204 |
| GET | `/pages` | public | Site pages in one language (`?locale=`; `?q=` searches title and slug) |  |
| GET | `/pages/{key}` | public | Site page by slug and language |  |
| POST | `/pages` | role: super admin, administrator | Creates a site page | 201 |
| POST | `/pages/{key}/translations` | role: super admin, administrator | Adds a translation | 201 |
| GET | `/pages/{key}/translations` | public | Published translations of a site page |  |
| PATCH | `/pages/{key}` | role: super admin, administrator | Edits a site page |  |

## publications  (optional)

Publications: collections of posts as chapters, public index and explicit order (`publication_id`/`publication`/`chapter_order` fields of posts).

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| GET | `/blogs/{slug}/publications` | public | Blog publications with chapter counts |  |
| POST | `/blogs/{slug}/publications` | write on the Space | Creates a publication | 201 |
| GET | `/blogs/{slug}/publications/{ref}` | public | Chapter index of a publication |  |
| PATCH | `/blogs/{slug}/publications/{ref}` | write on the Space | Edits name, title or description |  |
| PUT | `/blogs/{slug}/publications/{ref}/order` | write on the Space | Sets the chapter order |  |
| DELETE | `/blogs/{slug}/publications/{ref}` | write on the Space | Deletes the publication (posts remain, without a publication) | 204 |
| GET | `/users/me/publications` | user | Publications of all my blogs (owner or collaborator), with chapter counts |  |

## reports  (optional)

Reports of blogs and posts by registered readers, one per target, to be closed from the administration routes.

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| POST | `/blogs/{slug}/report` | user | Reports a blog (once only; not your own) | 201; rate limit 10/1 h per user |
| POST | `/posts/{post_id}/report` | user | Reports a published post (once only; not your own content) | 201; rate limit 10/1 h per user |
| GET | `/admin/blogs/{blog_id}/reports` | role: super admin, administrator | Open reports on a blog and its posts, with reason, note and reporter |  |
| POST | `/admin/blogs/{blog_id}/action` | role: super admin, administrator | Action from the reports panel (note required): closes the blog's open reports |  |

## taxonomy  (optional)

Per-blog post categories (`category_id`/`category` fields in post create, update and response).

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| GET | `/blogs/{slug}/categories` | public | Blog categories |  |
| POST | `/blogs/{slug}/categories` | write on the Space | Creates a category |  |
| PATCH | `/blogs/{slug}/categories/{category_id}` | write on the Space | Edits the name or slug of a category |  |
| DELETE | `/blogs/{slug}/categories/{category_id}` | write on the Space | Deletes a category |  |

## wiki  (optional)

Wiki and documentation Spaces: page tree, internal links [[page]] with backlinks, revision history with diffs.

| Verb | Path | Access | Description | Notes |
|---|---|---|---|---|
| GET | `/blogs/{slug}/outline` | public | Space outline (`?locale=`): readers see published posts, writers see all of them |  |
| PUT | `/blogs/{slug}/outline` | write on the Space | Reorders and moves pages (wiki/docs Spaces only); 409 if the list does not match the Space's posts |  |
| GET | `/posts/{post_id}/revisions` | user | Revision history, newest first (users who can write in the Space) |  |
| GET | `/posts/{post_id}/revisions/{revision}` | user | One revision with its full content |  |
| GET | `/posts/{post_id}/revisions/{revision}/diff` | user | Line-by-line differences between the revision and `?against=` (default: the previous one) |  |
| GET | `/posts/{post_id}/backlinks` | public | Pages of the same Space that link to this one with [[link]] (only those visible to the caller) |  |

## How this file is maintained

The tables are generated from the `api.toml` files (`summary` field of each route). When you add or change a route, write a
`summary` that explains **why the route exists**, not only what it does, and regenerate this document. Routes without a `summary`
appear with the identifier only.

The test module `sandbox` (used in tests) is not included.
