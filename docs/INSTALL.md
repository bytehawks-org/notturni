# Installazione di Notturni

Guide passo passo per ogni modo di installare la piattaforma. Per un primo giro di prova in locale, con il
percorso di test dalla registrazione alla pubblicazione di un post, vedi [GETTING_STARTED.md](../GETTING_STARTED.md);
per la configurazione di ogni variabile, [`.env.example`](../.env.example); per le operazioni quotidiane,
[BACKEND.md](BACKEND.md).

| Scenario | Quando sceglierlo | Sezione |
| --- | --- | --- |
| Podman / Docker Compose | prova, sviluppo, piccola istanza su un solo host | [2](#2-podman-e-docker-compose) |
| K3s (Kubernetes) | produzione su un nodo, espandibile a cluster | [3](#3-k3s) |
| VM Debian / Ubuntu | produzione senza container, servizi di sistema | [4](#4-vm-senza-container-debian-e-ubuntu) |
| VM Rocky / AlmaLinux | come sopra, famiglia RHEL con SELinux e firewalld | [5](#5-vm-senza-container-rocky-e-almalinux) |

Gli esempi usano `example.com` (dominio) e `192.168.x.x` (reti): sostituirli con i propri.

## 1. Prima di cominciare (comune a tutte le soluzioni)

### 1.1 Architettura in breve

```text
browser ──► reverse proxy (TLS) ──┬─ /api/*      ──► noct-api    :8000 ──┬─► PostgreSQL 16+
                                  ├─ /storage/*  ──► noct-api            ├─► Redis 7+
                                  └─ tutto il resto ► frontend :3000     ├─► NATS JetStream
                                                                         └─► storage (S3/Garage o filesystem)
                                                  noct-worker (nessuna porta): outbox, job periodici, consumer
```

`noct-api`, `noct-worker` e `noct-ctl` sono tre binari della stessa immagine/build. Il **worker è obbligatorio**
quando c'è NATS: senza, email (reset password, MFA), newsletter, backup e manutenzioni non partono.
Il servizio `moderation/` (Python, classificazione delle immagini) è **opzionale**: senza,
`NOCT_MODERATION_SERVICE_URL` resta vuoto e le immagini non vengono classificate.

### 1.2 Requisiti

| Risorsa | Minimo (prova / `solo`) | Consigliato (piattaforma) |
| --- | --- | --- |
| CPU / RAM | 2 vCPU / 2 GB | 4 vCPU / 4 GB (8 GB se si compila sulla VM) |
| Disco | 20 GB | 50 GB + volume dati dedicato |
| Architettura | x86_64 o ARM64 | uguale |
| Rete in ingresso | 80, 443 | 80, 443 (22 solo da rete di gestione) |

Servono in ogni caso: un dominio con record DNS `A`/`AAAA` verso il server (per i blog su sottodominio,
`nomeutente.example.com`, anche un record wildcard `*.example.com`), un server SMTP raggiungibile per le email
(reset password, codici MFA, newsletter) e, per i certificati wildcard, un DNS con API per la sfida DNS-01.

### 1.3 Segreti: `scripts/gen-env.sh`

Il backend **rifiuta di partire** con i segnaposto di `.env.example` (`NOCT_JWT_SECRET`) e non crea il Super Admin
con la password d'esempio. Il file `.env` si crea con segreti casuali:

```bash
git clone https://github.com/bytehawks-org/notturni.git && cd notturni
scripts/gen-env.sh            # crea .env (permessi 600) e stampa la password del Super Admin
```

Poi si modificano a mano le variabili che dipendono dall'installazione:

| Variabile | Valore tipico in produzione |
| --- | --- |
| `NOCT_INSTANCE_FQDN` | `example.com` |
| `NOCT_SUPER_ADMIN_EMAIL` | l'email dell'amministratore |
| `NOCT_DEPLOYMENT_MODE` | `platform` (multiutente) o `solo` (un proprietario, registrazione chiusa dopo il primo utente) |
| `NOCT_CORS_ORIGINS` | `https://example.com` |
| `NOCT_CORS_ORIGIN_REGEX` | `https://([a-z0-9-]+[.])?example[.]com` per i blog su sottodominio (`[.]` e non `\.`: funziona anche in `EnvironmentFile` di systemd) |
| `NOCT_SESSION_COOKIE_SECURE` | `true` (con https; `false` solo in prova su http) |
| `NOCT_SMTP_HOST` / `_PORT` / `_USER` / `_PASSWORD` / `_FROM_EMAIL` | il proprio provider SMTP |
| `NOCT_TRUSTED_PROXIES` | numero di proxy davanti all'API (1 con un solo reverse proxy). **`0` se l'API è esposta direttamente**: altrimenti un client può falsificare `X-Forwarded-For` e aggirare i limiti per IP |
| `NOCT_FRONTEND_REVALIDATE_URL` | URL interno del frontend + `/api/revalidate` |
| `NOCT_BACKUP_S3_*` | storage S3 **esterno** per il backup giornaliero (vuoto = backup disattivato) |

Conservare `.env` fuori da git e in un luogo sicuro: **`NOCT_SECRETS_KEY` non va perso né ruotato** (cifra i
segreti TOTP; senza, gli utenti con MFA non entrano più). Dopo la prima installazione cambiare la password del
Super Admin dall'interfaccia.

### 1.4 Verifica comune a fine installazione

```bash
curl -s https://example.com/api/v1/health/ready
# {"services":{"bus":{...},"cache":{...},"database":{...},"storage":{...}},"status":"ok"}
```

Poi login su `https://example.com` con `superadmin` e la password stampata da `gen-env.sh`
(**il login è per email**: `POST /api/v1/auth/login` con `{"email": …}`).

---

## 2. Podman e Docker Compose

Lo stack completo (Postgres, Redis, NATS, Garage + bootstrap, proxy delle letture pubbliche, moderazione,
MailHog, backend, worker, frontend) è in [`compose.yaml`](../compose.yaml). È pensato per **sviluppo e prova**:
pubblica le porte dei servizi sull'host e usa MailHog al posto di un SMTP reale.

### 2.1 Prerequisiti

| | Podman | Docker |
| --- | --- | --- |
| Debian / Ubuntu | `sudo apt install podman podman-compose` | [docs.docker.com/engine/install](https://docs.docker.com/engine/install/) (plugin `docker compose`) |
| Fedora / Rocky / Alma | `sudo dnf install podman podman-compose` | idem, repository Docker CE |
| Verifica | `podman compose version` | `docker compose version` |

Se `podman compose` non è disponibile, usare `podman-compose` (stessa sintassi). Con Docker sostituire
`podman compose` con `docker compose`: il file è identico. Git serve per clonare il repository.

### 2.2 Quickstart (5 comandi)

```bash
git clone https://github.com/bytehawks-org/notturni.git && cd notturni
scripts/gen-env.sh                      # segreti casuali; annotare la password del Super Admin
podman compose up -d --build            # (docker compose up -d --build)
podman compose ps                       # garage-bootstrap termina da solo con exit 0: è normale
curl -s http://localhost:8000/api/v1/health/ready
```

La prima volta il build del backend Rust richiede diversi minuti. Servizi raggiungibili:

| Servizio | URL |
| --- | --- |
| Frontend (dashboard e amministrazione) | <http://localhost:3000> |
| API / OpenAPI | <http://localhost:8000/api/v1/openapi.json> |
| Email di sviluppo (MailHog) | <http://localhost:8025> |
| Letture pubbliche dello storage (avatar, media) | <http://localhost:9000> |

Login con `superadmin` e la password stampata da `gen-env.sh`. Il resto del percorso di prova è in
[GETTING_STARTED.md](../GETTING_STARTED.md).

### 2.3 Operazioni

```bash
podman compose logs -f backend worker          # log
podman compose exec backend noct-ctl migrate status
podman compose exec backend noct-ctl token create --name core-engine   # token API machine-to-machine
podman compose up -d --build backend worker    # aggiornamento dopo un git pull
podman compose down                            # ferma (i dati restano nei volumi)
podman compose down -v                         # ferma e CANCELLA i dati
```

### 2.4 Da prova a istanza su un solo host

`compose.yaml` si può usare anche per una piccola istanza pubblica, a patto di:

1. **Reverse proxy con TLS** sull'host (Caddy, nginx o Traefik; vedi l'esempio Caddy in [4.9](#49-reverse-proxy-e-tls)),
   che inoltra `/api/*` a `localhost:8000`, `/storage/*` (solo con `localstorage`) a `localhost:8000` e il resto a
   `localhost:3000`; al servizio `frontend` in `compose.yaml` si passano, come variabili d'ambiente (lette a runtime),
   `NOCT_SITE_URL=https://example.com` e, per cambiare il nome, `NOCT_SITE_NAME`. Vanno tolti i build-arg
   `NEXT_PUBLIC_*` pensati per `localhost`.
2. **Chiudere le porte inutili**: togliere (o legare a `127.0.0.1:`) i `ports:` di `postgres`, `redis`, `nats`,
   `garage`, `mailhog`, `smtp-relay`, `moderation`; lasciare raggiungibili dall'esterno solo il reverse proxy.
   Il firewall dell'host deve bloccare tutto il resto.
3. **SMTP reale**: in `.env` e nel blocco `environment` del servizio `backend` sostituire MailHog
   (`NOCT_SMTP_HOST`, `_PORT`, `_USE_TLS`, credenziali), o usare il servizio `smtp-relay`.
4. **Impostare** `NOCT_SESSION_COOKIE_SECURE=true`, `NOCT_CORS_ORIGINS`, `NOCT_OAUTH_REDIRECT_BASE_URL`,
   `NOCT_S3_PUBLIC_URL` (URL pubblico del proxy delle letture, es. `https://s3.example.com`) e
   `NOCT_FRONTEND_URL` ai valori https reali; togliere i valori `localhost` dagli `environment:` del compose.
5. **Backup** verso uno storage S3 realmente esterno (`NOCT_BACKUP_S3_*`, rimuovendo i default che puntano a
   Garage locale).
6. Avvio automatico al boot: con Podman rootless, `loginctl enable-linger $USER` e
   `podman generate systemd` / [Quadlet](https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html);
   con Docker, `restart: unless-stopped` e `systemctl enable docker`.

Per un'installazione di produzione strutturata si consigliano K3s ([3](#3-k3s)) o le VM ([4](#4-vm-senza-container-debian-e-ubuntu), [5](#5-vm-senza-container-rocky-e-almalinux)).

---

## 3. K3s

I manifest sono in [`k8s/`](../k8s/README.md) (Kustomize). Un nodo singolo è sufficiente all'inizio; i `Deployment`
di backend e worker sono già replicabili. Usano Traefik (incluso in K3s), il provisioner `local-path` per i volumi
(sostituibile con Longhorn) e cert-manager per i certificati.

### 3.1 Prerequisiti

* VM o server con 4 vCPU / 4 GB RAM, Debian/Ubuntu/Rocky/Alma, accesso root, porte 80/443 (e 6443 solo da rete di gestione).
* Dominio con record `example.com` e `*.example.com` verso l'IP del nodo.
* Per il wildcard: DNS gestito da Cloudflare (la sfida DNS-01 dei manifest è configurata per Cloudflare; per altri
  provider adattare `k8s/cert-manager-issuer.yaml`).
* Un registro dove pubblicare le immagini (GHCR via CI, vedi 3.4) **oppure** buildarle sul nodo.

### 3.2 Installare K3s

```bash
curl -sfL https://get.k3s.io | sh -
sudo kubectl get nodes                       # Ready
mkdir -p ~/.kube && sudo cp /etc/rancher/k3s/k3s.yaml ~/.kube/config && sudo chown $USER ~/.kube/config
```

Su **Rocky/Alma** con SELinux attivo installare prima il pacchetto di policy se `get.k3s.io` lo segnala
(`k3s-selinux`, lo script lo propone da sé) e aprire il firewall:

```bash
sudo firewall-cmd --permanent --add-service={http,https} --add-port=6443/tcp
sudo firewall-cmd --permanent --zone=trusted --add-source=10.42.0.0/16 --add-source=10.43.0.0/16
sudo firewall-cmd --reload
```

Su **Ubuntu** con `ufw`: `sudo ufw allow 80,443/tcp && sudo ufw allow 6443/tcp`.

### 3.3 cert-manager

```bash
kubectl apply -f https://github.com/cert-manager/cert-manager/releases/download/v1.16.2/cert-manager.yaml
kubectl -n cert-manager rollout status deploy/cert-manager-webhook
```

### 3.4 Immagini

I manifest puntano a `ghcr.io/bytehawks-org/notturni-{backend,frontend,moderation}` con il tag `0.2.0-rust`
(`images:` in `k8s/kustomization.yaml`), pubblicato dal workflow `publish-images` a ogni push sul branch
`migration/rust`. Le immagini precedenti (tag `sha-…`) sono quelle Python e non vanno usate.

**A. CI su GitHub** (consigliato): l'immagine frontend è agnostica rispetto al dominio (URL e nome si
configurano a runtime nel ConfigMap: `NOCT_INSTANCE_FQDN`/`NOCT_SITE_URL`, `NOCT_SITE_NAME`). Lanciare il workflow (push sul branch o *Run workflow*
su `migration/rust`) e attendere le tre build (backend e moderazione arm64 sotto QEMU sono lente). Il pacchetto
GHCR deve essere leggibile dal cluster: pubblico, oppure `imagePullSecrets` con un token `read:packages`.
Per release successive il workflow pubblica anche `sha-<commit>` e, sui tag `vX.Y.Z`, la versione semver:
aggiornare i tre `newTag`.

**B. Build locale sul nodo** (senza registro):

```bash
sudo dnf install -y podman || sudo apt install -y podman      # una delle due
TAG=local-1
podman build -t ghcr.io/bytehawks-org/notturni-backend:$TAG .
podman build -t ghcr.io/bytehawks-org/notturni-frontend:$TAG ./frontend
podman build -t ghcr.io/bytehawks-org/notturni-moderation:$TAG ./moderation       # opzionale
for i in backend frontend moderation; do
  podman save ghcr.io/bytehawks-org/notturni-$i:$TAG | sudo k3s ctr images import -
done
```

poi `newTag: local-1` per le tre immagini in `k8s/kustomization.yaml`. Senza moderazione, togliere `moderation.yaml`
da `resources:` e svuotare `NOCT_MODERATION_SERVICE_URL` in `configmap.yaml`.

### 3.5 Configurazione

```bash
cd k8s
cp secret.example.yaml secret.yaml       # già in .gitignore: non committarlo
```

1. **`secret.yaml`**: sostituire **tutti** i valori d'esempio (`foo`, `changeme…`, `000…`). Generarli con
   `openssl rand -base64 48` (`NOCT_JWT_SECRET`), `openssl rand -base64 32` (`NOCT_SECRETS_KEY`,
   `NOCT_GARAGE_ADMIN_TOKEN`), `openssl rand -hex 32` (`NOCT_GARAGE_RPC_SECRET`,
   `NOCT_S3_SECRET_ACCESS_KEY`), `echo GK$(openssl rand -hex 12)` (`NOCT_S3_ACCESS_KEY_ID`, da replicare anche in
   `configmap.yaml`). Con i segnaposto il backend non parte.
2. **`configmap.yaml`**: `NOCT_INSTANCE_FQDN`, `NOCT_CORS_ORIGINS`, `NOCT_CORS_ORIGIN_REGEX`,
   `NOCT_OAUTH_REDIRECT_BASE_URL`, `NOCT_S3_PUBLIC_URL`, `NOCT_SMTP_*`, `NOCT_SMTP_FROM_EMAIL`,
   `NOCT_TURNSTILE_SITE_KEY` (**sostituire con la propria chiave**), `NOCT_BACKUP_S3_*`.
3. **Dominio**: sostituire `notturni.eu` con `example.com` in `ingress.yaml`, `ingressroute.yaml`,
   `ingressroute-s3.yaml`, `certificate.yaml` e `cert-manager-issuer.yaml` (email di Let's Encrypt e zona).
   `grep -rn notturni.eu k8s/` elenca tutti i punti.
4. **Cloudflare**: token API con permesso `Zone:DNS:Edit` sulla sola zona, nella chiave
   `NOCT_CLOUDFLARE_API_TOKEN` di `secret.yaml`.

### 3.6 Applicare e verificare

```bash
kubectl apply -k .
kubectl -n notturni get pods -w                  # tutti Running; il backend attende Garage via initContainer
kubectl -n notturni get certificate              # READY=True dopo qualche minuto (DNS-01)
curl -s https://example.com/api/v1/health/ready
```

Il Super Admin viene creato al primo avvio da `NOCT_SUPER_ADMIN_*` (nel `secret.yaml`). I `Deployment` usano
`runAsNonRoot` e una `NetworkPolicy` chiude tutto l'ingresso tranne frontend, backend e `s3-public` dal solo
Traefik (`kube-system`).

### 3.7 Operazioni

| Cosa | Comando |
| --- | --- |
| Log | `kubectl -n notturni logs deploy/backend -f` · `deploy/worker` |
| Stato migrazioni | `kubectl -n notturni exec deploy/backend -- noct-ctl migrate status` |
| Token machine-to-machine | `kubectl -n notturni exec deploy/backend -- noct-ctl token create --name core-engine` |
| Aggiornare | nuovo `newTag` in `kustomization.yaml`, poi `kubectl apply -k .` (le migrazioni partono da sole) |
| Più repliche | `kubectl -n notturni scale deploy/backend --replicas=3` (idem `worker`: lo scheduler è a elezione atomica) |
| Rimuovere tutto | `kubectl delete -k .` (i PVC restano finché non si cancellano) |

---

## 4. VM senza container: Debian e Ubuntu

Installazione nativa su una VM Debian 12/13 o Ubuntu 22.04/24.04: servizi di sistema (`systemd`), nessun
container. Tutti i comandi come utente con `sudo`.

### 4.1 Pacchetti di base

```bash
sudo apt update && sudo apt install -y ca-certificates curl git openssl build-essential cmake pkg-config \
  postgresql postgresql-client redis-server ufw
psql --version     # 16 o superiore; su Debian 12 (15) installare da apt.postgresql.org (PGDG) il pacchetto postgresql-16
```

Opzionale, per il frontend: Node.js 22 (vedi 4.7). Il client `postgresql-client` fornisce `pg_dump`, usato dal modulo
`backup`.

### 4.2 PostgreSQL

```bash
DBPASS=$(openssl rand -hex 16); echo "$DBPASS"        # annotarla: va in NOCT_POSTGRES_PASSWORD
sudo -u postgres psql -c "CREATE ROLE notturni LOGIN PASSWORD '$DBPASS'"
sudo -u postgres psql -c "CREATE DATABASE notturni OWNER notturni"
```

Postgres resta in ascolto solo su `localhost` (default): non aprire la 5432 sul firewall. Nessuna estensione
da installare e nessun superuser richiesto dall'applicazione.

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

### 4.5 Utente di servizio e directory

```bash
sudo useradd --system --home /opt/notturni --create-home --shell /usr/sbin/nologin notturni
sudo install -d -o notturni -g notturni -m750 /var/lib/notturni /var/lib/notturni/storage
sudo install -d -m750 -g notturni /etc/notturni
```

### 4.6 Backend: binari

Due strade; la seconda evita di compilare sulla VM (la compilazione release richiede ~4 GB di RAM, 10 GB di disco e
diversi minuti).

**A. Compilare dal sorgente**

```bash
curl -fsSL https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.97 && . ~/.cargo/env
git clone https://github.com/bytehawks-org/notturni.git ~/notturni && cd ~/notturni
cargo build --release --locked -p noct-api -p noct-worker -p noct-ctl
sudo install -m755 target/release/{noct-api,noct-worker,noct-ctl} /usr/local/bin/
```

**B. Estrarre i binari dall'immagine** (stessa architettura e libc dell'immagine: Debian 13 «trixie» o successiva)

```bash
sudo apt install -y podman
ID=$(podman create ghcr.io/bytehawks-org/notturni-backend:TAG)         # TAG: vedi 3.4
sudo podman cp $ID:/usr/local/bin/. /usr/local/bin/ && podman rm $ID
```

### 4.7 Configurazione e frontend

Generare `.env` come in [1.3](#13-segreti-scriptsgen-envsh) (dalla copia del repository), poi adattarlo alla VM:

```bash
cd ~/notturni && scripts/gen-env.sh /tmp/notturni.env
```

Valori da impostare/sostituire in `/tmp/notturni.env` (poi spostato in `/etc/notturni/notturni.env`):

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
# storage su filesystem (consigliato su una VM singola); per Garage/S3 vedi 4.10
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

Le variabili `NOCT_S3_*`, `NOCT_GARAGE_*` e `NOCT_S3_PUBLIC_URL` possono restare: con `localstorage` sono ignorate.

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

URL del sito, dell'API e nome si leggono a runtime dalle env del processo (`NOCT_SITE_URL`, `NOCT_API_PUBLIC_URL`, `NOCT_SITE_NAME`): cambiare dominio non richiede un nuovo build.

### 4.8 Servizi systemd

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
journalctl -u noct-api -n 30 --no-pager        # "in ascolto … 127.0.0.1:8000"
curl -s http://127.0.0.1:8000/api/v1/health/ready
```

Le migrazioni e la creazione del Super Admin avvengono al primo avvio dell'API. Il worker legge lo stesso file
e **deve** condividere lo stesso `NOCT_JWT_SECRET`, `NOCT_SECRETS_KEY` e database.

### 4.9 Reverse proxy e TLS

Esempio con **Caddy** (TLS automatico con Let's Encrypt):

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

Per i **blog su sottodominio** (`nomeutente.example.com`) serve un certificato wildcard, quindi la sfida DNS-01:
usare un Caddy con il plugin del proprio DNS (build da [caddyserver.com/download](https://caddyserver.com/download),
es. `caddy-dns/cloudflare`) e dichiarare `example.com, *.example.com { tls { dns cloudflare {env.CF_API_TOKEN} } … }`
con lo stesso corpo. Senza wildcard l'installazione funziona sul solo dominio principale (adatta a `solo`).

```bash
sudo systemctl reload caddy
```

### 4.10 Garage / S3 invece del filesystem (opzionale)

Con `NOCT_STORAGE_BACKEND=s3` servono un endpoint S3-compatible (Garage in locale, oppure AWS S3 / Cloudflare R2 /
altro) e `NOCT_S3_ENDPOINT_URL`, `NOCT_S3_REGION`, `NOCT_S3_ACCESS_KEY_ID`, `NOCT_S3_SECRET_ACCESS_KEY`,
`NOCT_S3_PUBLIC_URL`. Per Garage sulla VM: installare il binario ([garagehq.deuxfleurs.fr](https://garagehq.deuxfleurs.fr/)),
usare come modello [`garage/garage.toml`](../garage/garage.toml), poi `noct-ctl garage-bootstrap`
(con `NOCT_GARAGE_ADMIN_URL=http://127.0.0.1:3903`) crea layout, chiave e bucket. Le letture pubbliche richiedono il
proxy che espone solo i prefissi pubblici (vedi [`s3-public/default.conf.template`](../s3-public/default.conf.template)).

### 4.11 Firewall e hardening

```bash
sudo ufw default deny incoming && sudo ufw allow OpenSSH && sudo ufw allow 80,443/tcp && sudo ufw enable
sudo ss -ltnp | grep -E ':(5432|6379|4222|8000|3000)\b'     # tutti su 127.0.0.1, mai su 0.0.0.0
```

### 4.12 Backup, aggiornamento, diagnosi

| Cosa | Come |
| --- | --- |
| Backup giornaliero | imposta `NOCT_BACKUP_S3_*` (storage S3 **esterno**): il job `backup.run` fa `pg_dump` e rispecchia i bucket. Con `localstorage` copiare anche `/var/lib/notturni/storage` (es. `restic`/`rsync`) |
| Dump manuale | `sudo -u postgres pg_dump -Fc notturni > notturni-$(date +%F).dump` |
| Aggiornare | `cd ~/notturni && git pull && cargo build --release --locked -p noct-api -p noct-worker -p noct-ctl && sudo install -m755 target/release/{noct-api,noct-worker,noct-ctl} /usr/local/bin/ && sudo systemctl restart noct-api noct-worker` (frontend: rifare `npm run build` e ricopiare come in 4.7) |
| Log | `journalctl -u noct-api -u noct-worker -u notturni-frontend -f` |
| `noct-ctl` (migrazioni, messaggi falliti, token API) | `sudo systemd-run --pty --collect -p User=notturni -p EnvironmentFile=/etc/notturni/notturni.env /usr/local/bin/noct-ctl migrate status` (idem `dead-letters`, `token create --name core-engine`): usa lo stesso parser di variabili dei servizi |

---

## 5. VM senza container: Rocky e AlmaLinux

Rocky Linux / AlmaLinux 9 (e 10): stesso schema della sezione 4, con `dnf`, `firewalld` e **SELinux in modalità
enforcing** (lasciarlo attivo). Seguire i passi sotto al posto di 4.1–4.4, 4.9 e 4.11; **4.2 (ruolo e database),
4.5–4.8 e 4.10 sono identici**, con queste differenze: i servizi si chiamano `postgresql` e `redis`
(non `redis-server`), e nelle unit `After=` va usato `postgresql.service redis.service`.

### 5.1 Pacchetti

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

Se il modulo `redis:7` non è disponibile (Rocky/Alma 10), `sudo dnf install -y redis` (o `valkey`, compatibile).
`pg_dump` è nel pacchetto `postgresql`. Poi creare ruolo e database come in 4.2.

### 5.2 NATS

Come 4.4, con `ARCH=amd64` (x86_64) oppure `arm64` (aarch64) al posto di `dpkg --print-architecture`, e dopo aver
installato il binario assegnare il contesto SELinux corretto:

```bash
sudo restorecon -v /usr/local/bin/nats-server /usr/local/bin/noct-*
```

### 5.3 Binari, frontend, servizi

* Build: `curl -fsSL https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.97` e `cargo build --release --locked …`
  come in 4.6-A (i pacchetti di compilazione sono già in 5.1). I binari presi dall'immagine (4.6-B) sono
  costruiti su Debian: funzionano solo se la glibc della VM è sufficientemente recente (Rocky/Alma 9 ha 2.34:
  **compilare dal sorgente**).
* Node.js 22: `sudo dnf module enable -y nodejs:22 && sudo dnf install -y nodejs`, poi come in 4.7.
* Unit systemd: identiche a 4.8; `/usr/bin/node` è lo stesso percorso.

### 5.4 SELinux

Il percorso `/opt/notturni/frontend` e `/var/lib/notturni` sono accessibili ai servizi `systemd` con il contesto
predefinito; il solo permesso aggiuntivo serve al **reverse proxy** per contattare i backend locali:

```bash
sudo setsebool -P httpd_can_network_connect 1          # nginx/Apache; Caddy installato da pacchetto, se confinato
sudo semanage fcontext -a -t var_lib_t "/var/lib/notturni(/.*)?" && sudo restorecon -Rv /var/lib/notturni
sudo ausearch -m avc -ts recent | audit2why             # se un servizio non parte: cosa nega SELinux
```

Non usare `setenforce 0` per «far funzionare» l'installazione: leggere l'AVC con `audit2why` e correggere il contesto.

### 5.5 Reverse proxy

Caddy ha un repository COPR: `sudo dnf install -y 'dnf-command(copr)' && sudo dnf copr enable -y @caddy/caddy && sudo dnf install -y caddy`,
poi lo stesso `Caddyfile` di 4.9 e `sudo systemctl enable --now caddy`. In alternativa nginx
(`sudo dnf install -y nginx`) con `proxy_pass http://127.0.0.1:8000` per `/api/` e `/storage/`, e
`proxy_pass http://127.0.0.1:3000` per `/`, impostando `proxy_set_header X-Forwarded-For $remote_addr;`
(il backend si fida dell'ultimo valore: con `NOCT_TRUSTED_PROXIES=1`) e `proxy_set_header Host $host;`.
I certificati con `certbot --nginx` (dominio principale) o `certbot` con plugin DNS (wildcard).

### 5.6 Firewall

```bash
sudo firewall-cmd --permanent --add-service={http,https} && sudo firewall-cmd --reload
sudo firewall-cmd --list-all                                  # solo ssh, http, https
sudo ss -ltnp | grep -E ':(5432|6379|4222|8000|3000)\b'       # tutti su 127.0.0.1
```

### 5.7 Aggiornamento, backup, diagnosi

Come 4.12, con `journalctl`/`systemctl` identici. Per il dump: `sudo -u postgres pg_dump -Fc notturni > …`.

---

## 6. Verifica funzionale dopo l'installazione

```bash
B=https://example.com/api/v1
curl -s $B/health/ready                                                         # status ok
curl -s -c /tmp/c -H 'content-type: application/json' \
  -d '{"email":"admin@example.com","password":"<password Super Admin>"}' $B/auth/login
curl -s $B/config                                                               # configurazione pubblica
```

Poi da browser: login, creazione di un blog, pubblicazione di un post, apertura della pagina pubblica; da
`/dashboard` (amministrazione) controllare **Panoramica**: i servizi `postgres`, `cache`, `bus`, `storage` devono
essere `ok`. Verificare che un'email di prova (reset password) arrivi: se no, `journalctl -u noct-worker`.

## 7. Problemi comuni

| Sintomo | Causa probabile | Rimedio |
| --- | --- | --- |
| Il backend esce subito con «`NOCT_JWT_SECRET` è il valore d'esempio» | `.env` copiato da `.env.example` | `scripts/gen-env.sh` o `openssl rand -base64 48` |
| Il Super Admin non esiste (log: «creazione del Super Admin non riuscita») | password d'esempio o troppo corta | impostare `NOCT_SUPER_ADMIN_PASSWORD` e riavviare |
| Login non riuscito con `username` | l'autenticazione è per **email** | usare l'email |
| 429 sul login dopo pochi tentativi | limite per email/IP (anti brute-force) | attendere la finestra; dietro un proxy controllare `NOCT_TRUSTED_PROXIES` |
| Tutti gli utenti appaiono con lo stesso IP | `NOCT_TRUSTED_PROXIES` troppo basso/alto per la catena di proxy | impostare il numero di proxy fidati reale |
| Email, newsletter, backup non partono | `noct-worker` non in esecuzione, o SMTP/`NOCT_BACKUP_S3_*` non impostati | avviare il worker; `noct-ctl dead-letters` |
| Il browser non resta autenticato | cookie `Secure` su http, o origine non in `NOCT_CORS_ORIGINS` | https + `NOCT_SESSION_COOKIE_SECURE=true`, oppure `false` in prova su http |
| Immagini/avatar non si vedono | `NOCT_S3_PUBLIC_URL` / `NOCT_LOCAL_STORAGE_PUBLIC_URL` errati, o `/storage` non inoltrato | correggere l'URL pubblico e il reverse proxy |
| Il browser chiama il dominio sbagliato | `NOCT_SITE_URL`/`NOCT_INSTANCE_FQDN` (o `NOCT_API_PUBLIC_URL`) non impostati o errati nel ConfigMap | correggerli e riavviare il Deployment `frontend` (nessun rebuild) |
| `ImagePullBackOff` su K3s | il tag non è stato pubblicato (workflow non concluso), pacchetto GHCR privato o immagine non importata | vedi 3.4 |
| Pod non partono su K3s con NetworkPolicy | CNI senza supporto alle policy | usare il CNI di K3s (default) o rimuovere `networkpolicy.yaml` |
| Utenti con MFA non entrano più dopo un cambio di segreto | `NOCT_SECRETS_KEY` perso/ruotato | ripristinare la chiave originale |
