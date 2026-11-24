# Deploying with the Helm chart

This guide describes how to install, configure, operate and troubleshoot Notturni with the Helm chart in [`helm/notturni/`](../helm/notturni/README.md). It is written against the chart as it is in the repository (chart `0.1.0`, `appVersion` `0.2.0-rust`); every key, default and object name below comes from `values.yaml` and `templates/`.

**Status.** The chart passes `helm lint` and `helm template` for the three value sets in `helm/notturni/ci/` and the tests in `bins/noct-api/tests/helm_chart.rs`, with Helm 3.19.0 in CI. It has **not yet been installed on a real cluster**: the smoke test (task T019) is pending. Statements about runtime behavior that rely on a live cluster are marked "not yet verified on a cluster".

## Contents

1. [Overview and requirements](#1-overview-and-requirements)
2. [Profiles: solo and platform](#2-profiles-solo-and-platform)
3. [Quick start: solo](#3-quick-start-solo)
4. [Values reference](#4-values-reference)
5. [Secrets](#5-secrets)
6. [Storage and data](#6-storage-and-data)
7. [Modules and the database-wins caveat](#7-modules-and-the-database-wins-caveat)
8. [Configuration examples](#8-configuration-examples)
9. [Operations](#9-operations)
10. [Security and compliance notes](#10-security-and-compliance-notes)
11. [Validation and CI](#11-validation-and-ci)
12. [Troubleshooting](#12-troubleshooting)

## 1. Overview and requirements

The chart installs Notturni on Kubernetes with one set of templates and two profiles. The default is the lean **solo** stack (PostgreSQL, backend, frontend, Ingress). The **platform** profile adds Redis, NATS with a separate worker, Garage object storage, image moderation, an SMTP relay and per-blog subdomains, each behind its own values flag. The Kustomize manifests in [`k8s/`](../k8s/README.md) remain the reference for the full platform stack. Use the chart or the manifests for a given namespace, never both.

| Requirement | Detail |
|---|---|
| Helm | Helm 3. CI uses 3.19.0 (pinned, checksum verified); older 3.x versions are not tested. |
| Kubernetes | `kubeVersion: ">=1.27.0-0"` in `Chart.yaml`. K3s on a single node is the target. |
| Ingress controller | Traefik, the K3s default (`ingress.className: traefik`). The security-headers `Middleware` and the wildcard `IngressRoute` use the Traefik CRDs (`traefik.io/v1alpha1`). On another controller set `ingress.traefik.securityHeaders=false` and do not enable subdomains or Garage (the Garage proxy route is also an `IngressRoute`). |
| Storage class | A default `StorageClass` (K3s: `local-path`), or set `persistence.*.storageClass`. |
| DNS and TLS | A public host name for `ingress.host` pointing at the node. TLS needs a Secret with the certificate or cert-manager, which is not part of K3s ([k8s/README.md](../k8s/README.md#tls-certificates-cert-manager)). |
| Architecture | The chart pins no architecture. Images are multi-arch tags, and a chart test rejects `arm64v8` and `amd64/` references, so the same values work on x86_64 and ARM64. |

Resource names follow `<fullname>-<component>`. `<fullname>` is the release name, or `<release>-notturni` when the release name does not contain `notturni`; `fullnameOverride` replaces it and `nameOverride` replaces the chart-name part. The examples below use release `notturni`, so the backend Deployment is `notturni-backend`. Names are truncated at 63 characters.

## 2. Profiles: solo and platform

`mode` is `solo` (default) or `platform`. On its own, `mode` only sets `NOCT_DEPLOYMENT_MODE` for the backend, worker and frontend, and enables one render-time check (subdomains need `platform`). The components are chosen by their own flags, so `mode: platform` without the other flags still deploys the lean stack. What the modes mean to the application (registration, UI labels, collaborators) is described in [CONFIGURATION.md](CONFIGURATION.md#deployment-profiles-solo-and-platform).

### Objects per profile

| Profile | Switches | Kubernetes objects |
|---|---|---|
| Solo (default) | none | `Secret` `<fullname>-secrets` (unless `secrets.existingSecret`), `ConfigMap` `-config`; `PersistentVolumeClaim` `-postgres` and `-storage`; `Deployment` + `Service` `-postgres`, `-backend`, `-frontend`; `Ingress` `<fullname>` (paths `/api`, `/storage`, `/`); Traefik `Middleware` `-security-headers`. |
| Redis | `redis.enabled` | `Deployment` + `Service` `-redis` (no volume). |
| NATS and worker | `nats.enabled` | `PersistentVolumeClaim` + `Deployment` + `Service` `-nats` (JetStream, port 4222); `Deployment` `-worker`. |
| Moderation | `moderation.enabled` | `Deployment` + `Service` `-moderation` (port 8100). |
| SMTP relay | `smtp.relay.enabled` | `Deployment` + `Service` `-smtp-relay` (port 587). |
| S3 with Garage | `storage.type=s3` and `storage.s3.garage.enabled` | `ConfigMap` `-garage-config`, `PersistentVolumeClaim` `-garage-meta` and `-garage-data`, `Deployment` + `Service` `-garage`; `ConfigMap` `-s3-public-config`, `Deployment` + `Service` `-s3-public` (read-only nginx proxy); `IngressRoute` `-s3` (host `s3.<ingress.host>` by default); a `garage-bootstrap` init container on the backend. The `-storage` PVC is not created. |
| Subdomains | `subdomains.enabled` (needs `mode=platform`) | `IngressRoute` `-blog-subdomains` (wildcard `<slug>.<host>` to the frontend). |
| cert-manager | `ingress.tls.certManager.enabled` | `Certificate` `-tls`; with `createIssuer.enabled` also an `Issuer` `-issuer`. |
| Network policies | `networkPolicy.enabled` | `NetworkPolicy` `-default-deny`, `-allow-release`, `-allow-ingress-controller`. |

`ci/platform-values.yaml` turns on all of these together. With external S3 (`storage.type=s3` without Garage) no Garage or `s3-public` objects exist. With an external PostgreSQL (`postgres.external.host`) the `-postgres` Deployment, Service and PVC are not created.

### Replicas and update strategy

The chart refuses to render a combination the application cannot run:

- `storage.type=local` requires `backend.replicaCount: 1`. Local storage lives on a ReadWriteOnce volume mounted by one pod, and the backend serves `/storage` from it.
- `nats.enabled=false` requires `backend.replicaCount: 1`. Without NATS the backend uses the in-process bus and runs the worker inline.

With local storage the backend Deployment, and the worker Deployment when NATS is on, use `strategy: Recreate`. A RWO volume cannot be mounted by the old and the new pod at the same time, so an upgrade stops the old pod first and causes a short downtime. With S3 storage the backend uses the default rolling update. The PostgreSQL, NATS and Garage Deployments are always `Recreate` with one replica. To scale the backend, set `storage.type=s3` and `nats.enabled=true` (and Redis for shared rate limits, see [CONFIGURATION.md](CONFIGURATION.md#deployment-profiles-solo-and-platform)). If you run local storage together with NATS, the backend and the worker both mount the same RWO claim and must be scheduled on the same node.

## 3. Quick start: solo

Create the namespace and a Secret you manage (recommended; see [Secrets](#5-secrets)):

```bash
kubectl create namespace notturni
kubectl -n notturni create secret generic notturni-secrets \
  --from-literal=NOCT_POSTGRES_PASSWORD="$(openssl rand -hex 16)" \
  --from-literal=NOCT_JWT_SECRET="$(openssl rand -base64 48)" \
  --from-literal=NOCT_REVALIDATE_SECRET="$(openssl rand -hex 24)"
```

Minimal values file (`my-solo.yaml`):

```yaml
mode: solo
ingress:
  host: example.com
secrets:
  existingSecret: notturni-secrets
```

Install and check:

```bash
helm lint helm/notturni -f my-solo.yaml
helm install notturni helm/notturni -n notturni -f my-solo.yaml
kubectl -n notturni get pods,pvc,ingress
kubectl -n notturni rollout status deploy/notturni-backend
```

`ingress.host` is the only required value. Without SMTP settings the instance sends no email. To let the chart generate the secrets instead, leave `secrets.existingSecret` empty; `NOTES.txt` then points at the generated Secret `<fullname>-secrets` and never prints its values.

### First login and the admin account

There are two ways to get the first Super Admin.

- **First-run registration (solo default).** Leave `admin.email` empty. Open `https://example.com/`, register: the first user becomes the Super Admin and registration then closes.
- **From values.** Set `admin.email` (and optionally `admin.username`, default `superadmin`). The backend creates the account at start, and the chart adds `NOCT_SUPER_ADMIN_PASSWORD` to the generated Secret. Read it with:

  ```bash
  kubectl -n notturni get secret notturni-secrets \
    -o jsonpath='{.data.NOCT_SUPER_ADMIN_PASSWORD}' | base64 -d; echo
  ```

  With `secrets.existingSecret` you provide the key `NOCT_SUPER_ADMIN_PASSWORD` yourself. The backend creates the account only when username, email and password are all present, and rejects the `.env.example` placeholder. If `admin.email` is set but the key is missing from your Secret, the backend starts normally and creates **no** Super Admin (verified on minikube); it logs a warning. Then register the first user through the sign-up page, or add the key and restart the backend.

Login is by **email**, not by username (`POST /api/v1/auth/login`).

### Collaborator accounts

Solo closes registration after the first user, so the Super Admin creates every other account. Use the admin form at `/admin/utenti/nuovo`, or the CLI inside the backend pod (the password is read from stdin, never from an argument):

```bash
printf '%s\n' "$PW" | kubectl -n notturni exec -i deploy/notturni-backend -- \
  noct-ctl user create --email bob@example.com --username bob
```

The command prints the new user's id. With an empty stdin it generates a password and prints it once. Then invite the user from the blog's collaborators tab. Roles and the full flow are in [CONFIGURATION.md](CONFIGURATION.md#collaborators-in-solo-mode).

## 4. Values reference

The schema in `values.schema.json` rejects wrong types, an invalid `mode` or `storage.type`, and an empty or malformed `ingress.host`. It also rejects unknown keys under `admin` and `secrets`, but not elsewhere: a misspelled key under another section is ignored silently, so check the rendered output.

### Mode, site identity and admin

| Key | Default | Meaning |
|---|---|---|
| `mode` | `solo` | `solo` or `platform` (`NOCT_DEPLOYMENT_MODE`). |
| `nameOverride`, `fullnameOverride` | empty | Replace the chart name or the whole base name of the resources. |
| `site.name` | `Notturni` | Display name (`NOCT_SITE_NAME`). |
| `site.slug` | `notturni` | Instance slug (`NOCT_SITE_SLUG`). |
| `site.experimentalFeatures` | empty | `NOCT_EXPERIMENTAL_FEATURES`, comma separated `wiki`, `docs`. Empty means blogs only. |
| `site.turnstileSiteKey` | empty | Public Cloudflare Turnstile key (`NOCT_TURNSTILE_SITE_KEY`). The secret key goes in the Secret as `NOCT_TURNSTILE_SECRET_KEY`. |
| `site.trustedProxies` | `1` | `NOCT_TRUSTED_PROXIES`: reverse proxies in front of the API (the Ingress controller). |
| `admin.username` | `superadmin` | Username of the first Super Admin. |
| `admin.email` | empty | Creates the first Super Admin when set. Empty skips it (solo: use the first-run registration). |

### Images

| Key | Default | Meaning |
|---|---|---|
| `image.pullPolicy` | `IfNotPresent` | Applied to every container. |
| `image.imagePullSecrets` | `[]` | List of `{name: ...}` for a private registry. |
| `image.backend`, `image.frontend`, `image.moderation` | `ghcr.io/bytehawks-org/notturni-{backend,frontend,moderation}`, tag empty | Empty `tag` means the chart `appVersion` (`0.2.0-rust`). The worker and the `garage-bootstrap` init container use the backend image. |
| `image.postgres` | `docker.io/library/postgres:16-alpine` | |
| `image.redis` | `docker.io/library/redis:7-alpine` | |
| `image.nats` | `docker.io/library/nats:2-alpine` | |
| `image.garage` | `docker.io/dxflrs/garage:v2.4.1` | |
| `image.s3Public` | `docker.io/nginxinc/nginx-unprivileged:1.28-alpine` | |
| `image.smtpRelay` | `docker.io/boky/postfix:v4.4.0` | |

Each image has `repository` and `tag`. To mirror images, change `repository`.

### Ingress and TLS

| Key | Default | Meaning |
|---|---|---|
| `ingress.enabled` | `true` | Creates the `Ingress`, the `Middleware`, the subdomain and Garage `IngressRoute`s and the `Certificate`. Off means no external routing at all. |
| `ingress.className` | `traefik` | `ingressClassName`. |
| `ingress.host` | empty, required | Public host. Becomes `NOCT_INSTANCE_FQDN` and the base of every public URL (`NOCT_SITE_URL`, CORS origin, OAuth redirect base). |
| `ingress.annotations` | `{}` | Extra annotations on the `Ingress`. |
| `ingress.tls.enabled` | `true` | TLS on the Ingress. Off switches the scheme to `http` everywhere and sets `NOCT_SESSION_COOKIE_SECURE=false`. |
| `ingress.tls.secretName` | empty | Certificate Secret. Empty means `<fullname>-tls`. |
| `ingress.tls.certManager.enabled` | `false` | Creates a cert-manager `Certificate` named `<fullname>-tls` for the host, plus `*.<host>` with subdomains, or `s3.<host>` with Garage. cert-manager must be installed separately. Needs `ingress.tls.enabled=true`. |
| `ingress.tls.certManager.issuerRef` | `{name: letsencrypt-prod, kind: ClusterIssuer}` | Existing issuer to use. Ignored when `createIssuer.enabled`. |
| `ingress.tls.certManager.createIssuer.enabled` | `false` | Creates a namespaced `Issuer` with a Cloudflare DNS-01 solver (needed for the wildcard). The API token is read from the Secret key `NOCT_CLOUDFLARE_API_TOKEN`. |
| `ingress.tls.certManager.createIssuer.email` | empty | ACME account email. Required when `createIssuer.enabled`. |
| `ingress.tls.certManager.createIssuer.server` | `https://acme-v02.api.letsencrypt.org/directory` | ACME directory. |
| `ingress.traefik.securityHeaders` | `true` | Creates the `Middleware` (HSTS one year with subdomains and preload, `nosniff`, frame options `DENY`, referrer policy) and references it from the Ingress annotation. The CSP is not set here: the frontend generates it per request. |
| `subdomains.enabled` | `false` | Per-blog subdomains. Needs `mode=platform` and a wildcard certificate (DNS-01). Adds the wildcard `IngressRoute`, `NOCT_SUBDOMAINS_ENABLED=true`, `NOCT_CORS_ORIGIN_REGEX` and `NOCT_SESSION_COOKIE_DOMAIN=.<host>`. |

### Storage

| Key | Default | Meaning |
|---|---|---|
| `storage.type` | `local` | `local`: a PVC served by the backend at `/storage`. `s3`: an S3-compatible service. |
| `storage.local.publicUrl` | empty | `NOCT_LOCAL_STORAGE_PUBLIC_URL`. Empty means `<scheme>://<ingress.host>/storage`. |
| `storage.local.basePath` | `/data/storage` | Mount path in the backend and worker (`NOCT_LOCAL_STORAGE_BASE_PATH`). Must start with `/`. |
| `storage.s3.endpointUrl` | empty | External S3 or R2 endpoint. Required for `type=s3` without Garage. |
| `storage.s3.region` | empty | `NOCT_S3_REGION` for external S3. With Garage the chart sets `garage`. |
| `storage.s3.publicUrl` | empty | `NOCT_S3_PUBLIC_URL`, the public base URL of the objects. Required for external S3. With Garage it defaults to `<scheme>://<s3 host>`. |
| `storage.s3.buckets.avatars`, `.content`, `.audit` | `avatars`, `notturni-content`, `notturni-audit` | Bucket names. |
| `storage.s3.garage.enabled` | `false` | Deploys Garage, the `s3-public` proxy and the bootstrap init container. Needs `storage.type=s3`. |
| `storage.s3.garage.publicHost` | empty | Host of the public read proxy. Empty means `s3.<ingress.host>`. |
| `backup.s3.endpointUrl`, `.region`, `.bucket` | empty | Target of the `backup` module, an external S3. An empty `bucket` leaves the job inactive. Credentials: `NOCT_BACKUP_S3_ACCESS_KEY_ID`, `NOCT_BACKUP_S3_SECRET_ACCESS_KEY` in the Secret. |

S3 credentials are Secret keys `NOCT_S3_ACCESS_KEY_ID` and `NOCT_S3_SECRET_ACCESS_KEY`. With both absent the AWS default credential chain (an IAM role) applies. With Garage the chart generates them.

### Optional services

| Key | Default | Meaning |
|---|---|---|
| `redis.enabled` | `false` | Deploys Redis and sets `NOCT_REDIS_HOST` and `NOCT_REDIS_PORT`. Off means an in-memory cache and no `NOCT_REDIS_*` variables. Redis has no volume and no authentication; it only holds rate limits and locks. |
| `nats.enabled` | `false` | Deploys NATS JetStream and a separate `worker` Deployment, sets `NOCT_NATS_URL`. Off means the in-process bus and an inline worker. |
| `moderation.enabled` | `false` | Deploys the moderation service, sets `NOCT_MODERATION_SERVICE_URL` and keeps `image_moderation` in the module list. The service fails open: while it is down, uploads are accepted unfiltered. |
| `smtp.host` | empty | External SMTP server (`NOCT_SMTP_HOST`). |
| `smtp.port` | `587` | `NOCT_SMTP_PORT`. |
| `smtp.useTls` | `true` | `NOCT_SMTP_USE_TLS`. |
| `smtp.fromEmail` | empty | `NOCT_SMTP_FROM_EMAIL`. Emitted only when SMTP or the relay is configured. |
| `smtp.relay.enabled` | `false` | Deploys the postfix relay and points `NOCT_SMTP_HOST` at it (port 587, TLS off inside the cluster). `smtp.host`, `smtp.port` and `smtp.useTls` are then ignored. |
| `smtp.relay.host` | empty | Upstream smarthost (`NOCT_SMTP_RELAY_HOST`). Empty means direct delivery. |

SMTP credentials are the Secret keys `NOCT_SMTP_USER` and `NOCT_SMTP_PASSWORD`; relay credentials are `NOCT_SMTP_RELAY_USERNAME` and `NOCT_SMTP_RELAY_PASSWORD`. With neither SMTP nor relay configured, no `NOCT_SMTP_*` variable exists and no email is sent.

### Modules

| Key | Default | Meaning |
|---|---|---|
| `modules` | 21 modules, without `image_moderation` | `NOCT_MODULES` at the first start only; see [section 7](#7-modules-and-the-database-wins-caveat). The default list is `link_preview`, `gdpr`, `auth_mfa`, `auth_sso`, `pages`, `backup`, `comments`, `discovery`, `fragments`, `git_sync`, `graph`, `interests`, `legal`, `newsletter`, `notes`, `publications`, `reports`, `taxonomy`, `wiki`, `branding`, `custom_domains`. An empty list omits `NOCT_MODULES` (backend default set). Items must match `^[a-z][a-z0-9_]*$`. |

### Persistence and database

| Key | Default | Meaning |
|---|---|---|
| `persistence.keepOnUninstall` | `true` | Adds `helm.sh/resource-policy: keep` to every chart PVC. |
| `persistence.postgres.size` | `5Gi` | Size of the `-postgres` PVC. |
| `persistence.postgres.storageClass` | empty | Empty means the cluster default. |
| `persistence.postgres.existingClaim` | empty | Use this claim instead of creating one. |
| `persistence.postgres.accessMode` | `ReadWriteOnce` | |
| `persistence.storage.size` | `10Gi` | Size of the local-storage PVC `-storage`. |
| `persistence.storage.storageClass`, `.existingClaim`, `.accessMode` | empty, empty, `ReadWriteOnce` | As above. Only used with `storage.type=local`. |
| `persistence.nats.size`, `.storageClass` | `2Gi`, empty | NATS JetStream volume. |
| `persistence.garage.metaSize`, `.dataSize`, `.storageClass` | `1Gi`, `10Gi`, empty | Garage metadata and data volumes. |
| `postgres.external.host` | empty | When set, the chart deploys no PostgreSQL and points `NOCT_POSTGRES_HOST` here. |
| `postgres.external.port` | `5432` | Used only with an external host. |
| `postgres.user` | `admin` | `NOCT_POSTGRES_USER`; also the user the bundled PostgreSQL is initialised with. |
| `postgres.database` | `notturni` | `NOCT_POSTGRES_DB`. |

The NATS and Garage claims have no `existingClaim` option. The PostgreSQL password is always the Secret key `NOCT_POSTGRES_PASSWORD`; the chart sets no TLS option for the database connection.

### Secrets, workloads and network

| Key | Default | Meaning |
|---|---|---|
| `secrets.existingSecret` | empty | Name of a Secret you manage. See [section 5](#5-secrets). |
| `secrets.extra` | `{}` | Extra keys added to the chart-created Secret. Ignored with `existingSecret`. |
| `backend.replicaCount` | `1` | Must be 1 with local storage or without NATS. |
| `backend.resources`, `.nodeSelector`, `.tolerations`, `.affinity` | empty | Passed through to the pod. No requests or limits are set by default. |
| `backend.extraEnv` | `[]` | Extra container `env` entries (a list of `name`/`value` or `valueFrom`). |
| `frontend.replicaCount` | `1` | The frontend is stateless and can scale. |
| `frontend.resources`, `.nodeSelector`, `.tolerations`, `.affinity`, `.extraEnv` | empty | As for the backend. |
| `worker.replicaCount` | `1` | Only deployed when `nats.enabled=true`. |
| `worker.resources` | `{}` | The worker has no `nodeSelector`, `tolerations`, `affinity` or `extraEnv`. |
| `postgresPod.resources` | `{}` | Resources of the bundled PostgreSQL. |
| `networkPolicy.enabled` | `false` | Default-deny ingress for the release pods. |
| `networkPolicy.ingressControllerNamespace` | `kube-system` | Namespace allowed to reach the frontend, backend and `s3-public` (K3s Traefik runs in `kube-system`). |

There are no `resources` keys for Redis, NATS, moderation, Garage, `s3-public` or the SMTP relay; those pods run without requests or limits.

## 5. Secrets

Secrets never go in a values file. The chart offers two ways to supply them.

### Chart-generated Secret (default)

With `secrets.existingSecret` empty, the chart creates the Secret `<fullname>-secrets`. On first install it generates `NOCT_POSTGRES_PASSWORD`, `NOCT_JWT_SECRET` (64 characters) and `NOCT_REVALIDATE_SECRET`; `NOCT_SUPER_ADMIN_PASSWORD` when `admin.email` is set; and, with Garage, `NOCT_S3_ACCESS_KEY_ID`, `NOCT_S3_SECRET_ACCESS_KEY`, `NOCT_GARAGE_RPC_SECRET` and `NOCT_GARAGE_ADMIN_TOKEN`. On `helm upgrade` the template uses `lookup` to read the live Secret and keeps every existing key, so the values survive upgrades. Keys you add with `secrets.extra` are rewritten from your input on every upgrade. Pass them with `--set-file` or an untracked values file.

The chart does not generate `NOCT_SECRETS_KEY`, the key that encrypts TOTP secrets and Git sync credentials at rest. Without it, the backend derives the key from the JWT secret, so rotating the JWT secret would make those stored values unreadable. In production, add it with `secrets.extra` or put it in your own Secret, and keep a copy outside the cluster ([CONFIGURATION.md](CONFIGURATION.md#security-and-gdprnis2)).

### `existingSecret`

Set `secrets.existingSecret` to the name of a Secret you create. The chart then creates no Secret. Keys:

| Key | Required | Used for |
|---|---|---|
| `NOCT_POSTGRES_PASSWORD` | yes | PostgreSQL password (bundled or external). |
| `NOCT_JWT_SECRET` | yes | Token signing; at least 32 random bytes. The backend refuses to start with a shorter value, or with a value containing `cambia-questo` or `changeme`. |
| `NOCT_REVALIDATE_SECRET` | yes | Shared with the frontend for cache revalidation. The frontend marks it optional, so a missing key does not block its start. |
| `NOCT_SUPER_ADMIN_PASSWORD` | with `admin.email` | First Super Admin password. |
| `NOCT_SMTP_USER`, `NOCT_SMTP_PASSWORD` | optional | External SMTP login. |
| `NOCT_SMTP_RELAY_USERNAME`, `NOCT_SMTP_RELAY_PASSWORD` | optional | Upstream login of the relay. |
| `NOCT_S3_ACCESS_KEY_ID`, `NOCT_S3_SECRET_ACCESS_KEY` | optional | S3 credentials; both absent means the AWS default chain. Required with Garage. |
| `NOCT_GARAGE_RPC_SECRET`, `NOCT_GARAGE_ADMIN_TOKEN` | with Garage | Required: the Garage pod reads them without a default. |
| `NOCT_BACKUP_S3_ACCESS_KEY_ID`, `NOCT_BACKUP_S3_SECRET_ACCESS_KEY` | optional | Target of `backup.s3`. |
| `NOCT_OAUTH_*`, `NOCT_TURNSTILE_SECRET_KEY` | optional | SSO providers and captcha. |
| `NOCT_CLOUDFLARE_API_TOKEN` | with `createIssuer` | DNS-01 solver token. |
| `NOCT_SECRETS_KEY` | recommended | Encryption key described above. |

The backend and worker load the whole Secret with `envFrom`, so any key you add becomes an environment variable. The frontend receives only `NOCT_REVALIDATE_SECRET`.

### Why GitOps needs `existingSecret`

`lookup` queries the live cluster, so it works only in `helm install` and `helm upgrade`. Tools that run `helm template` (Argo CD, Flux with rendered manifests) get no result from `lookup` and would render new random values at every sync. Use `secrets.existingSecret` there, with a Secret delivered by Sealed Secrets, SOPS or an external secrets operator.

### Rotation

Changing a key in a Secret you manage does not restart the pods: the Deployments' `checksum/config` annotation tracks only the ConfigMap. After editing a Secret, run `kubectl -n notturni rollout restart deploy/notturni-backend` (and the frontend for `NOCT_REVALIDATE_SECRET`, and the worker if present). Rotating `NOCT_JWT_SECRET` invalidates sessions and, without `NOCT_SECRETS_KEY`, the stored TOTP and Git credentials. Changing `NOCT_POSTGRES_PASSWORD` in the Secret does not change the password inside an existing database: alter the role in PostgreSQL first. Rotating a chart-generated key means editing the Secret by hand, since the chart keeps what exists.

## 6. Storage and data

### Volumes

| PVC | Created when | Holds |
|---|---|---|
| `<fullname>-postgres` | bundled PostgreSQL and no `persistence.postgres.existingClaim` | The database (personal data). Data directory `/var/lib/postgresql/data/pgdata`; the subdirectory avoids storage classes that create `lost+found`. |
| `<fullname>-storage` | `storage.type=local` and no `persistence.storage.existingClaim` | Uploaded files, mounted at `storage.local.basePath`. |
| `<fullname>-nats` | `nats.enabled` | JetStream data. |
| `<fullname>-garage-meta`, `-garage-data` | Garage enabled | Garage metadata and objects. |

With `persistence.keepOnUninstall: true` (default) each of these carries `helm.sh/resource-policy: keep`, so `helm uninstall` leaves them. Claims you pass as `existingClaim` are never created or deleted by the chart. Setting `keepOnUninstall=false` makes the next uninstall delete the chart-created claims, and the data with them. The annotation is applied at creation; changing the value on an existing release is not covered here and was not verified.

### Backup

Back up two things: the PostgreSQL data and the storage volume (or the S3 buckets). Both hold personal data. A consistent database backup is a `pg_dump` taken from the pod:

```bash
kubectl -n notturni exec deploy/notturni-postgres -- \
  sh -c 'pg_dump -U "$POSTGRES_USER" -Fc "$POSTGRES_DB"' > notturni-$(date +%F).dump
```

For local storage, copy the volume contents, for example with `kubectl cp` from the backend pod (`/data/storage`) or a volume snapshot if your storage class supports it. The `backup` module can also push a dump and the buckets to an external S3 configured with `backup.s3.*`. Test the restore; the commands here were not run on a cluster.

### Restore outline

1. Install the release with the same `secrets.existingSecret` and, if you re-attach old volumes, `persistence.postgres.existingClaim` and `persistence.storage.existingClaim`. A freshly generated Secret over an old database leaves PostgreSQL with the old password.
2. For a restore from a dump, stop the backend (`kubectl -n notturni scale deploy/notturni-backend --replicas=0`), restore with `pg_restore --clean --if-exists -U <user> -d <database>` into the database, then scale back to 1. Restore into an empty database where you can.
3. Copy the files back into the storage volume, or restore the S3 buckets.
4. Check readiness and the audit trail after the first start.

### Moving between storage types

Changing `storage.type` on a live release does not copy existing files from the volume to S3 or back.

## 7. Modules and the database-wins caveat

`NOCT_MODULES` (from the `modules` list) applies only at the first start of an empty database. After that, the module state table in the database wins, and you change modules in the admin panel at `/admin/modules`. Editing `modules` and running `helm upgrade` on an installed release changes the ConfigMap but not the active set. The same holds for a solo install pointed at a database that previously ran as `platform`: `image_moderation` stays active until you turn it off in `/admin/modules` ([CONFIGURATION.md](CONFIGURATION.md#existing-database-moved-to-solo)).

`image_moderation` is handled by the chart: the template drops it from your list and appends it back only when `moderation.enabled=true` and the list is not empty. With an empty `modules` list the variable is omitted, and the backend default set applies, which includes `image_moderation`. `ci/platform-values.yaml` uses that on purpose.

## 8. Configuration examples

The CI value sets are the tested starting points. They render without error; none has been installed on a cluster.

### Solo (the CI file)

`ci/solo-values.yaml`:

```yaml
mode: solo
ingress:
  host: example.com
admin:
  email: admin@example.com
```

### Solo with external S3

Needs `storage.s3.endpointUrl` and `storage.s3.publicUrl`, otherwise the render fails. Because S3 replaces the volume, you can raise `backend.replicaCount` only if you also enable NATS.

```yaml
mode: solo
ingress:
  host: blog.example.com
storage:
  type: s3
  s3:
    endpointUrl: https://s3.eu.example.com
    region: eu-1
    publicUrl: https://cdn.example.com
    buckets: {avatars: av, content: content, audit: audit}
secrets:
  existingSecret: notturni-secrets   # add NOCT_S3_ACCESS_KEY_ID and NOCT_S3_SECRET_ACCESS_KEY
```

The `publicUrl` host must serve the `avatars` and `content` buckets read-only to browsers.

### Solo with Redis

```yaml
mode: solo
ingress:
  host: example.com
redis:
  enabled: true
```

Adds the `-redis` Deployment and `NOCT_REDIS_HOST`. Switching it on later is `helm upgrade notturni helm/notturni -n notturni -f my-solo.yaml --set redis.enabled=true`.

### Platform with the bundled components

`ci/platform-values.yaml`:

```yaml
mode: platform
ingress:
  host: example.com
  tls:
    certManager:
      enabled: true
      createIssuer: {enabled: true, email: admin@example.com}
subdomains: {enabled: true}
storage:
  type: s3
  s3: {garage: {enabled: true}}
redis: {enabled: true}
nats: {enabled: true}
moderation: {enabled: true}
smtp:
  fromEmail: info@example.com
  relay: {enabled: true, host: smtp.example.com}
backup:
  s3: {endpointUrl: https://backup.example.com, region: auto, bucket: notturni-backups}
modules: []
backend: {replicaCount: 2}
frontend: {replicaCount: 2}
worker: {replicaCount: 2}
networkPolicy: {enabled: true}
persistence:
  postgres: {storageClass: local-path}
```

The file is abridged here (`ingress` and `storage` are written in flow style). It needs DNS for `example.com`, `*.example.com` and `s3.example.com`, the Secret key `NOCT_CLOUDFLARE_API_TOKEN`, and cert-manager installed. Several backend replicas on one node are fine; with a multi-node cluster the Garage and NATS volumes still tie those pods to a node.

### Existing PostgreSQL claim

```yaml
persistence:
  postgres:
    existingClaim: my-postgres
```

The chart creates no `-postgres` claim and mounts yours in the bundled PostgreSQL Deployment. To use a server outside the cluster instead:

```yaml
postgres:
  external:
    host: db.example.com
    port: 5432
  user: notturni
  database: notturni
```

Then no PostgreSQL objects are created, and the password comes from `NOCT_POSTGRES_PASSWORD`.

### External SMTP or the relay

External server:

```yaml
smtp:
  host: smtp.example.com
  port: 587
  useTls: true
  fromEmail: info@example.com
```

Put `NOCT_SMTP_USER` and `NOCT_SMTP_PASSWORD` in the Secret. Or deploy the relay, which accepts mail from the backend without credentials and forwards it to a smarthost:

```yaml
smtp:
  fromEmail: info@example.com
  relay:
    enabled: true
    host: smtp.example.com   # empty = direct delivery
```

Relay credentials are `NOCT_SMTP_RELAY_USERNAME` and `NOCT_SMTP_RELAY_PASSWORD`. The relay allows senders from `ingress.host` only (`ALLOWED_SENDER_DOMAINS`), so `fromEmail` must be in that domain.

### Everything non-default

`ci/full-values.yaml` sets every option (external S3, existing Secret, existing claims, annotations, `extraEnv`, `nameOverride`) and is the test file for the rarely used template branches. It is not a production recommendation.

## 9. Operations

### Upgrade and rollback

```bash
helm upgrade notturni helm/notturni -n notturni -f my-solo.yaml
helm history notturni -n notturni
helm rollback notturni <revision> -n notturni
```

Pods restart when the ConfigMap changes (the `checksum/config` annotation). Always pass the same `-f` file (or `--reuse-values` deliberately) so that you do not fall back to defaults. With local storage the upgrade has a short downtime because of `Recreate`. `helm rollback` restores the chart objects, not the database: a migration that has run stays applied. Take a database backup before upgrading to a new `appVersion`.

### Migrations

The chart has no migration Job and no init container for migrations. The backend applies them itself at start: it migrates the core, reads the desired module set, then migrates the active modules, under an advisory lock that makes several replicas safe. `NOCT_AUTO_MIGRATE=false` disables that. The chart sets it only on the worker, which therefore never migrates and relies on the backend; whether the worker waits for the first migration at a fresh install was not verified. Inspect the state from the pod:

```bash
kubectl -n notturni exec deploy/notturni-backend -- noct-ctl migrate status
kubectl -n notturni exec deploy/notturni-backend -- noct-ctl migrate verify
```

Do not use `migrate apply` against production by hand unless you know why the backend did not apply it.

### Probes

| Workload | Readiness | Liveness |
|---|---|---|
| backend | `GET /api/v1/health/ready` on 8000, 5 s delay, every 10 s | `GET /api/v1/health` on 8000, 10 s delay, every 20 s |
| frontend | `GET /` on 3000, 5 s delay, every 10 s | none |
| postgres | `pg_isready -U $(POSTGRES_USER)`, every 5 s | none |
| nats | `GET /healthz` on 8222 | same, 10 s delay |
| moderation | `GET /health` on 8100, 30 s delay | same, 45 s delay |
| garage | TCP 3903 | TCP 3900 |
| s3-public | `GET /healthz` on 8080 | same |

Redis and the SMTP relay have no probes. Garage uses a TCP probe because its `/health` answers 503 until the layout is assigned by the bootstrap container.

### Logs and shell commands

```bash
kubectl -n notturni logs deploy/notturni-backend
kubectl -n notturni logs deploy/notturni-backend -c garage-bootstrap   # Garage only
kubectl -n notturni logs deploy/notturni-worker                        # NATS only
```

The backend logs the active storage, cache and bus at start and each migration it applies. `noct-ctl` is in the backend image and reads its configuration from the pod environment:

```bash
kubectl -n notturni exec deploy/notturni-backend -- noct-ctl validate
kubectl -n notturni exec deploy/notturni-backend -- noct-ctl modules
kubectl -n notturni exec deploy/notturni-backend -- noct-ctl outbox
printf '%s\n' "$NEW" | kubectl -n notturni exec -i deploy/notturni-backend -- \
  noct-ctl user reset-password --email admin@example.com
```

`user reset-password` reads the new password from stdin, deletes the user's sessions and writes an audit event. It is the recovery path without SMTP ([CONFIGURATION.md](CONFIGURATION.md#password-recovery-without-smtp)). Use `-i` and no `-t` so that stdin carries the password.

### Scaling limits

Backend replicas above 1 need `storage.type=s3` and `nats.enabled=true`; the chart enforces it. Add `redis.enabled=true` so that rate limits are shared across replicas. The frontend can scale freely. PostgreSQL, NATS, Garage and Redis are single-replica Deployments. The cluster limits, not the chart, decide how far a single-node K3s can go.

### Uninstall

```bash
helm uninstall notturni -n notturni
```

This deletes pods, Services, the Ingress, ConfigMaps and the chart-created Secret (including generated passwords). PVCs stay with the default `keepOnUninstall: true`. To reinstall on the old data, keep or restore the Secret and point `existingClaim` at the old claims. To delete the data, remove the claims deliberately (`kubectl -n notturni delete pvc ...`); that cannot be undone.

## 10. Security and compliance notes

- **Personal data.** The PostgreSQL and storage volumes (and Garage volumes) hold personal data under GDPR. Record the retention and backup in your register of processing activities, encrypt the underlying disks if your storage class allows it, and restrict `kubectl` access to the namespace, since anyone who can read the Secret or exec into the backend can read everything. A `backup` module target is an external processor: register it, preferably in the EU.
- **Audit trail.** The application writes an audit log (`audit` bucket, `noct-ctl audit`). Admin actions done with `noct-ctl user reset-password` are audited with actor `system`. Kubernetes API access is logged only if you enable cluster audit logging.
- **Secrets.** Keep them out of values and Git ([section 5](#5-secrets)). The chart never prints them in `NOTES.txt`. Set `NOCT_SECRETS_KEY` and keep it outside the cluster.
- **TLS.** `ingress.tls.enabled` defaults to `true`; the session cookie is `Secure` only then. Turning TLS off is for tests. The Traefik `Middleware` adds HSTS with a one-year `max-age`, subdomains and preload.
- **No moderation in solo.** Without `moderation.enabled`, uploads are not classified, and authors mark sensitive images by hand. The operator decides whether this is acceptable and should say so in the terms of use ([CONFIGURATION.md](CONFIGURATION.md#image-moderation-off)). With moderation on, the service fails open when unavailable.
- **Pod security.** The backend and worker pods set `runAsNonRoot: true` and `seccompProfile: RuntimeDefault`; with local storage `fsGroup: 10001` (the image's user) so that the volume is writable. Their containers, and the `garage-bootstrap` init container, set `allowPrivilegeEscalation: false` and drop all capabilities. The chart sets no `readOnlyRootFilesystem`. The frontend, PostgreSQL, Redis, NATS, moderation, Garage, `s3-public` and the relay have no `securityContext` in the chart and run with their image defaults. The nginx image for `s3-public` is the unprivileged variant. No ServiceAccount, `automountServiceAccountToken` or Pod Security admission setting is touched.
- **Network.** `networkPolicy.enabled=true` denies all ingress to release pods except from the same release and, for the frontend, backend and `s3-public`, from the namespace in `networkPolicy.ingressControllerNamespace`. Egress is not restricted. It works only if the CNI enforces NetworkPolicy (K3s has a built-in controller). The Garage admin API (port 3903) is reachable in the cluster only and is never routed by an Ingress.
- **Exposure.** The Ingress routes `/api`, `/storage` (local storage) and `/` only. Only the `avatars` and `content` buckets are public; the audit archive never is.

## 11. Validation and CI

Run these before installing, with a values file of your own:

```bash
helm lint helm/notturni -f my-solo.yaml
helm template notturni helm/notturni -n notturni -f my-solo.yaml | less
```

The CI job `helm` (`.github/workflows/ci.yml`) installs Helm 3.19.0 and runs `helm lint` and `helm template` for every `helm/notturni/ci/*-values.yaml` (solo, platform, full). It then runs `cargo test --locked -p noct-api --test helm_chart --test deploy_configs`. The chart tests check, among others:

- the solo render has exactly three workloads (PostgreSQL, backend, frontend) and no Redis, NATS, worker, Garage, moderation or wildcard objects;
- the platform render contains every component, two `IngressRoute`s, three `NetworkPolicy` objects and the expected `NOCT_*` values;
- external services, an existing Secret and existing claims render correctly (`full`);
- `image_moderation` and `moderation.enabled` stay consistent;
- invalid combinations fail to render with a clear message;
- every `NOCT_*` variable the chart emits is read by the stack;
- the files in `helm/notturni/files/` equal their sources in `k8s/`, `NOTES.txt` prints no secret values, and no image uses an architecture-specific or `latest` tag.

Tests that call `helm` are skipped with a warning when it is not installed locally.

### Setup wizard

The Super Admin wizard at `/admin/setup` asks for mode, storage, cache, SMTP, subdomains, moderation and NATS, then prints a `.env` snippet and a Helm `values.yaml` using the real chart keys (`mode`, `ingress.host`, `storage.*`, `redis.enabled`, `nats.enabled`, `smtp.*`, `moderation.enabled`, `subdomains.enabled`, `secrets.existingSecret`). It applies nothing and prints placeholders (`<s3-endpoint-url>`, `<name-of-your-secret>`) that you must replace; it also lists the Secret keys to create. It never prints `modules`, so the chart default list applies. Two differences to know: the wizard's own module list lacks `interests`, which the chart default includes, and it does not print `smtp.fromEmail`.

## 12. Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `helm install` fails with `ingress.host is required` | `ingress.host` empty | Set `ingress.host` to the public host name. |
| Render fails with `storage.type=local ... backend.replicaCount must be 1` | More than one replica with local storage | Set `backend.replicaCount: 1`, or use `storage.type=s3` (with `nats.enabled=true`). |
| Render fails with `nats.enabled=false ... replicaCount must be 1` | Several backend replicas without NATS | Enable NATS or use one replica. |
| Render fails with `subdomains.enabled=true needs mode=platform` | Subdomains in solo | Use `mode: platform`, or turn subdomains off. |
| Render fails for `storage.type=s3` | No `endpointUrl` and no Garage, or external S3 without `publicUrl` | Set `storage.s3.endpointUrl` and `storage.s3.publicUrl`, or enable Garage. |
| Backend `CrashLoopBackOff`, log says `NOCT_JWT_SECRET` too short or "valore d'esempio" | Secret key shorter than 32 bytes, or it contains `cambia-questo` or `changeme` | Generate a real value (`openssl rand -base64 48`), update the Secret, restart the backend. |
| Pods stay `CreateContainerConfigError`, "secret not found" | `secrets.existingSecret` names a Secret that does not exist in the release namespace, or lacks a mandatory key | Create it in the same namespace before the install, with the keys in [section 5](#5-secrets). |
| Pods `Pending` with "unbound PersistentVolumeClaim" | No default `StorageClass`, or `storageClass` wrong | Create or set a class (`persistence.*.storageClass`, K3s: `local-path`); check `kubectl describe pvc`. |
| Worker `Pending` or stuck with local storage and NATS | The worker and the backend need the same RWO volume on one node | Schedule both on one node (`nodeSelector`), or use S3 storage. |
| Upload works but images do not load | `NOCT_LOCAL_STORAGE_PUBLIC_URL` not reachable by the browser, or the CSP `img-src` does not include it | Leave `storage.local.publicUrl` empty (it follows `ingress.host`) or set the real public URL; make sure `/storage` reaches the backend through the Ingress. Not yet verified on a cluster. |
| `/storage/...` returns 404 | Ingress disabled or a custom controller without the `/storage` path; or a private bucket (audit) | Check `kubectl get ingress`. Only `avatars` and `content` are public. |
| Login or CORS errors, cookie not set, wrong redirect | `ingress.host` does not match the host the browser uses, or TLS off while the browser uses https | Set `ingress.host` to the real host. The host feeds `NOCT_CORS_ORIGINS`, the OAuth redirect and the cookie settings. |
| Certificate not issued, browser shows the Traefik default certificate | No cert-manager, wrong `issuerRef`, or no `ingress.tls.secretName` Secret | Install cert-manager and check `kubectl describe certificate`; or create the TLS Secret named `<fullname>-tls` (or `ingress.tls.secretName`). |
| Wildcard certificate stays pending | Wildcard needs DNS-01; the Cloudflare token is missing | Add `NOCT_CLOUDFLARE_API_TOKEN` to the Secret and set `createIssuer`. |
| Ingress has no effect on a non-Traefik controller | The `Middleware` and `IngressRoute` need the Traefik CRDs | Set `ingress.traefik.securityHeaders=false`, `ingress.className` to your class, and do not enable subdomains or Garage. |
| After reinstall, the backend cannot log in to PostgreSQL | The old PVC kept the old password; the chart generated a new one | Reinstall with the old Secret (`secrets.existingSecret`) and `existingClaim`, or alter the role password in PostgreSQL. |
| Changed `modules` but nothing changed | The database module table wins after the first start | Change modules in `/admin/modules` ([section 7](#7-modules-and-the-database-wins-caveat)). |
| No email arrives | No SMTP host and no relay: the chart sets no `NOCT_SMTP_*` | Set `smtp.host` or `smtp.relay.enabled`. In solo, use `noct-ctl user reset-password` to recover access. |
| Secret changed but pods use the old value | Pods read the Secret at start only | `kubectl rollout restart` the affected Deployments. |
| `kubectl exec ... noct-ctl user create` hangs | stdin is empty or not closed | Pipe the password with `printf`, and use `-i` without `-t`. |

If a symptom is not listed, start with `kubectl -n notturni get pods,pvc,ingress`, then `kubectl describe` on the failing object and the backend log.
