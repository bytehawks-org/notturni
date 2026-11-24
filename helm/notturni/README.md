# Notturni Helm chart

Installs Notturni on Kubernetes. The default profile is the lean **solo** stack: PostgreSQL, the backend (1 replica, uploaded files on a PVC, in-process bus and inline worker), the frontend and a Traefik Ingress. Redis, NATS with a separate worker, Garage, image moderation, an SMTP relay and per-blog subdomains are switched on with values (the **platform** profile).

The chart lives next to the Kustomize manifests in [`k8s/`](../../k8s/README.md), which stay the reference for the full platform deployment. Pick one of the two for an installation; do not apply both to the same namespace. Chart `version` is `0.1.0`, `appVersion` is `0.2.0-rust`, `kubeVersion` is `>=1.27.0-0`. The chart was checked with Helm 3.19.0 (`helm lint`, `helm template`, the `helm_chart.rs` tests and the CI job `helm`); it has **not** been installed on a real cluster yet. The manual checklist is at the end of this file.

The full deployment guide (values reference, secrets, backup and restore, operations, troubleshooting) is [docs/HELM.md](../../docs/HELM.md). For what each profile means and the operating caveats (MemoryBus, moderation, collaborators, password recovery), see [docs/CONFIGURATION.md](../../docs/CONFIGURATION.md#deployment-profiles-solo-and-platform).

## Prerequisites

- Kubernetes 1.27 or newer; K3s on a single node is the target.
- Traefik as Ingress controller (bundled with K3s). The security-headers `Middleware` and the wildcard `IngressRoute` use the Traefik CRDs (`traefik.io/v1alpha1`); on another controller set `ingress.traefik.securityHeaders=false` and do not enable subdomains.
- A default `StorageClass` (K3s: `local-path`), or set `persistence.*.storageClass`.
- A public host name (`ingress.host`, required) with DNS pointing at the node. For TLS: a Secret with the certificate, or cert-manager (not part of K3s; see [k8s/README.md](../../k8s/README.md#tls-certificates-cert-manager)).
- Helm 3.

## Install

Solo, with a Secret you manage (recommended):

```bash
kubectl create namespace notturni
kubectl -n notturni create secret generic notturni-secrets \
  --from-literal=NOCT_POSTGRES_PASSWORD="$(openssl rand -hex 16)" \
  --from-literal=NOCT_JWT_SECRET="$(openssl rand -base64 48)" \
  --from-literal=NOCT_REVALIDATE_SECRET="$(openssl rand -hex 24)"

cat > my-solo.yaml <<'EOF'
mode: solo
ingress:
  host: example.com
smtp:
  host: smtp.example.com      # optional; leave out to send no email
secrets:
  existingSecret: notturni-secrets
EOF

helm lint helm/notturni -f my-solo.yaml
helm install notturni helm/notturni -n notturni -f my-solo.yaml
kubectl -n notturni get pods,pvc,ingress
```

The first registered user becomes the Super Admin (solo closes registration after that). To create the Super Admin from values instead, set `admin.email`; its password is then generated in the chart Secret, or taken from the key `NOCT_SUPER_ADMIN_PASSWORD` of `existingSecret`. If `existingSecret` lacks that key, the backend starts without creating the Super Admin (it logs a warning). With a chart-generated Secret `NOTES.txt` prints the command that reads it; passwords are never printed.

Platform, with the bundled components (this is `ci/platform-values.yaml`, trimmed):

```yaml
mode: platform
ingress:
  host: example.com
  tls:
    certManager:
      enabled: true
      createIssuer: { enabled: true, email: admin@example.com }
subdomains: { enabled: true }
storage:
  type: s3
  s3: { garage: { enabled: true } }
redis: { enabled: true }
nats: { enabled: true }
moderation: { enabled: true }
smtp:
  fromEmail: info@example.com
  relay: { enabled: true, host: smtp.example.com }
modules: []        # empty = backend default set, including image_moderation
backend: { replicaCount: 2 }
frontend: { replicaCount: 2 }
worker: { replicaCount: 2 }
```

`createIssuer` needs the key `NOCT_CLOUDFLARE_API_TOKEN` in the Secret (DNS-01 for the wildcard). Install it the same way, `helm install notturni helm/notturni -n notturni -f my-platform.yaml`.

## Upgrade

```bash
helm upgrade notturni helm/notturni -n notturni -f my-solo.yaml
```

Switching a component on is a values change plus `helm upgrade`, for example `--set redis.enabled=true` adds the Redis Deployment and the `NOCT_REDIS_*` variables. Database migrations run at backend start (`NOCT_AUTO_MIGRATE`, advisory lock). The chart-created Secret keeps its generated keys across upgrades through `lookup`.

Changing `mode`, `storage.type` or `modules` on an installed release does not rewrite the state already in the database: the module table in the database wins over `NOCT_MODULES` after the first start ([details](../../docs/CONFIGURATION.md#existing-database-moved-to-solo)). Moving from local storage to S3 does not copy the existing files.

## Uninstall and data

```bash
helm uninstall notturni -n notturni
```

The PVCs (PostgreSQL, local storage, and with the platform options NATS and Garage) carry `helm.sh/resource-policy: keep` (`persistence.keepOnUninstall: true`, the default), so **uninstall does not delete them**: they hold personal data. Deleting them is a separate, deliberate step (`kubectl -n notturni delete pvc ...`), which is destructive. Setting `keepOnUninstall=false` makes the next uninstall delete them.

Reinstalling over kept PVCs with a freshly generated Secret leaves PostgreSQL with the old password. Use `persistence.postgres.existingClaim` / `persistence.storage.existingClaim` together with `secrets.existingSecret`, so the old data and its credentials stay paired.

## Secrets

Secrets never go in values files. Two ways:

- `secrets.existingSecret`: the name of a Secret you manage. Mandatory keys: `NOCT_POSTGRES_PASSWORD`, `NOCT_JWT_SECRET` (at least 32 random bytes), `NOCT_REVALIDATE_SECRET`. Optional keys: `NOCT_SUPER_ADMIN_PASSWORD`, `NOCT_SMTP_USER`, `NOCT_SMTP_PASSWORD`, `NOCT_S3_ACCESS_KEY_ID`, `NOCT_S3_SECRET_ACCESS_KEY` (with both absent, the AWS default credential chain applies), `NOCT_GARAGE_RPC_SECRET`, `NOCT_GARAGE_ADMIN_TOKEN`, `NOCT_BACKUP_S3_*`, `NOCT_SMTP_RELAY_USERNAME` / `_PASSWORD`, `NOCT_OAUTH_*`, `NOCT_TURNSTILE_SECRET_KEY`, `NOCT_CLOUDFLARE_API_TOKEN`. Keep `NOCT_SECRETS_KEY` there too in production (see [CONFIGURATION.md](../../docs/CONFIGURATION.md#security-and-gdprnis2)).
- Empty `secrets.existingSecret`: the chart creates `<release>-secrets` and generates the mandatory keys on the first install (plus the Garage keys when Garage is on). Extra keys go in `secrets.extra`, passed with `--set-file` or an untracked values file.

GitOps note: the generated Secret relies on `lookup`, which only works against a live cluster (`helm install`/`upgrade`). Tools that only run `helm template` (Argo CD, Flux with rendered manifests) have no `lookup`, so the generated values would change at every render. Use `secrets.existingSecret` there (for example from Sealed Secrets or an external secrets operator).

## Values reference

Defaults are in [`values.yaml`](values.yaml); the schema in [`values.schema.json`](values.schema.json) rejects wrong types.

| Key | Default | Meaning |
|---|---|---|
| `mode` | `solo` | `solo` or `platform` (`NOCT_DEPLOYMENT_MODE`) |
| `site.name`, `site.slug` | `Notturni`, `notturni` | `NOCT_SITE_NAME`, `NOCT_SITE_SLUG` |
| `site.experimentalFeatures`, `site.turnstileSiteKey`, `site.trustedProxies` | empty, empty, `1` | `NOCT_EXPERIMENTAL_FEATURES`, `NOCT_TURNSTILE_SITE_KEY`, `NOCT_TRUSTED_PROXIES` |
| `admin.username`, `admin.email` | `superadmin`, empty | First Super Admin; empty email skips it |
| `ingress.enabled`, `ingress.className`, `ingress.host` | `true`, `traefik`, empty | `host` is required; it becomes `NOCT_INSTANCE_FQDN` |
| `ingress.tls.enabled`, `.secretName` | `true`, `<release>-tls` | TLS and its Secret |
| `ingress.tls.certManager.enabled`, `.issuerRef`, `.createIssuer.*` | `false` | cert-manager Certificate; `createIssuer` makes a Cloudflare DNS-01 Issuer |
| `ingress.traefik.securityHeaders` | `true` | HSTS, nosniff, frame options Middleware |
| `subdomains.enabled` | `false` | Per-blog subdomains; needs `mode=platform` and a wildcard certificate |
| `storage.type` | `local` | `local` (PVC served at `/storage`) or `s3` |
| `storage.local.publicUrl`, `.basePath` | empty, `/data/storage` | Empty URL = `<scheme>://<ingress.host>/storage` |
| `storage.s3.endpointUrl`, `.region`, `.publicUrl`, `.buckets.*` | empty | External S3 or R2; `endpointUrl` and `publicUrl` required without Garage |
| `storage.s3.garage.enabled`, `.publicHost` | `false`, `s3.<ingress.host>` | Bundled Garage + `s3-public` proxy + bootstrap |
| `backup.s3.endpointUrl`, `.region`, `.bucket` | empty | External backup target; empty = job inactive |
| `redis.enabled` | `false` | Redis; off = in-memory cache and no `NOCT_REDIS_*` |
| `nats.enabled` | `false` | NATS JetStream and the `worker` Deployment; off = in-process bus, inline worker |
| `moderation.enabled` | `false` | Moderation service and the `image_moderation` module; off removes the module from `modules` |
| `smtp.host`, `.port`, `.useTls`, `.fromEmail` | empty, `587`, `true` | External SMTP; empty host and no relay = no email |
| `smtp.relay.enabled`, `.host` | `false` | Postfix relay (`NOCT_SMTP_HOST` points at it, port 587, no TLS inside the cluster) |
| `modules` | solo list without `image_moderation` | `NOCT_MODULES` at first start; empty list omits the variable |
| `persistence.keepOnUninstall` | `true` | Keep PVCs on uninstall |
| `persistence.postgres.*`, `persistence.storage.*` | 5Gi, 10Gi | `size`, `storageClass`, `existingClaim`, `accessMode` |
| `persistence.nats.*`, `persistence.garage.*` | 2Gi; 1Gi meta, 10Gi data | Platform options |
| `postgres.external.host`, `.port`, `postgres.user`, `.database` | empty, `5432`, `admin`, `notturni` | External PostgreSQL skips the bundled one; password key `NOCT_POSTGRES_PASSWORD` |
| `secrets.existingSecret`, `secrets.extra` | empty, `{}` | See Secrets |
| `backend.*`, `frontend.*`, `worker.*` | 1 replica | `replicaCount`, `resources`, `nodeSelector`, `tolerations`, `affinity`, `extraEnv` (the worker has `replicaCount` and `resources` only) |
| `networkPolicy.enabled`, `.ingressControllerNamespace` | `false`, `kube-system` | Default-deny ingress; needs a CNI that enforces it |
| `image.*` | pinned tags | Backend and frontend default to `appVersion`; `image.imagePullSecrets`, `image.pullPolicy` |

## Consistency rules

`helm template` and `helm install` fail with a clear message when:

- `mode` is not `solo` or `platform`, `storage.type` is not `local` or `s3`, or `ingress.host` is empty;
- `subdomains.enabled=true` with `mode=solo`;
- `storage.type=local` with `backend.replicaCount` above 1 (the volume is ReadWriteOnce), or `nats.enabled=false` with more than one backend replica (in-process bus and inline worker);
- `storage.type=s3` without `storage.s3.endpointUrl` or Garage, or with an external S3 and no `storage.s3.publicUrl`; Garage with `storage.type=local`;
- `ingress.tls.certManager.enabled` without `ingress.tls.enabled`, or `createIssuer` without an email.

## Local storage behind the Ingress

With `storage.type=local` the backend serves `/storage` from its volume, and the Ingress routes `/storage` to the backend and everything else to the frontend. `NOCT_LOCAL_STORAGE_PUBLIC_URL` defaults to `<scheme>://<ingress.host>/storage`; set `storage.local.publicUrl` to override it. The frontend adds that origin to the CSP `img-src`. Only the avatar and content buckets are public; the audit archive is never served. The Traefik headers Middleware adds `nosniff`; the backend guesses the content type from the file extension. The upload and the public URL were not exercised on a cluster: step 4 of the checklist covers them.

## Persistence, backup and limits

- Single backend replica with local storage: the PVC is ReadWriteOnce, and PostgreSQL always uses `strategy: Recreate`, and the backend and worker use it when storage is local: an upgrade stops the old pod before starting the new one (a short downtime). To scale out, use `storage.type=s3` and `nats.enabled=true`.
- Back up the PostgreSQL PVC and, in solo, the storage PVC: both hold personal data (GDPR). The module `backup` can copy a dump and the buckets to an external S3 (`backup.s3.*`).
- Without NATS, jobs handed to the in-memory bus but not yet consumed are lost on restart; jobs still in the outbox survive ([details](../../docs/CONFIGURATION.md#no-nats-memorybus-limits)).
- Without SMTP, no email is sent. Recover a password with `kubectl -n notturni exec -i deploy/notturni-backend -- noct-ctl user reset-password --email ...` (password on stdin; [details](../../docs/CONFIGURATION.md#password-recovery-without-smtp)).
- Without `moderation.enabled`, sensitive images are not filtered automatically: the operator accepts this ([details](../../docs/CONFIGURATION.md#image-moderation-off)).

## Smoke test checklist (manual)

Run this on a **throwaway** cluster (K3s with Traefik and a StorageClass), never on production. Agents do not run it. Steps marked **DESTRUCTIVE** delete data. Use a host name that resolves to the cluster (DNS or an `/etc/hosts` line) and a values file `my-solo.yaml` like the one above, plus `secrets.existingSecret` or a generated Secret. Release `notturni`, namespace `notturni`; resource names follow `<release>-<component>` (adjust if you set `fullnameOverride`).

| # | Step | Expected result |
|---|---|---|
| 1 | `helm lint helm/notturni -f my-solo.yaml`, then `helm template notturni helm/notturni -n notturni -f my-solo.yaml \| kubectl apply --dry-run=server -f -` | Lint reports `0 chart(s) failed`; the dry run accepts every object (the Traefik `Middleware` needs the Traefik CRDs) |
| 2 | `helm install notturni helm/notturni -n notturni --create-namespace -f my-solo.yaml`, then `kubectl -n notturni get pods,pvc,ingress` | Only postgres, backend and frontend pods, all Ready; two PVCs (postgres, storage) Bound; one Ingress with the host |
| 3 | `kubectl -n notturni logs deploy/notturni-backend` | Migrations applied; inline worker; the storage, cache and bus lines show local, memory and memory; no Redis, NATS or moderation connection attempts |
| 4 | Open `https://<host>/` (or `kubectl -n notturni port-forward svc/notturni-frontend 3000:3000` and <http://localhost:3000>, noting that uploads then need `storage.local.publicUrl` to be reachable). Register the first user. Create a blog (see the note below the table). Upload an image in a post. Open its `https://<host>/storage/...` URL. Publish a post | The first user is the Super Admin; the blog is reachable on a path; the image URL answers `200` with an `image/*` content type; the post is public. A private bucket path (for example the audit bucket) answers `404` |
| 5 | Inspect the response headers and the browser console on the post page | HSTS, `X-Content-Type-Options: nosniff` and frame options present (Traefik Middleware); the CSP lets the image load; the console shows no CSP errors |
| 6 | Create a collaborator: `printf '%s\n' "$PW" \| kubectl -n notturni exec -i deploy/notturni-backend -- noct-ctl user create --email bob@example.com --username bob`, or the form at `/admin/utenti/nuovo`. Invite `bob` as `mediatore` from the blog's collaborators tab, log in as bob and accept | The command prints an id; bob can log in with the given password; the invitation is accepted; registration still answers `409` for others |
| 7 | `kubectl -n notturni rollout restart deploy/notturni-backend`, wait for Ready. Then `printf '%s\n' "$NEW" \| kubectl -n notturni exec -i deploy/notturni-backend -- noct-ctl user reset-password --email bob@example.com` | Posts and the uploaded image are still there (the PVCs persist); the reset prints "Password reimpostata; sessioni revocate."; bob's old session is gone and the new password works |
| 8 | `helm upgrade notturni helm/notturni -n notturni -f my-solo.yaml --set redis.enabled=true` | A Redis pod appears; the backend restarts and its log reports the Redis cache; the site still works |
| 9 | Platform render, **in a separate namespace** and ideally a separate release: `helm template notturni-p helm/notturni -n notturni-p -f helm/notturni/ci/platform-values.yaml`, then the dry run as in step 1. Only install it if the cluster has room (Garage, NATS, moderation need resources and the wildcard needs DNS-01) | Renders without error; with an install, the worker, NATS, Redis, Garage, `s3-public` and moderation pods become Ready and the backend log shows storage s3, cache redis, bus nats |
| 10 | **DESTRUCTIVE.** `helm uninstall notturni -n notturni`, then `kubectl -n notturni get pvc` | Pods, Services and the Ingress are gone; the PVCs remain (expected). Delete the PVCs or the namespace (`kubectl delete namespace notturni`) only after confirming no data is needed |

Note on step 4: solo serves blogs at `/<slug>` only (`NOCT_SUBDOMAINS_ENABLED=false`). Create the blog from the dashboard and open the address the interface shows. The public URL is `scheme://host` without a port: a non-standard port (for example a port-forward on 8443) is not supported, because the browser would call the API on the default port. Use the standard HTTPS port behind the Ingress, or a `--resolve`/hosts mapping to port 443.

Record the results (date, chart version, cluster, pass or fail per step, deviations) in the Work log of the task file for the smoke test, and open an issue for anything that fails.
