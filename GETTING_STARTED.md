# Local startup — first test guide

The fastest way to bring up the whole stack locally and run a first test, from registration to publishing a post. For details on each component see [README.md](README.md) (overview and structure), [docs/API.md](docs/API.md) (endpoint reference) and the READMEs of [backend](docs/BACKEND.md) and [frontend](frontend/README.md).

## Prerequisites

- [Podman](https://podman.io/) (rootless is fine).
- A compose provider for Podman: native `podman compose` is not guaranteed on every installation. If `podman compose version` fails, install `podman-compose`.

  ```bash
  sudo dnf install podman-compose   # Fedora (reference development environment)
  # alternatively, if the system python has no pip module:
  python3 -m venv /tmp/podman-compose-venv && /tmp/podman-compose-venv/bin/pip install podman-compose
  # then use /tmp/podman-compose-venv/bin/podman-compose in place of "podman compose"
  ```

Nothing else needs to be installed by hand. By default the stack is the lean **solo** profile: PostgreSQL, backend and frontend run in containers. Redis, NATS, Garage (S3 storage), image moderation and the separate worker belong to the **platform** profile, which you can add later (step 2).

## 1. Configuration

```bash
scripts/gen-env.sh      # creates .env from .env.example with random secrets and prints the Super Admin password
```

The backend **does not start** with the placeholders in `.env.example` (`NOCT_JWT_SECRET`), and it does not create the Super Admin with the example password. For this reason, use the script: it replaces all secrets (JWT, TOTP secrets key, Postgres and Super Admin passwords, Garage credentials). The other settings are fine as they are for a first local test. For real installations see [docs/INSTALL.md](docs/INSTALL.md). Variables to know:

- `NOCT_DEPLOYMENT_MODE` — `.env.example` sets `solo`: one owner, registration closes after the first user (who becomes Super Admin), blogs live at `/<slug>` (`NOCT_SUBDOMAINS_ENABLED=false`), files are stored on a volume, and there is no Redis, NATS or image moderation. `platform` is the multi-user stack (see [Deployment profiles](docs/CONFIGURATION.md#deployment-profiles-solo-and-platform)). This guide follows solo and marks the differences for platform.
- `NOCT_S3_ACCESS_KEY_ID` / `NOCT_S3_SECRET_ACCESS_KEY` / `NOCT_GARAGE_RPC_SECRET` / `NOCT_GARAGE_ADMIN_TOKEN` — Garage credentials, needed only for platform (format `GK` + 24 hex / 64 hex). They are commented out in `.env.example`; `scripts/gen-env.sh --platform` generates and uncomments them (it does not change the mode, storage or subdomain variables: set those by hand).
- `NOCT_JWT_SECRET` / `NOCT_SECRETS_KEY` — token signing, and the key that encrypts TOTP secrets at rest (do not lose or rotate it). The backend rejects placeholders and secrets shorter than 32 bytes.
- `NOCT_SUPER_ADMIN_USERNAME`/`_EMAIL`/`_PASSWORD` — if all three are set (the script sets the password), the backend creates this account as Super Admin on first startup. No manual step is needed to enter the administration area (step 4).

## 2. Starting the stack

```bash
podman compose up -d --build
# or, if you do not have the native podman compose:
podman-compose up -d --build
```

The first run takes a few minutes (image builds, download of PostgreSQL, and compilation of the Rust backend). `podman compose ps` shows only `postgres`, `backend` and `frontend`. When the start is complete, these services are reachable:

- Frontend (dashboard and administration included): <http://localhost:3000>
- Backend — OpenAPI schema: <http://localhost:8000/api/v1/openapi.json>
- Backend — health check: <http://localhost:8000/api/v1/health> (readiness: `/api/v1/health/ready`)
- Uploaded files (local storage): `http://localhost:8000/storage/...`

To test the full **platform** stack instead, create `.env` with `scripts/gen-env.sh --platform`, set `NOCT_DEPLOYMENT_MODE=platform`, `NOCT_SUBDOMAINS_ENABLED=true` and `NOCT_STORAGE_BACKEND=s3`, then run:

```bash
podman compose -f compose.yaml -f compose.platform.yaml --profile platform up -d --build
```

`garage-bootstrap` is a one-shot job (`noct-ctl garage-bootstrap`: Garage layout, key and bucket). It exits on its own with exit code 0, so it is normal that it does not show as `Up`. Extra services: Garage API <http://localhost:3900> and public reads (avatars/media) <http://localhost:9000>. Add `--profile dev-mail` for MailHog (<http://localhost:8025>, with `NOCT_SMTP_HOST=mailhog`). The reason for two files is in [docs/INSTALL.md](docs/INSTALL.md#22b-quickstart-platform).

Quick check that the backend responds and sees the database:

```bash
curl http://localhost:8000/api/v1/health
# {"status":"ok"}
```

## 3. Database migrations

They run automatically at backend startup. The active modules apply their own migrations under an advisory lock, which is safe even with multiple replicas. To see their status:

```bash
podman compose exec backend noct-ctl migrate status
```

## 4. First user

Nothing to do: the backend creates a Super Admin on its own at first startup, from `NOCT_SUPER_ADMIN_USERNAME`/`_EMAIL`/`_PASSWORD`. The step is idempotent, so a later restart does not touch the account. Sign in directly at <http://localhost:3000> with the Super Admin's **email** and the password printed by `gen-env.sh`. The administration entries appear in the dashboard menu.

If you prefer your own account over the preset (or you emptied those variables), register a normal user from the interface (step 5) or via API, then promote it by hand:

```bash
podman exec -it $(podman ps -qf "name=postgres") \
  psql -U admin -d notturni -c \
  "UPDATE users SET platform_role='super_admin' WHERE username='<your-username>';"
```

Sign in again after the promotion. The role is read at login, not updated in a session that is already open. In solo mode (the default of `.env.example`) this step is not needed: the first registered user is already Super Admin, and registration is then closed. Other people get an account from the Super Admin, in `/admin/utenti/nuovo` or with `noct-ctl user create` (see [Collaborators in solo mode](docs/CONFIGURATION.md#collaborators-in-solo-mode)).

For direct API access without going through the editor or admin areas (core engine, scripts), run `podman compose exec backend noct-ctl token create --name core-engine`. The value is shown only once. See [docs/API.md](docs/API.md#api-tokens-core-engine--direct-user-access).

## 5. First test run — web interface

The UI labels below are in Italian. The English gloss follows each label on first use.

1. Open <http://localhost:3000>. Which path applies depends on the `NOCT_SUPER_ADMIN_*` variables in `.env`:
   - **Default `.env`** (generated by `scripts/gen-env.sh` from `.env.example`, all three variables set): the owner already exists, and it owns the primary blog. The **Registrati** (Sign up) form is closed. Sign in with the Super Admin's email and the password printed by the script (step 4), then go to step 2.
   - **Empty `NOCT_SUPER_ADMIN_*` variables**: click **Registrati** (Sign up) and create an account. In `solo` the first account becomes Super Admin and owns the primary blog. Registration then closes.
2. You are taken to the dashboard. Click **Nuovo blog** (New blog), then choose a slug (at least 4 characters in platform mode; any length in solo mode) and a title.
3. Open the blog you just created, go to the **Post** tab (Posts), and click **Nuovo post** (New post). Write a title, a slug and content (Markdown), then create the draft.
4. In the post editor, click **Pubblica** (Publish).
5. Go back to the list. The post is now public. You can also check it via API: `curl http://localhost:8000/api/v1/blogs/<blog-slug>/posts`.
6. Try the **Aspetto** (Appearance) tab to change the blog palette. Try **Impostazioni** (Settings) to open comments to users who are not registered.
7. Go to **Profilo** (Profile) in the top menu. Set a bio, upload an avatar, and add a social link. If the user you logged in with is Super Admin or Administrator, the same menu also shows these entries. With the defaults of step 4, the Super Admin is a separate account from the one you just registered (in solo, the first registered account already is the Super Admin). Log in again with the `NOCT_SUPER_ADMIN_*` credentials, or with the user you promoted by hand if you followed the alternative in step 4.
   - **Pagine** (Pages): create pages such as "About us" or "Privacy", with the same editor as posts.
   - **Utenti** (Users): roles and activation. Only in `platform` mode; in solo the entry is **Collaboratori** (Collaborators), to create accounts.
   - **Impostazioni** (Settings): in solo it also shows the read-only infrastructure summary. `/admin/setup` is a wizard that only prints `.env` and Helm values.
   - **Tutti i blog** (All blogs): list of all blogs on the platform, with search and suspension.
   - **Moderazione** (Moderation): list of all posts on the platform, with search and hide/show for each post.

   Each section has a search field.
8. Try the theme selector at the top right (labels *chiaro*, *scuro*, *automatico*: light, dark, automatic). In automatic mode, the browser asks for geolocation permission to compute sunrise and sunset. If you deny it, the selector falls back to the system preferences.

## 6. First test run — direct API

This is an alternative to step 5 (or a complement to it). It helps you understand what happens under the frontend. Full reference in [docs/API.md](docs/API.md).

```bash
# registration + login (in solo only the first account can register)
curl -s -X POST http://localhost:8000/api/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username":"prova","email":"prova@example.com","password":"Password123!"}'

TOKEN=$(curl -s -X POST http://localhost:8000/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"prova@example.com","password":"Password123!"}' \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['access_token'])")

# create a blog
curl -s -X POST http://localhost:8000/api/v1/blogs \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"slug":"blog-di-prova","title":"Blog di prova"}'

# create and publish a post
POST_ID=$(curl -s -X POST http://localhost:8000/api/v1/blogs/blog-di-prova/posts \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"slug":"primo-post","title":"Ciao mondo","content":"# Funziona!"}' \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['id'])")

curl -s -X POST http://localhost:8000/api/v1/posts/$POST_ID/publish \
  -H "Authorization: Bearer $TOKEN"

# public check
curl -s http://localhost:8000/api/v1/blogs/blog-di-prova/posts
```

## 7. Stopping and cleaning up the environment

```bash
podman compose down          # stops the containers, keeps the data (volumes)
podman compose down -v       # stops and also deletes the data (start from scratch)
```

## 8. Common problems

- **`podman compose` not found / "looking up compose provider failed"** — install `podman-compose` (see Prerequisites) and use `podman-compose` in place of `podman compose` in all the commands above.
- **The backend does not start, and the log mentions the manifest or the configuration** — the backend validates everything at startup. It stops with a message that names the variable or the module (for example, a `NOCT_JWT_SECRET` that is too short). Check `podman compose logs backend`.
- **An authenticated call returns 401 right after login** — check that `NOCT_CORS_ORIGINS` includes the origin from which you open the frontend. Over local http, also check that `NOCT_SESSION_COOKIE_SECURE=false`.
- **The frontend cannot call the backend (network or CORS errors in the console)** — this happens if the frontend runs at a URL other than `http://localhost:3000`. `NOCT_CORS_ORIGINS` in `.env` must include that URL.
- **Avatar, media or appearance upload does not work** — in solo the files go to local storage: check that `NOCT_LOCAL_STORAGE_PUBLIC_URL` is the URL the browser reaches (`http://localhost:8000/storage` locally). With the S3 backend (platform), check that `garage` is `Up` and that `garage-bootstrap` finished successfully (`podman ps -a`). Otherwise run `podman compose logs garage-bootstrap`. Usually `NOCT_S3_ACCESS_KEY_ID`/`_SECRET_ACCESS_KEY` do not follow the Garage format; see `.env.example`. An image that is uploaded but not visible in the browser can be checked at <http://localhost:9000/avatars/...>.
- **No email arrives (MFA code, password reset)** — in solo `NOCT_SMTP_HOST` is empty, so nothing is sent. Use `--profile dev-mail` with `NOCT_SMTP_HOST=mailhog` and read <http://localhost:8025>, or recover a password with `noct-ctl user reset-password` (see [docs/CONFIGURATION.md](docs/CONFIGURATION.md#password-recovery-without-smtp)). Messages that cannot be sent end up in `podman compose exec backend noct-ctl dead-letters`.
- **You want to be sure background jobs run** — in solo the worker runs inside the backend: `podman compose logs -f backend`. On platform, follow `podman compose logs -f worker`. Newsletter campaigns, post copying, audit maintenance and backups all run through the worker.

## Next steps

Once you have verified that everything runs, to go deeper:

- [README.md](README.md) — overview, stack, repository structure.
- [docs/BACKEND.md](docs/BACKEND.md) — development, testing and operations of the Rust backend.
- [docs/API.md](docs/API.md) — every endpoint, with request and response examples and the authorization rules.
- [ROADMAP.md](ROADMAP.md) — full table of product specifications, their status, and the backlog of upcoming developments.
- [k8s/README.md](k8s/README.md) — Kubernetes deployment manifests (initial draft).
- [helm/notturni/README.md](helm/notturni/README.md) — Helm chart (solo by default, platform with values).
