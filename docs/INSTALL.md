# Installing Notturni

Step-by-step guides for each way to install the platform. For a first trial run locally, with the test path from
registration to publishing a post, see [GETTING_STARTED.md](../GETTING_STARTED.md); for the configuration of each
variable, see [`.env.example`](../.env.example); for day-to-day operations, see [BACKEND.md](BACKEND.md).

| Scenario | When to choose it | Section |
| --- | --- | --- |
| Podman / Docker Compose | trial, development, small instance on a single host | [2](#2-podman-and-docker-compose) |
| K3s (Kubernetes) | production on one node, expandable to a cluster | [3](#3-k3s) |
| VM Debian / Ubuntu | production without containers, system services | [4](#4-vm-without-containers-debian-and-ubuntu) |
| VM Rocky / AlmaLinux | as above, RHEL family with SELinux and firewalld | [5](#5-vm-without-containers-rocky-and-almalinux) |

The examples use `example.com` (domain) and `192.168.x.x` (networks): replace them with your own values.

## 1. Before you start (common to all solutions)

### 1.1 Architecture in brief

```text
browser ──► reverse proxy (TLS) ──┬─ /api/*      ──► noct-api    :8000 ──┬─► PostgreSQL 16+
                                  ├─ /storage/*  ──► noct-api            ├─► Redis 7+
                                  └─ everything else ► frontend :3000    ├─► NATS JetStream
                                                                         └─► storage (S3/Garage or filesystem)
                                                  noct-worker (no port): outbox, periodic jobs, consumers
```

`noct-api`, `noct-worker` and `noct-ctl` are three binaries from the same image/build. The **worker is required**
when NATS is used: without it, email (password reset, MFA), newsletters, backups and maintenance do not run.
The `moderation/` service (Python, image classification) is **optional**: without it,
`NOCT_MODERATION_SERVICE_URL` stays empty and images are not classified.

### 1.2 Requirements

| Resource | Minimum (trial / `solo`) | Recommended (platform) |
| --- | --- | --- |
| CPU / RAM | 2 vCPU / 2 GB | 4 vCPU / 4 GB (8 GB if you build on the VM) |
| Disk | 20 GB | 50 GB + dedicated data volume |
| Architecture | x86_64 or ARM64 | same |
| Inbound network | 80, 443 | 80, 443 (22 only from the management network) |

In every case you need: a domain with `A`/`AAAA` DNS records pointing to the server (for blogs on subdomains,
`username.example.com`, also a wildcard record `*.example.com`), an SMTP server for email
(password reset, MFA codes, newsletters; optional in solo, where recovery uses `noct-ctl user reset-password`) and, for wildcard certificates, a DNS provider with an API for the
DNS-01 challenge.

### 1.3 Secrets: `scripts/gen-env.sh`

The backend **refuses to start** with the placeholders from `.env.example` (`NOCT_JWT_SECRET`), and it does not
create the Super Admin with the example password. Create the `.env` file with random secrets:

```bash
git clone https://github.com/bytehawks-org/notturni.git && cd notturni
scripts/gen-env.sh            # creates .env (mode 600) and prints the Super Admin password
scripts/gen-env.sh --platform # same, plus the S3/Garage secrets (platform profile)
```

The script prints the Super Admin password **once, to the terminal**, and also stores it in `.env`
(`NOCT_SUPER_ADMIN_PASSWORD`, file mode 600). Copy it to a password manager. Do not run the script
where the terminal output is logged (CI logs, shared screen sessions). Change the password after the first login.

Then edit by hand the variables that depend on the installation:

| Variable | Typical value in production |
| --- | --- |
| `NOCT_INSTANCE_FQDN` | `example.com` |
| `NOCT_SUPER_ADMIN_EMAIL` | the administrator's email |
| `NOCT_DEPLOYMENT_MODE` | `platform` (multi-user) or `solo` (one owner, registration closed after the first user) |
| `NOCT_CORS_ORIGINS` | `https://example.com` |
| `NOCT_CORS_ORIGIN_REGEX` | `https://([a-z0-9-]+[.])?example[.]com` for blogs on subdomains (`[.]` and not `\.`: this also works in a systemd `EnvironmentFile`) |
| `NOCT_SESSION_COOKIE_SECURE` | `true` (with https; `false` only for trials over http) |
| `NOCT_SMTP_HOST` / `_PORT` / `_USER` / `_PASSWORD` / `_FROM_EMAIL` | your own SMTP provider |
| `NOCT_TRUSTED_PROXIES` | number of proxies in front of the API (1 with a single reverse proxy). **`0` if the API is exposed directly**: otherwise a client can forge `X-Forwarded-For` and bypass the per-IP limits |
| `NOCT_FRONTEND_REVALIDATE_URL` | internal URL of the frontend + `/api/revalidate` |
| `NOCT_BACKUP_S3_*` | **external** S3 storage for the daily backup (empty = backup disabled) |

Keep `.env` out of git and in a safe place: **`NOCT_SECRETS_KEY` must not be lost or rotated** (it encrypts the
TOTP secrets; without it, users with MFA can no longer sign in). After the first installation, change the Super
Admin password from the interface.

### 1.4 Common check at the end of the installation

```bash
curl -s https://example.com/api/v1/health/ready
# {"services":{"bus":{...},"cache":{...},"database":{...},"storage":{...}},"status":"ok"}
```

Then sign in at `https://example.com` with `superadmin` and the password printed by `gen-env.sh`
(**sign-in is by email**: `POST /api/v1/auth/login` with `{"email": …}`).

---

## 2. Podman and Docker Compose

[`compose.yaml`](../compose.yaml) defines the stack for **development and trials**: it publishes the service ports on the host. By default it starts the lean **solo** profile (PostgreSQL, backend with inline worker and local storage on a named volume, frontend). The components of the **platform** profile (Redis, NATS, Garage + bootstrap, the public-read proxy `s3-public`, image moderation, the separate worker) sit behind the compose profile `platform`; `smtp-relay` behind `mail-relay`; MailHog behind `dev-mail`. For what each profile means, see [Deployment profiles](CONFIGURATION.md#deployment-profiles-solo-and-platform).

### 2.1 Prerequisites

| | Podman | Docker |
| --- | --- | --- |
| Debian / Ubuntu | `sudo apt install podman podman-compose` | [docs.docker.com/engine/install](https://docs.docker.com/engine/install/) (`docker compose` plugin) |
| Fedora / Rocky / Alma | `sudo dnf install podman podman-compose` | same, Docker CE repository |
| Check | `podman compose version` | `docker compose version` |

If `podman compose` is not available, use `podman-compose` (same syntax). With Docker, replace
`podman compose` with `docker compose`: the file is the same. Git is needed to clone the repository.

### 2.2 Quickstart: solo (5 commands)

```bash
git clone https://github.com/bytehawks-org/notturni.git && cd notturni
scripts/gen-env.sh                      # random secrets; note the Super Admin password
podman compose up -d --build            # (docker compose up -d --build)
podman compose ps                       # only postgres, backend and frontend
curl -s http://localhost:8000/api/v1/health/ready
```

The first build of the Rust backend takes several minutes. `.env.example` is already set for solo (`NOCT_DEPLOYMENT_MODE=solo`, `NOCT_SUBDOMAINS_ENABLED=false`, `NOCT_STORAGE_BACKEND=localstorage`, no Redis, NATS, S3 or moderation). Reachable services:

| Service | URL |
| --- | --- |
| Frontend (dashboard and administration) | <http://localhost:3000> |
| API / OpenAPI | <http://localhost:8000/api/v1/openapi.json> |
| Uploaded files (local storage, served by the backend) | `http://localhost:8000/storage/...` |

Sign in with the Super Admin email and the password printed by `gen-env.sh`. Without SMTP no email is sent. To read test emails, add `--profile dev-mail` and set `NOCT_SMTP_HOST=mailhog`, `NOCT_SMTP_PORT=1025`, `NOCT_SMTP_USE_TLS=false`: MailHog is then at <http://localhost:8025>. The rest of the trial path is in [GETTING_STARTED.md](../GETTING_STARTED.md). Collaborator accounts, password recovery without SMTP and the other solo specifics are in [CONFIGURATION.md](CONFIGURATION.md#collaborators-in-solo-mode).

### 2.2b Quickstart: platform

podman-compose 1.6 fails with a `KeyError` on a `depends_on` that has a `condition` and points to a service of an inactive profile, even with `required: false`. The dependencies on the platform services therefore live in an override file:

```bash
podman compose -f compose.yaml -f compose.platform.yaml --profile platform up -d --build
```

This adds Redis, NATS, Garage (`garage-bootstrap` exits by itself with exit 0: this is normal), `s3-public`, the moderation service and the separate worker, and points the backend at them (S3 storage, Redis cache, NATS bus). `--profile platform` alone starts the containers but leaves the backend environment and dependencies in their solo form.

Generate `.env` with `scripts/gen-env.sh --platform`: besides the common secrets it generates and uncomments `NOCT_S3_ACCESS_KEY_ID`, `NOCT_S3_SECRET_ACCESS_KEY`, `NOCT_GARAGE_RPC_SECRET` and `NOCT_GARAGE_ADMIN_TOKEN`. The option does not change the mode: set `NOCT_DEPLOYMENT_MODE=platform`, `NOCT_SUBDOMAINS_ENABLED=true` and `NOCT_STORAGE_BACKEND=s3` by hand, and uncomment the other `NOCT_S3_*` lines you need (the compose files already point the backend to `garage`). Verified with podman 6.2.0 and podman-compose 1.6.0; Docker Compose is not verified here. Extra ports: Garage S3 API <http://localhost:3900>, public reads (avatars, media) <http://localhost:9000>.

### 2.3 Operations

```bash
podman compose logs -f backend                 # logs (platform: add worker)
podman compose exec backend noct-ctl migrate status
podman compose exec backend noct-ctl token create --name core-engine   # machine-to-machine API token
podman compose up -d --build backend            # update after a git pull (platform: add worker)
podman compose down                            # stop (data stays in the volumes)
podman compose down -v                         # stop and DELETE the data
```

### 2.4 From trial to a single-host instance

`compose.yaml` can also be used for a small public instance, provided that you:

1. **Put a reverse proxy with TLS** on the host (Caddy, nginx or Traefik; see the Caddy example in [4.9](#49-reverse-proxy-and-tls)),
   which forwards `/api/*` to `localhost:8000`, `/storage/*` (only with `localstorage`) to `localhost:8000`, and
   everything else to `localhost:3000`. Pass to the `frontend` service in `compose.yaml`, as environment variables
   (read at runtime), `NOCT_SITE_URL=https://example.com` and, to change the name, `NOCT_SITE_NAME`. Remove the
   `NEXT_PUBLIC_*` build arguments, which are meant for `localhost`.
2. **Close the unneeded ports**: remove (or bind to `127.0.0.1:`) the `ports:` entries of `postgres` and, if you run them, `redis`, `nats`,
   `garage`, `mailhog`, `smtp-relay` and `moderation`; only the reverse proxy should be reachable from outside.
   The host firewall must block everything else.
3. **Real SMTP**: in `.env` and in the `environment` block of the `backend` service, replace MailHog
   (`NOCT_SMTP_HOST`, `_PORT`, `_USE_TLS`, credentials), or use the `smtp-relay` service.
4. **Set** `NOCT_SESSION_COOKIE_SECURE=true`, `NOCT_CORS_ORIGINS`, `NOCT_OAUTH_REDIRECT_BASE_URL`,
   `NOCT_S3_PUBLIC_URL` (public URL of the read proxy, for example `https://s3.example.com`) and
   `NOCT_FRONTEND_URL` to the real https values; remove the `localhost` values from the `environment:` blocks of the compose file.
5. **Backup** to a truly external S3 storage (`NOCT_BACKUP_S3_*`, removing the defaults that point to the local
   Garage).
6. Automatic start at boot: with rootless Podman, `loginctl enable-linger $USER` and
   `podman generate systemd` / [Quadlet](https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html);
   with Docker, `restart: unless-stopped` and `systemctl enable docker`.

For a structured production installation, K3s ([3](#3-k3s)) or VMs ([4](#4-vm-without-containers-debian-and-ubuntu), [5](#5-vm-without-containers-rocky-and-almalinux)) are recommended.

---

## 3. K3s

The manifests are in [`k8s/`](../k8s/README.md) (Kustomize). A single node is enough to start with; the backend and
worker `Deployment`s can already be replicated. They use Traefik (bundled with K3s), the `local-path` provisioner for
volumes (replaceable with Longhorn) and cert-manager for certificates.

### 3.1 Prerequisites

* VM or server with 4 vCPU / 4 GB RAM, Debian/Ubuntu/Rocky/Alma, root access, ports 80/443 (and 6443 only from the management network).
* Domain with `example.com` and `*.example.com` records pointing to the node's IP.
* For the wildcard: DNS managed by Cloudflare (the manifests' DNS-01 challenge is configured for Cloudflare; for other
  providers, adapt `k8s/cert-manager-issuer.yaml`).
* A registry where the images are published (GHCR via CI, see 3.4) **or** build them on the node.

### 3.2 Install K3s

```bash
curl -sfL https://get.k3s.io | sh -
sudo kubectl get nodes                       # Ready
mkdir -p ~/.kube && sudo cp /etc/rancher/k3s/k3s.yaml ~/.kube/config && sudo chown $USER ~/.kube/config
```

On **Rocky/Alma** with SELinux enabled, first install the policy package if `get.k3s.io` reports it
(`k3s-selinux`; the script offers it by itself), and open the firewall:

```bash
sudo firewall-cmd --permanent --add-service={http,https} --add-port=6443/tcp
sudo firewall-cmd --permanent --zone=trusted --add-source=10.42.0.0/16 --add-source=10.43.0.0/16
sudo firewall-cmd --reload
```

On **Ubuntu** with `ufw`: `sudo ufw allow 80,443/tcp && sudo ufw allow 6443/tcp`.

### 3.3 cert-manager

```bash
kubectl apply -f https://github.com/cert-manager/cert-manager/releases/download/v1.16.2/cert-manager.yaml
kubectl -n cert-manager rollout status deploy/cert-manager-webhook
```

### 3.4 Images

The manifests point to `ghcr.io/bytehawks-org/notturni-{backend,frontend,moderation}` with the tag `0.2.1`
(`images:` in `k8s/kustomization.yaml`), published by the `publish-images` workflow when a `vX.Y.Z` git tag is pushed. Pushes to `develop` publish the mutable `develop` tag and `sha-<commit>`; pushes to `main` publish `main` and `latest`. The earlier images (tags `sha-…` from before the Rust rewrite) are the Python ones and must not be used.

**A. GitHub CI** (recommended): the frontend image does not depend on the domain (URL and name are set at runtime
in the ConfigMap: `NOCT_INSTANCE_FQDN`/`NOCT_SITE_URL`, `NOCT_SITE_NAME`). Start the workflow (push the `vX.Y.Z` tag, or *Run workflow*
on the branch) and wait for the three builds (the backend and moderation builds are slow). The GHCR
package must be readable by the cluster: public, or `imagePullSecrets` with a `read:packages` token.
For later releases update the three `newTag` values to the new semver version.

**B. Local build on the node** (no registry):

```bash
sudo dnf install -y podman || sudo apt install -y podman      # one of the two
TAG=local-1
podman build -t ghcr.io/bytehawks-org/notturni-backend:$TAG .
podman build -t ghcr.io/bytehawks-org/notturni-frontend:$TAG ./frontend
podman build -t ghcr.io/bytehawks-org/notturni-moderation:$TAG ./moderation       # optional
for i in backend frontend moderation; do
  podman save ghcr.io/bytehawks-org/notturni-$i:$TAG | sudo k3s ctr images import -
done
```

Then set `newTag: local-1` for the three images in `k8s/kustomization.yaml`. Without moderation, remove `moderation.yaml`
from `resources:` and empty `NOCT_MODERATION_SERVICE_URL` in `configmap.yaml`.

### 3.5 Configuration

```bash
cd k8s
cp secret.example.yaml secret.yaml       # already in .gitignore: do not commit it
```

1. **`secret.yaml`**: replace **all** the example values (`foo`, `changeme…`, `000…`). Generate them with
   `openssl rand -base64 48` (`NOCT_JWT_SECRET`), `openssl rand -base64 32` (`NOCT_SECRETS_KEY`,
   `NOCT_GARAGE_ADMIN_TOKEN`), `openssl rand -hex 32` (`NOCT_GARAGE_RPC_SECRET`,
   `NOCT_S3_SECRET_ACCESS_KEY`), `echo GK$(openssl rand -hex 12)` (`NOCT_S3_ACCESS_KEY_ID`, to be replicated also in
   `configmap.yaml`). With the placeholders, the backend does not start.
2. **`configmap.yaml`**: `NOCT_INSTANCE_FQDN`, `NOCT_CORS_ORIGINS`, `NOCT_CORS_ORIGIN_REGEX`,
   `NOCT_OAUTH_REDIRECT_BASE_URL`, `NOCT_S3_PUBLIC_URL`, `NOCT_SMTP_*`, `NOCT_SMTP_FROM_EMAIL`,
   `NOCT_TURNSTILE_SITE_KEY` (**replace with your own key**), `NOCT_BACKUP_S3_*`.
3. **Domain**: replace `notturni.eu` with `example.com` in `ingress.yaml`, `ingressroute.yaml`,
   `ingressroute-s3.yaml`, `certificate.yaml` and `cert-manager-issuer.yaml` (Let's Encrypt email and zone).
   `grep -rn notturni.eu k8s/` lists all the places.
4. **Cloudflare**: API token with the `Zone:DNS:Edit` permission on the zone only, in the
   `NOCT_CLOUDFLARE_API_TOKEN` key of `secret.yaml`.

### 3.6 Apply and verify

```bash
kubectl apply -k .
kubectl -n notturni get pods -w                  # all Running; the backend waits for Garage via an initContainer
kubectl -n notturni get certificate              # READY=True after a few minutes (DNS-01)
curl -s https://example.com/api/v1/health/ready
```

The Super Admin is created on first start from `NOCT_SUPER_ADMIN_*` (in `secret.yaml`). The `Deployment`s use
`runAsNonRoot`, and a `NetworkPolicy` closes all inbound traffic except to frontend, backend and `s3-public`, and only
from Traefik (`kube-system`).

### 3.7 Operations

| What | Command |
| --- | --- |
| Logs | `kubectl -n notturni logs deploy/backend -f` · `deploy/worker` |
| Migration status | `kubectl -n notturni exec deploy/backend -- noct-ctl migrate status` |
| Machine-to-machine token | `kubectl -n notturni exec deploy/backend -- noct-ctl token create --name core-engine` |
| Update | new `newTag` in `kustomization.yaml`, then `kubectl apply -k .` (migrations run automatically) |
| More replicas | `kubectl -n notturni scale deploy/backend --replicas=3` (same for `worker`: the scheduler uses atomic leader election) |
| Remove everything | `kubectl delete -k .` (PVCs remain until you delete them) |

### 3.8 Helm chart (alternative to the Kustomize manifests)

The chart in [`helm/notturni/`](../helm/notturni/README.md) installs the same stack with a choice of profiles. It defaults to the lean solo profile: PostgreSQL, backend (1 replica, local storage on a PVC), frontend and a Traefik Ingress. Redis, NATS with a worker, Garage, image moderation, the SMTP relay and per-blog subdomains are switched on with values. Kustomize in `k8s/` stays the reference for the full platform profile; see [k8s/README.md](../k8s/README.md).

```bash
cat > my-solo.yaml <<'EOF'
mode: solo
ingress:
  host: example.com
smtp:
  host: smtp.example.com
secrets:
  existingSecret: notturni-secrets
EOF
helm lint helm/notturni -f my-solo.yaml
helm install notturni helm/notturni -n notturni --create-namespace -f my-solo.yaml
kubectl -n notturni get pods,pvc,ingress
```

Create the Secret first (keys and a ready-made command in the [chart README](../helm/notturni/README.md#secrets)), or leave `secrets.existingSecret` empty and let the chart generate the mandatory keys. Upgrades use `helm upgrade`. `helm uninstall` does not delete the PVCs (`persistence.keepOnUninstall: true`). The chart has not yet been installed on a real cluster: the manual checklist is in the [chart README](../helm/notturni/README.md#smoke-test-checklist-manual). The full deployment guide (profiles, values reference, secrets, backup, operations, troubleshooting) is [HELM.md](HELM.md).

---

## 4. VM without containers: Debian and Ubuntu

Native installation on a Debian 12/13 or Ubuntu 22.04/24.04 VM: system services (`systemd`), no
containers. Run all commands as a user with `sudo`.

### 4.1 Base packages

```bash
sudo apt update && sudo apt install -y ca-certificates curl git openssl build-essential cmake pkg-config \
  postgresql postgresql-client redis-server ufw
psql --version     # 16 or later; on Debian 12 (15), install the postgresql-16 package from apt.postgresql.org (PGDG)
```

Optional, for the frontend: Node.js 22 (see 4.7). The `postgresql-client` package provides `pg_dump`, which the
`backup` module uses.

### 4.2 PostgreSQL

```bash
DBPASS=$(openssl rand -hex 16); echo "$DBPASS"        # note it: it goes into NOCT_POSTGRES_PASSWORD
sudo -u postgres psql -c "CREATE ROLE notturni LOGIN PASSWORD '$DBPASS'"
sudo -u postgres psql -c "CREATE DATABASE notturni OWNER notturni"
```

Postgres listens only on `localhost` (default): do not open port 5432 on the firewall. The application requires
no extension to install and no superuser.

### 4.3 Redis

```bash
sudo sed -i 's/^# *bind .*/bind 127.0.0.1 ::1/; s/^protected-mode .*/protected-mode yes/' /etc/redis/redis.conf
sudo systemctl enable --now redis-server
redis-cli ping      # PONG
```

### 4.4 NATS JetStream

```bash
NATS=v2.11.8; ARCH=$(dpkg --print-architecture)       # amd64 | arm64
curl -fsSL https://github.com/nats-io/nats-server/releases/download/$NATS/nats-server-$NATS-linux-$ARCH.tar.gz | tar xz
sudo install -m755 nats-server-$NATS-linux-$ARCH/nats-server /usr/local/bin/
sudo useradd --system --home /var/lib/nats --create-home --shell /usr/sbin/nologin nats
sudo tee /etc/systemd/system/nats.service >/dev/null <<'EOF'
[Unit]
Description=NATS JetStream
After=network.target

[Service]
User=nats
ExecStart=/usr/local/bin/nats-server -js -sd /var/lib/nats -a 127.0.0.1 -p 4222
Restart=on-failure
NoNewPrivileges=true
ProtectSystem=strict
ReadWritePaths=/var/lib/nats
PrivateTmp=true

[Install]
WantedBy=multi-user.target
EOF
sudo systemctl daemon-reload && sudo systemctl enable --now nats
```

### 4.5 Service user and directories

```bash
sudo useradd --system --home /opt/notturni --create-home --shell /usr/sbin/nologin notturni
sudo install -d -o notturni -g notturni -m750 /var/lib/notturni /var/lib/notturni/storage
sudo install -d -m750 -g notturni /etc/notturni
```

### 4.6 Backend: binaries

There are two options. The second avoids compiling on the VM (a release build needs about 4 GB of RAM, 10 GB of disk and
several minutes).

**A. Build from source**

```bash
curl -fsSL https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.97 && . ~/.cargo/env
git clone https://github.com/bytehawks-org/notturni.git ~/notturni && cd ~/notturni
cargo build --release --locked -p noct-api -p noct-worker -p noct-ctl
sudo install -m755 target/release/{noct-api,noct-worker,noct-ctl} /usr/local/bin/
```

**B. Extract the binaries from the image** (same architecture and libc as the image: Debian 13 "trixie" or later)

```bash
sudo apt install -y podman
ID=$(podman create ghcr.io/bytehawks-org/notturni-backend:TAG)         # TAG: see 3.4
sudo podman cp $ID:/usr/local/bin/. /usr/local/bin/ && podman rm $ID
```

### 4.7 Configuration and frontend

Generate `.env` as in [1.3](#13-secrets-scriptsgen-envsh) (from the repository copy), then adapt it to the VM:

```bash
cd ~/notturni && scripts/gen-env.sh /tmp/notturni.env
```

Values to set or replace in `/tmp/notturni.env` (then moved to `/etc/notturni/notturni.env`):

```ini
NOCT_INSTANCE_FQDN=example.com
NOCT_BIND_ADDR=127.0.0.1:8000
NOCT_TRUSTED_PROXIES=1
NOCT_POSTGRES_HOST=127.0.0.1
NOCT_POSTGRES_USER=notturni
NOCT_POSTGRES_PASSWORD=<DBPASS di 4.2>
NOCT_POSTGRES_DB=notturni
NOCT_REDIS_HOST=127.0.0.1
NOCT_NATS_URL=nats://127.0.0.1:4222
# storage on filesystem (recommended on a single VM); for Garage/S3 see 4.10
NOCT_STORAGE_BACKEND=localstorage
NOCT_LOCAL_STORAGE_BASE_PATH=/var/lib/notturni/storage
NOCT_LOCAL_STORAGE_PUBLIC_URL=https://example.com/storage
NOCT_CORS_ORIGINS=https://example.com
NOCT_CORS_ORIGIN_REGEX=https://([a-z0-9-]+[.])?example[.]com
NOCT_SESSION_COOKIE_SECURE=true
NOCT_FRONTEND_URL=https://example.com
NOCT_OAUTH_REDIRECT_BASE_URL=https://example.com
NOCT_FRONTEND_REVALIDATE_URL=http://127.0.0.1:3000/api/revalidate
NOCT_SMTP_HOST=smtp.example.com
NOCT_SMTP_USER=...
NOCT_SMTP_PASSWORD=...
NOCT_SMTP_FROM_EMAIL=no-reply@example.com
NOCT_MODERATION_SERVICE_URL=
```

The `NOCT_S3_*`, `NOCT_GARAGE_*` and `NOCT_S3_PUBLIC_URL` variables can stay: with `localstorage` they are ignored.

```bash
sudo install -m640 -o root -g notturni /tmp/notturni.env /etc/notturni/notturni.env && shred -u /tmp/notturni.env
```

**Frontend** (Node.js 22):

```bash
curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash - && sudo apt install -y nodejs
cd ~/notturni/frontend && npm ci
npm run build
sudo mkdir -p /opt/notturni/frontend
sudo cp -r .next/standalone/. /opt/notturni/frontend/
sudo cp -r .next/static /opt/notturni/frontend/.next/static && sudo cp -r public /opt/notturni/frontend/public
sudo chown -R notturni:notturni /opt/notturni/frontend
```

The site URL, the API URL and the name are read at runtime from the process environment (`NOCT_SITE_URL`, `NOCT_API_PUBLIC_URL`, `NOCT_SITE_NAME`): changing the domain does not require a new build.

### 4.8 systemd services

```bash
sudo tee /etc/systemd/system/noct-api.service >/dev/null <<'EOF'
[Unit]
Description=Notturni API
After=network-online.target postgresql.service redis-server.service nats.service
Wants=network-online.target

[Service]
User=notturni
Group=notturni
EnvironmentFile=/etc/notturni/notturni.env
ExecStart=/usr/local/bin/noct-api
Restart=on-failure
RestartSec=3
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ReadWritePaths=/var/lib/notturni

[Install]
WantedBy=multi-user.target
EOF

sed 's/Notturni API/Notturni worker/; s#/usr/local/bin/noct-api#/usr/local/bin/noct-worker#; s/^After=\(.*\)/After=\1 noct-api.service/' \
  /etc/systemd/system/noct-api.service | sudo tee /etc/systemd/system/noct-worker.service >/dev/null

sudo tee /etc/systemd/system/notturni-frontend.service >/dev/null <<'EOF'
[Unit]
Description=Notturni frontend (Next.js)
After=network-online.target noct-api.service

[Service]
User=notturni
Group=notturni
WorkingDirectory=/opt/notturni/frontend
Environment=NODE_ENV=production PORT=3000 HOSTNAME=127.0.0.1
Environment=NOCT_BACKEND_INTERNAL_URL=http://127.0.0.1:8000
EnvironmentFile=/etc/notturni/notturni.env
ExecStart=/usr/bin/node server.js
Restart=on-failure
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable --now noct-api noct-worker notturni-frontend
journalctl -u noct-api -n 30 --no-pager        # look for the startup line "in ascolto … 127.0.0.1:8000" (listening on)
curl -s http://127.0.0.1:8000/api/v1/health/ready
```

Migrations and the Super Admin creation run on the first API start. The worker reads the same file
and **must** share the same `NOCT_JWT_SECRET`, `NOCT_SECRETS_KEY` and database.

### 4.9 Reverse proxy and TLS

Example with **Caddy** (automatic TLS with Let's Encrypt):

```bash
sudo apt install -y debian-keyring debian-archive-keyring apt-transport-https
curl -1sLf https://dl.cloudsmith.io/public/caddy/stable/gpg.key | sudo gpg --dearmor -o /usr/share/keyrings/caddy.gpg
curl -1sLf https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt | sudo tee /etc/apt/sources.list.d/caddy.list
sudo apt update && sudo apt install -y caddy
```

`/etc/caddy/Caddyfile`:

```caddyfile
example.com {
	encode zstd gzip
	header {
		Strict-Transport-Security "max-age=31536000"
		X-Content-Type-Options nosniff
		Referrer-Policy strict-origin-when-cross-origin
	}
	handle /api/* {
		reverse_proxy 127.0.0.1:8000
	}
	handle /storage/* {
		reverse_proxy 127.0.0.1:8000
	}
	handle {
		reverse_proxy 127.0.0.1:3000
	}
}
```

For **subdomain blogs** (`username.example.com`) a wildcard certificate is needed, so the DNS-01 challenge:
use a Caddy build with the plugin for your DNS provider (build from [caddyserver.com/download](https://caddyserver.com/download),
for example `caddy-dns/cloudflare`) and declare `example.com, *.example.com { tls { dns cloudflare {env.CF_API_TOKEN} } … }`
with the same body. Without a wildcard the installation works on the main domain only (suitable for `solo`).

```bash
sudo systemctl reload caddy
```

### 4.10 Garage / S3 instead of the filesystem (optional)

With `NOCT_STORAGE_BACKEND=s3` you need an S3-compatible endpoint (Garage locally, or AWS S3 / Cloudflare R2 /
another provider) and `NOCT_S3_ENDPOINT_URL`, `NOCT_S3_REGION`, `NOCT_S3_ACCESS_KEY_ID`, `NOCT_S3_SECRET_ACCESS_KEY`,
`NOCT_S3_PUBLIC_URL`. For Garage on the VM: install the binary ([garagehq.deuxfleurs.fr](https://garagehq.deuxfleurs.fr/)),
use [`garage/garage.toml`](../garage/garage.toml) as a template, then `noct-ctl garage-bootstrap`
(with `NOCT_GARAGE_ADMIN_URL=http://127.0.0.1:3903`) creates the layout, key and buckets. Public reads require the
proxy that exposes only the public prefixes (see [`s3-public/default.conf.template`](../s3-public/default.conf.template)).

### 4.11 Firewall and hardening

```bash
sudo ufw default deny incoming && sudo ufw allow OpenSSH && sudo ufw allow 80,443/tcp && sudo ufw enable
sudo ss -ltnp | grep -E ':(5432|6379|4222|8000|3000)\b'     # all on 127.0.0.1, never on 0.0.0.0
```

### 4.12 Backup, update, diagnostics

| What | How |
| --- | --- |
| Daily backup | set `NOCT_BACKUP_S3_*` (**external** S3 storage): the `backup.run` job runs `pg_dump` and mirrors the buckets. With `localstorage`, also copy `/var/lib/notturni/storage` (for example with `restic`/`rsync`) |
| Manual dump | `sudo -u postgres pg_dump -Fc notturni > notturni-$(date +%F).dump` |
| Update | `cd ~/notturni && git pull && cargo build --release --locked -p noct-api -p noct-worker -p noct-ctl && sudo install -m755 target/release/{noct-api,noct-worker,noct-ctl} /usr/local/bin/ && sudo systemctl restart noct-api noct-worker` (frontend: run `npm run build` again and copy as in 4.7) |
| Logs | `journalctl -u noct-api -u noct-worker -u notturni-frontend -f` |
| `noct-ctl` (migrations, failed messages, API tokens) | `sudo systemd-run --pty --collect -p User=notturni -p EnvironmentFile=/etc/notturni/notturni.env /usr/local/bin/noct-ctl migrate status` (same for `dead-letters`, `token create --name core-engine`): it uses the same variable parser as the services |

---

## 5. VM without containers: Rocky and AlmaLinux

Rocky Linux / AlmaLinux 9 (and 10): same layout as section 4, with `dnf`, `firewalld` and **SELinux in
enforcing mode** (keep it enabled). Follow the steps below instead of 4.1–4.4, 4.9 and 4.11; **4.2 (role and database),
4.5–4.8 and 4.10 are identical**, with these differences: the services are named `postgresql` and `redis`
(not `redis-server`), and the `After=` lines in the units must use `postgresql.service redis.service`.

### 5.1 Packages

```bash
sudo dnf install -y epel-release
sudo dnf install -y git curl openssl gcc gcc-c++ make cmake pkgconf-pkg-config policycoreutils-python-utils
sudo dnf module enable -y postgresql:16 redis:7
sudo dnf install -y postgresql-server postgresql redis
sudo postgresql-setup --initdb
sudo systemctl enable --now postgresql
sudo sed -i 's/^# *bind .*/bind 127.0.0.1 ::1/; s/^protected-mode .*/protected-mode yes/' /etc/redis/redis.conf
sudo systemctl enable --now redis && redis-cli ping
```

If the `redis:7` module is not available (Rocky/Alma 10), run `sudo dnf install -y redis` (or `valkey`, compatible).
`pg_dump` is in the `postgresql` package. Then create the role and database as in 4.2.

### 5.2 NATS

As in 4.4, with `ARCH=amd64` (x86_64) or `arm64` (aarch64) instead of `dpkg --print-architecture`. After
installing the binary, assign the correct SELinux context:

```bash
sudo restorecon -v /usr/local/bin/nats-server /usr/local/bin/noct-*
```

### 5.3 Binaries, frontend, services

* Build: `curl -fsSL https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.97` and `cargo build --release --locked …`
  as in 4.6-A (the build packages are already installed in 5.1). The binaries taken from the image (4.6-B) are
  built on Debian: they work only if the VM's glibc is recent enough (Rocky/Alma 9 has 2.34:
  **build from source**).
* Node.js 22: `sudo dnf module enable -y nodejs:22 && sudo dnf install -y nodejs`, then as in 4.7.
* systemd units: identical to 4.8; `/usr/bin/node` is the same path.

### 5.4 SELinux

The paths `/opt/notturni/frontend` and `/var/lib/notturni` are accessible to the `systemd` services with the
default context. The only extra permission needed is for the **reverse proxy** to reach the local backends:

```bash
sudo setsebool -P httpd_can_network_connect 1          # nginx/Apache; Caddy installed from a package, if confined
sudo semanage fcontext -a -t var_lib_t "/var/lib/notturni(/.*)?" && sudo restorecon -Rv /var/lib/notturni
sudo ausearch -m avc -ts recent | audit2why             # if a service does not start: what SELinux denies
```

Do not use `setenforce 0` to "make the installation work": read the AVC with `audit2why` and fix the context.

### 5.5 Reverse proxy

Caddy is available from a COPR repository: `sudo dnf install -y 'dnf-command(copr)' && sudo dnf copr enable -y @caddy/caddy && sudo dnf install -y caddy`,
then the same `Caddyfile` as in 4.9 and `sudo systemctl enable --now caddy`. Alternatively, nginx
(`sudo dnf install -y nginx`) with `proxy_pass http://127.0.0.1:8000` for `/api/` and `/storage/`, and
`proxy_pass http://127.0.0.1:3000` for `/`, setting `proxy_set_header X-Forwarded-For $remote_addr;`
(the backend trusts the last value: with `NOCT_TRUSTED_PROXIES=1`) and `proxy_set_header Host $host;`.
Certificates with `certbot --nginx` (main domain) or `certbot` with a DNS plugin (wildcard).

### 5.6 Firewall

```bash
sudo firewall-cmd --permanent --add-service={http,https} && sudo firewall-cmd --reload
sudo firewall-cmd --list-all                                  # only ssh, http, https
sudo ss -ltnp | grep -E ':(5432|6379|4222|8000|3000)\b'       # all on 127.0.0.1
```

### 5.7 Update, backup, diagnostics

As in 4.12, with identical `journalctl`/`systemctl` commands. For the dump: `sudo -u postgres pg_dump -Fc notturni > …`.

---

## 6. Functional verification after installation

```bash
B=https://example.com/api/v1
curl -s $B/health/ready                                                         # status ok
curl -s -c /tmp/c -H 'content-type: application/json' \
  -d '{"email":"admin@example.com","password":"<Super Admin password>"}' $B/auth/login
curl -s $B/config                                                               # public configuration
```

Then, in the browser: sign in, create a blog, publish a post, and open the public page. From
`/dashboard` (administration), check **Panoramica** (Overview): the `postgres`, `cache`, `bus` and `storage` services must
be `ok`. Verify that a test email (password reset) arrives. If it does not, run `journalctl -u noct-worker`.

## 7. Common problems

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| The backend exits immediately with "`NOCT_JWT_SECRET` è il valore d'esempio" (`NOCT_JWT_SECRET` is the example value) | `.env` copied from `.env.example` | `scripts/gen-env.sh` or `openssl rand -base64 48` |
| The Super Admin does not exist (log: "creazione del Super Admin non riuscita", Super Admin creation failed) | example or too short password | set `NOCT_SUPER_ADMIN_PASSWORD` and restart |
| Sign-in fails with `username` | sign-in uses **email** | use the email address |
| 429 on sign-in after a few attempts | per-email/IP limit (anti brute-force) | wait for the window to reset; behind a proxy, check `NOCT_TRUSTED_PROXIES` |
| All users appear with the same IP | `NOCT_TRUSTED_PROXIES` too low/high for the proxy chain | set the real number of trusted proxies |
| Email, newsletters, backups do not run | `noct-worker` not running, or SMTP/`NOCT_BACKUP_S3_*` not set | start the worker; `noct-ctl dead-letters` |
| The browser does not stay signed in | `Secure` cookie over http, or origin not in `NOCT_CORS_ORIGINS` | https + `NOCT_SESSION_COOKIE_SECURE=true`, or `false` for a trial over http |
| Images/avatars do not show | wrong `NOCT_S3_PUBLIC_URL` / `NOCT_LOCAL_STORAGE_PUBLIC_URL`, or `/storage` not forwarded | fix the public URL and the reverse proxy |
| The browser calls the wrong domain | `NOCT_SITE_URL`/`NOCT_INSTANCE_FQDN` (or `NOCT_API_PUBLIC_URL`) not set or wrong in the ConfigMap | fix them and restart the `frontend` Deployment (no rebuild needed) |
| `ImagePullBackOff` on K3s | the tag was not published (workflow not finished), the GHCR package is private, or the image was not imported | see 3.4 |
| Pods do not start on K3s with NetworkPolicy | CNI without network policy support | use the K3s CNI (default) or remove `networkpolicy.yaml` |
| Users with MFA cannot sign in after a secret change | `NOCT_SECRETS_KEY` lost or rotated | restore the original key |
