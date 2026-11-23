# Notturni – Kubernetes manifests

These Kustomize manifests are the reference for the full **platform** deployment (Redis, NATS, worker, Garage, moderation, subdomains) and are unchanged in structure. A Helm chart with a lean **solo** default and the same components behind values is in [`helm/notturni/`](../helm/notturni/README.md). Use one or the other for an installation, not both in the same namespace. `configmap.yaml` now sets `NOCT_DEPLOYMENT_MODE` and `NOCT_SUBDOMAINS_ENABLED` explicitly (`platform`, `true`).

First draft of the manifests for a single-node K3s cluster (see
[ROADMAP.md](../ROADMAP.md#3-architecture-stack-and-infrastructure)). It requires
a `StorageClass` and Traefik as IngressController (Traefik is already included
in a standard K3s installation, unless it was explicitly disabled).
`postgres.yaml`/`garage.yaml` default to `storageClassName: local-path` (the
provisioner bundled with K3s, suitable for a single node with no extra
components to install). Longhorn remains the target for a future multi-node
cluster. Replace that value to enable it when you move to that setup.

`ingress.yaml`/`ingressroute.yaml` now serve **https** under the real domain
`notturni.eu` (apex + wildcard `*.notturni.eu` for per-subdomain blogs). TLS
is managed by **cert-manager**, which is not included in these manifests and
must be installed separately (see below). It uses an explicit `Certificate`
resource (`certificate.yaml`) and a Let's Encrypt `ClusterIssuer` with a
DNS-01 challenge on Cloudflare (`cert-manager-issuer.yaml`). DNS-01 is
required for the wildcard, because the HTTP-01 challenge cannot issue it.
`NOCT_SESSION_COOKIE_SECURE="true"` in `configmap.yaml` matches this setup.
Always change these two settings together, never one without the other. To
return to an http-only test via the node IP, without a domain or cert-manager,
remove the `tls`/`host` blocks from `ingress.yaml`/`ingressroute.yaml` **and**
set that variable to `"false"` at the same time.

## TLS certificates (cert-manager)

```bash
# cert-manager is not included in a standard K3s installation (unlike
# Longhorn/Traefik). Install it before applying these manifests:
helm repo add jetstack https://charts.jetstack.io
helm install cert-manager jetstack/cert-manager \
  --namespace cert-manager --create-namespace --set crds.enabled=true

# Cloudflare API token (Zone:DNS:Edit permission on the notturni.eu zone only)
# goes in secret.yaml, key NOCT_CLOUDFLARE_API_TOKEN. See the comment there.
```

**First test with `letsencrypt-staging`**: the production Let's Encrypt has
strict limits (5 duplicate certificates per week for the exact domain).
Temporarily change `issuerRef.name` in `certificate.yaml` from
`letsencrypt-prod` to `letsencrypt-staging`. Check that
`kubectl describe certificate notturni-eu-tls -n notturni` reaches `Ready`.
The staging certificate is not trusted by browsers, but issuing it confirms
that the DNS-01 solver and the Cloudflare token work. Then switch back to
`letsencrypt-prod` and delete the `notturni-eu-tls` Secret so that issuance
restarts with the correct issuer
(`kubectl delete secret notturni-eu-tls -n notturni`).

## Data migration from MinIO to Garage

Only for installations that already hold data on MinIO (the official image
is no longer published). Bucket names and object keys stay unchanged, so the
references in the database (`object_key` of avatars/media) do not change. Only
the objects are copied. **Not verified against a real MinIO**: the Garage side
has been tested. Try it first on a copy.

1. Keep the existing MinIO Deployment/pod running (with the image already on
   the node). Rename it if needed (`minio-legacy`) to avoid confusion with the
   new one. Apply the new manifests (`kubectl apply -k .`) with Garage keys
   generated as described below. The backend starts with an empty Garage:
   the bootstrap creates the buckets and the layout.
2. Copy the objects with `rclone` (from a pod or host that can reach both):

   ```bash
   export RCLONE_CONFIG_OLD_TYPE=s3 RCLONE_CONFIG_OLD_PROVIDER=Minio \
          RCLONE_CONFIG_OLD_ENDPOINT=http://minio-legacy:9000 \
          RCLONE_CONFIG_OLD_ACCESS_KEY_ID=<old key> RCLONE_CONFIG_OLD_SECRET_ACCESS_KEY=<old secret>
   export RCLONE_CONFIG_NEW_TYPE=s3 RCLONE_CONFIG_NEW_PROVIDER=Other \
          RCLONE_CONFIG_NEW_ENDPOINT=http://garage:3900 RCLONE_CONFIG_NEW_REGION=garage \
          RCLONE_CONFIG_NEW_ACCESS_KEY_ID=<GK...> RCLONE_CONFIG_NEW_SECRET_ACCESS_KEY=<64 hex>
   for b in avatars notturni-content notturni-audit; do rclone copy OLD:$b NEW:$b --progress; done
   rclone check OLD:avatars NEW:avatars   # same for the other buckets
   ```

3. Check an existing avatar and media item at `https://s3.notturni.eu/...`.
   Then remove MinIO and its PVC (`minio-data`).

Note: the infrastructure backups already written under the `minio/` prefix of
the backup bucket stay where they are. New backups use `s3/`.

## Pod security

`backend.yaml` and `worker.yaml` run with `runAsNonRoot` (numeric UID 10001 in
the image), seccomp profile `RuntimeDefault`, no privilege escalation, and no
capabilities. `networkpolicy.yaml` closes ingress to the namespace. Pods can
reach each other. From outside (Traefik, namespace `kube-system`), only
`frontend`, `backend` and `s3-public` are reachable. Postgres, Redis, NATS,
Garage and moderation are not exposed. A CNI that enforces NetworkPolicy is
required (K3s: built-in controller). Full installation guide:
[docs/INSTALL.md](../docs/INSTALL.md#3-k3s).

The policy covers only **ingress**. The worker can connect to any host. Git
sync (`git_sync`) and link previews contact addresses chosen by users. They
defend themselves by checking that the host resolves to public addresses. But
a DNS rebinding window remains between that check and the connection made by
`git`. To close it in production, add an **egress** NetworkPolicy for the
worker that excludes private networks and cloud metadata (`169.254.169.254`).
It must allow only the namespace services and the public Internet.

## Setup

```bash
cp secret.example.yaml secret.yaml
# Edit secret.yaml with real values. Do not commit it (already in .gitignore).

kubectl apply -k .
```

**Updating the image tag** (after a new push to GHCR): change it in one place,
the three `newTag` entries under `images:` at the bottom of
`kustomization.yaml`, not in the individual manifests. `notturni-backend`
covers both `backend.yaml` **and** `worker.yaml` with the same tag. Kustomize
replaces every `image:` that references that exact name, which is repeated
identically in several files. Then run `kubectl apply -k .`.

For a first test without a registry (images built locally on the node instead
of published to GHCR): build them on the same machine as the K3s node, or
import them with `k3s ctr images import` if they were built elsewhere. Update
the three `newTag` (and, if needed, `newName`) values in `kustomization.yaml`
accordingly. Then, in `configmap.yaml`, replace `NOCT_CORS_ORIGINS` and
`NOCT_OAUTH_REDIRECT_BASE_URL` with the address that is actually reachable
(for example `http://<node-ip>`, see the comments next to those variables).
Also set `NOCT_SITE_URL=http://<node-ip>`. It is read at runtime, so no
rebuild is needed (see the note below).

## Notes

- The images published to GHCR by the CI build/push flow (see the "Setup"
  section above for how to update their tag). `imagePullPolicy:
  IfNotPresent`. If the referenced tag was never published (or is not present
  locally on the node for a test without a registry), the pod stays in
  `ImagePullBackOff`.
- `postgres.yaml` sets `PGDATA` to a subdirectory of the volume
  (`/var/lib/postgresql/data/pgdata`) instead of the mount root. A Longhorn
  volume (ext4) comes with a `lost+found` directory created by the
  filesystem, and `initdb` refuses to initialize a non-empty data directory.
  Without this setting, the first start would fail.
- `redis.yaml` has no persistence (cache and rate limits, rebuildable).
  `nats.yaml` has a dedicated volume for JetStream. Messages already written
  to the database transactional outbox survive a loss of that volume anyway.
- `ingress.yaml` handles catch-all path-based routing under the fixed host
  `notturni.eu`. Per-blog subdomain routing is in `ingressroute.yaml`
  (Traefik `IngressRoute`, `HostRegexp` on `*.notturni.eu`). The two resources
  coexist. The second does not replace the first. Both use `https`
  (`entryPoints: [websecure]`) and share the same TLS Secret
  `notturni-eu-tls` (see the "TLS certificates" section above). They share it
  because cert-manager cannot annotate a Traefik `IngressRoute` CRD directly,
  as it does with a standard `Ingress`. That is why an explicit `Certificate`
  resource is used instead of the `cert-manager.io/cluster-issuer` annotation.
  Custom per-user domains are a later task (see
  [ROADMAP.md](../ROADMAP.md#3-architecture-stack-and-infrastructure)).
- `middleware-security-headers.yaml` (Traefik `Middleware`) sets minimal HSTS,
  `X-Content-Type-Options: nosniff`, `X-Frame-Options` and `Referrer-Policy`.
  It applies to both resources above (annotation on `ingress.yaml`,
  `middlewares` field on `ingressroute.yaml`), so there is one place to keep
  up to date. It does **not** set a Content-Security-Policy. A static CSP
  identical for every request cannot include a nonce. Also, `script-src 'self'`
  without a nonce or `unsafe-inline` blocks the inline scripts that Next.js
  injects for hydration, not only third-party scripts. This was found in
  production: the page was served (200), but no form or button responded,
  including login and registration. The CSP is now generated per request with
  a fresh nonce in `frontend/src/proxy.ts`.
- Object storage is **Garage** (`garage.yaml`, single node, config in
  `garage.toml`). A freshly started Garage is not usable (no layout, no key).
  The `garage-bootstrap` initContainer in `backend.yaml`
  (`noct-ctl garage-bootstrap`, idempotent) assigns the node to the layout,
  imports the application key, and creates the buckets with a global alias.
  The key must use the Garage format. `NOCT_S3_ACCESS_KEY_ID` (in
  `configmap.yaml`) = `GK` + 24 hex. `NOCT_S3_SECRET_ACCESS_KEY` (in
  `secret.yaml`) = 64 hex. Generate them with `echo "GK$(openssl rand -hex 12)"`
  and `openssl rand -hex 32`. Likewise for `NOCT_GARAGE_RPC_SECRET` (64 hex)
  and `NOCT_GARAGE_ADMIN_TOKEN`. The committed values are placeholders.
- Garage **does not implement S3 bucket policies**. Avatars and media are
  served from its web endpoint through the `s3-public.yaml` proxy (nginx,
  config in `s3-public.conf.template`). The proxy maps `/{bucket}/{key}` to
  `Host: {bucket}` and forwards only GET/HEAD on the public prefixes: the
  avatar bucket and `.../userdata/*/*/media/*` in the content bucket. Post
  backups and audit archives stay unreachable. `garage.toml` and
  `s3-public.conf.template` are copies of `garage/garage.toml` and
  `s3-public/default.conf.template`, which `compose.yaml` uses. Kustomize
  cannot read files outside this directory. `cargo test -p noct-api --test deploy_configs`
  checks that they do not diverge.
- `ingressroute-s3.yaml` (Traefik `IngressRoute`) exposes only the `s3-public`
  proxy, on the host set in `NOCT_S3_PUBLIC_URL` (`configmap.yaml`, default
  `s3.notturni.eu`). It never exposes the S3 API or the Garage admin API. It
  has an explicit priority higher than the wildcard match in
  `ingressroute.yaml`. Without it, that host would fall through to the
  frontend (404). This was observed in production: uploads succeeded, but the
  media was unreachable. Writes stay behind the `NOCT_S3_ACCESS_KEY_ID` /
  `_SECRET_ACCESS_KEY` credentials, used server-side only. `s3` is also in
  `RESERVED_SUBDOMAINS`/`RESERVED_BLOG_SLUGS` (`frontend/src/proxy.ts`,
  `crates/noct-blogs/src/rules.rs`), so a user cannot register a blog with
  that slug.
- The alternative storage backend (`NOCT_STORAGE_BACKEND=localstorage`, see
  [ROADMAP.md](../ROADMAP.md#3-architecture-stack-and-infrastructure)) is not
  wired into these manifests. It would need a dedicated volume on the backend
  Deployment, and it is not meant for multiple replicas anyway (the filesystem
  is not shared between pods).
- The site URL, API URL and platform name of the frontend are read **at
  runtime** from the ConfigMap (`NOCT_INSTANCE_FQDN`/`NOCT_SITE_URL`,
  `NOCT_API_PUBLIC_URL`, `NOCT_SITE_NAME`, `frontend/src/lib/site.ts`). The
  same image serves any domain. Changing them requires only a restart of the
  `frontend` Deployment. The `NEXT_PUBLIC_*` variables are only a fallback for
  local development.
- `worker.yaml` (Deployment, 2 replicas, no `Service`): `noct-worker`. It runs
  the outbox relay, the scheduler for periodic jobs, and the consumers of
  events and jobs from **all** active modules (MFA and reset emails,
  newsletter confirmations and sending, post copies, ...). It replaces the
  three consumer Deployments and the two `CronJob`s of the Python version.
  Multiple replicas are safe. NATS distributes the jobs, and each interval of
  a periodic job is assigned to one replica with an atomic `UPDATE`. When
  `NOCT_NATS_URL` is set, the API runs no consumers (`NOCT_WORKER_INLINE` is
  derived from it).
- Periodic jobs (declared with `schedule` in the `module.toml` files, enqueued
  by the worker): scheduled newsletter campaigns (every minute). Audit
  maintenance (daily): archives the completed ISO weeks to storage and deletes
  records older than `audit_retention_days`, always limited to the archived
  watermark. Permanent deletion of blogs older than the 30-day grace period.
  Infrastructure backup (`backup` module): `pg_dump` plus mirroring of the
  buckets to an **external** S3 storage. Without `NOCT_BACKUP_S3_BUCKET` in
  `configmap.yaml`, the run does nothing. Point it to a provider that is
  truly external and separate from the content storage before production.
  The Markdown of each published post is copied to the content bucket by the
  same module, on event. The state can be read from
  `GET /api/v1/admin/audit-log` and from `noct-ctl`.
- **Hot-swappable modules**: optional modules are enabled and disabled from
  `/api/v1/admin/modules` (Super Admin) without restarting anything. The
  desired state is stored in the database (`noct_modules`). The API and the
  worker reread it every 5 seconds (immediately on the API that received the
  request). They apply missing migrations, rebuild the router and consumers,
  and swap them in. `NOCT_MODULES` is only the seed for the first
  installation. Disabling a module never deletes data or tables. If a module
  migration fails, `GET /admin/modules` shows it with the error
  (`pending: true`). It retries every 30 seconds.
- `moderation.yaml` (Deployment + Service, port 8100): the automatic image
  moderation service (`moderation/`). It is `fail open` by design. A problem
  in this service never blocks an upload (see `noct-media`). But without
  `NOCT_MODERATION_SERVICE_URL` set in `configmap.yaml` (it is present now),
  no image would ever be moderated. Its `readinessProbe`/`livenessProbe` use a
  higher `initialDelaySeconds` than usual, because the model (torch) is loaded
  at startup.
- `smtp-relay.yaml` is a Deployment + Service that is always on. It is a
  containerized SMTP service (`boky/postfix`), an alternative to pointing
  `NOCT_SMTP_HOST` directly at an external provider for MFA codes and other
  backend emails. With `NOCT_SMTP_RELAY_HOST` empty (default), the container
  sends directly. This still requires a domain with SPF, DKIM and reverse DNS
  in place for good deliverability. With it set, the container forwards to
  that external smarthost, using the credentials in `secret.yaml`. It is not
  connected to `NOCT_SMTP_HOST` by default. Choosing which of the two to use
  is an explicit decision. Locally, the equivalent is the `smtp-relay` service
  in `compose.yaml`.
