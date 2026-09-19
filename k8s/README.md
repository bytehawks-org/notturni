# Notturni – manifest Kubernetes

Primo draft dei manifest per un singolo nodo K3s (vedi
[ROADMAP.md](../ROADMAP.md#3-architettura-stack-e-infrastruttura)). Richiede:
Longhorn (storage class `longhorn`), Traefik come IngressController (entrambi
già inclusi in una installazione K3s standard, salvo li si sia disattivati
esplicitamente).

`ingress.yaml`/`ingressroute.yaml` servono ora **https** sotto il dominio
reale `notturni.eu` (apex + wildcard `*.notturni.eu` per i blog per
sottodominio), con TLS gestito da **cert-manager** (non incluso in questi
manifest, va installato a parte — vedi sotto) tramite una risorsa
`Certificate` esplicita (`certificate.yaml`) e un `ClusterIssuer` Let's
Encrypt con sfida DNS-01 su Cloudflare (`cert-manager-issuer.yaml`,
obbligatoria per il wildcard: la sfida HTTP-01 non lo copre).
`NOCT_SESSION_COOKIE_SECURE="true"` in `configmap.yaml` è coerente con
questo (i due vanno sempre cambiati insieme, mai uno senza l'altro — per
tornare a un test solo-http via IP nodo, senza dominio/cert-manager,
rimuovere i blocchi `tls`/`host` da `ingress.yaml`/`ingressroute.yaml` **e**
riportare quella variabile a `"false"` nello stesso momento).

## Certificati TLS (cert-manager)

```bash
# cert-manager non è incluso in un'installazione K3s standard (a differenza
# di Longhorn/Traefik) — installarlo prima di applicare questi manifest:
helm repo add jetstack https://charts.jetstack.io
helm install cert-manager jetstack/cert-manager \
  --namespace cert-manager --create-namespace --set crds.enabled=true

# token API Cloudflare (permesso Zone:DNS:Edit sulla sola zona notturni.eu)
# in secret.yaml, chiave NOCT_CLOUDFLARE_API_TOKEN — vedi commento lì
```

**Primo test con `letsencrypt-staging`**: Let's Encrypt di produzione ha
limiti stretti (5 certificati duplicati/settimana per dominio esatto).
Cambiare temporaneamente `issuerRef.name` in `certificate.yaml` da
`letsencrypt-prod` a `letsencrypt-staging`, verificare che
`kubectl describe certificate notturni-eu-tls -n notturni` arrivi a `Ready`
(il certificato staging non è fidato dal browser, ma la sua emissione
conferma che il solver DNS-01/il token Cloudflare funzionano), poi tornare
a `letsencrypt-prod` e cancellare il Secret `notturni-eu-tls` per far
ripartire l'emissione con l'issuer giusto
(`kubectl delete secret notturni-eu-tls -n notturni`).

## Setup

```bash
cp secret.example.yaml secret.yaml
# modificare secret.yaml con valori reali — non committarlo (già in .gitignore)

kubectl apply -k .
```

Per un primo test senza registro (immagini `notturni-backend:latest`/
`notturni-frontend:latest` costruite localmente, vedi nota sotto): buildarle
sulla stessa macchina del nodo K3s (o importarle con `k3s ctr images
import` se costruite altrove), poi in `configmap.yaml` sostituire
`NOCT_CORS_ORIGINS`/`NOCT_OAUTH_REDIRECT_BASE_URL` con l'indirizzo davvero
raggiungibile stasera (es. `http://<ip-nodo>`, vedi i commenti accanto a
quelle variabili) — e ricostruire l'immagine frontend con
`--build-arg NEXT_PUBLIC_API_URL=http://<ip-nodo>` (letto solo in fase di
build, non a runtime — vedi nota più sotto).

## Note

- `backend.yaml` / `frontend.yaml` referenziano immagini locali
  (`notturni-backend:latest`, `notturni-frontend:latest`); vanno sostituite
  con un riferimento a registro una volta disponibile un flusso di
  build/push. Fino ad allora, `imagePullPolicy: IfNotPresent` richiede che
  l'immagine sia già presente sul nodo (buildata lì o importata) — senza,
  il pod resta in `ImagePullBackOff`.
- `postgres.yaml` imposta `PGDATA` su una sottodirectory del volume
  (`/var/lib/postgresql/data/pgdata`) invece della radice del mount: un
  volume Longhorn (ext4) arriva con un `lost+found` creato dal filesystem, e
  `initdb` si rifiuta di inizializzare una data directory non vuota —
  fallirebbe al primo avvio senza questo accorgimento.
- `redis.yaml` e `rabbitmq.yaml` non hanno persistenza in questo primo draft.
- `ingress.yaml` gestisce il routing catch-all path-based sotto l'host fisso
  `notturni.eu`; il routing per sottodominio-per-blog è invece in
  `ingressroute.yaml` (`IngressRoute` Traefik, `HostRegexp` su
  `*.notturni.eu`) — le due risorse convivono, la seconda non sostituisce la
  prima. Entrambe in `https` (`entryPoints: [websecure]`), stesso Secret TLS
  condiviso `notturni-eu-tls` (vedi sezione "Certificati TLS" sopra) —
  condiviso perché cert-manager non può annotare direttamente una CRD
  Traefik `IngressRoute` come farebbe con un `Ingress` standard, da cui la
  scelta di una risorsa `Certificate` esplicita invece dell'annotazione
  `cert-manager.io/cluster-issuer`. Il dominio custom per-utente resta
  invece un lavoro successivo (vedi
  [ROADMAP.md](../ROADMAP.md#3-architettura-stack-e-infrastruttura)).
- `middleware-security-headers.yaml` (Traefik `Middleware`): CSP/HSTS/
  `X-Content-Type-Options: nosniff` minimi, applicato a entrambe le risorse
  sopra (annotazione su `ingress.yaml`, campo `middlewares` su
  `ingressroute.yaml`) — un solo posto da tenere aggiornato.
- MinIO non è esposto pubblicamente da questi manifest (nessuna regola Ingress
  dedicata): `NOCT_S3_PUBLIC_URL` in `configmap.yaml` è un placeholder,
  senza un'esposizione reale gli avatar caricati non saranno raggiungibili
  dall'esterno del cluster.
- Il backend di storage alternativo (`NOCT_STORAGE_BACKEND=localstorage`,
  vedi [ROADMAP.md](../ROADMAP.md#3-architettura-stack-e-infrastruttura)) non
  è cablato in questi manifest: richiederebbe un volume dedicato sul
  Deployment del backend e non è comunque pensato per repliche multiple
  (filesystem non condiviso tra pod).
- `NEXT_PUBLIC_API_URL` (frontend) viene inglobato nel bundle in fase di
  build dell'immagine (`docker build --build-arg NEXT_PUBLIC_API_URL=...`),
  non letto a runtime: non basta un env/ConfigMap sul Deployment, l'immagine
  va ricostruita con l'URL pubblico reale dell'API prima del deploy. Stesso
  discorso per `NEXT_PUBLIC_SITE_URL` (SEO: `metadataBase`/canonical/
  `sitemap.xml`/`robots.txt`, `frontend/src/lib/site.ts`) — va ricostruita con
  il dominio pubblico reale del sito (es. `https://notturni.eu`), non
  `localhost`, altrimenti canonical/sitemap/robots puntano tutti all'host
  sbagliato.
- I worker consumer di coda (`worker-post-backup.yaml`,
  `worker-email-otp.yaml`, `worker-newsletter.yaml`) sono Deployment
  long-running senza `Service` (non ricevono richieste in ingresso, solo
  consumo da RabbitMQ) — stesso `envFrom` di `backend.yaml`, niente
  override locali stile `mailhog`/`localhost:9000` di `compose.yaml`.
  `worker-newsletter` è l'unico dei tre che accede anche al database
  (iscritti/campagne), coperto dallo stesso ConfigMap/Secret condiviso.
- `moderation.yaml` (Deployment+Service, porta 8100): servizio di
  moderazione automatica delle immagini (`moderation/`), `fail open` per
  design (un problema di questo servizio non blocca mai l'upload — vedi
  `app/domain/moderation.py`) ma senza `NOCT_MODERATION_SERVICE_URL`
  valorizzato in `configmap.yaml` (ora presente) nessuna immagine verrebbe
  mai moderata. `readinessProbe`/`livenessProbe` con `initialDelaySeconds`
  più alto del solito: il modello (torch) viene caricato all'avvio.
- `audit-maintenance.yaml` è invece un `CronJob` (non un consumer): archivia
  su storage le settimane ISO chiuse di `audit_log` e cancella gli eventi
  oltre `NOCT_AUDIT_RETENTION_DAYS`. Gira una volta al giorno; in locale
  l'equivalente è il servizio `worker-audit-maintenance` di `compose.yaml`
  (stesso modulo con `--loop`).
- `backup.yaml` è un altro `CronJob`: backup infrastrutturale di Postgres
  (`pg_dump`) e mirror di tutti i bucket MinIO/S3 applicativi verso uno
  storage S3 **esterno** (`app/workers/backup.py`, distinto dal backup
  applicativo dei singoli post di `worker-post-backup`, che resta nello
  stesso bucket applicativo). Senza `NOCT_BACKUP_S3_BUCKET` configurato
  (placeholder in `configmap.yaml`) il job termina subito senza fare nulla —
  va puntato a un provider S3 realmente esterno e separato da quello dei
  contenuti prima della produzione. In locale l'equivalente è il servizio
  `worker-backup` di `compose.yaml` (stesso modulo con `--loop`, verso lo
  stesso MinIO locale su un bucket dedicato solo per avere qualcosa di
  verificabile in sviluppo).
- `smtp-relay.yaml` è invece un Deployment+Service, sempre attivo: un
  servizio SMTP containerizzato (`boky/postfix`) come alternativa a puntare
  `NOCT_SMTP_HOST` direttamente a un provider esterno per l'invio del
  codice MFA via email. `NOCT_SMTP_RELAY_HOST` vuoto (default): il
  container spedisce direttamente (richiede comunque un dominio con SPF/
  DKIM/reverse DNS a posto per una buona deliverability); valorizzato:
  inoltra verso quello smarthost esterno con le credenziali in
  `secret.yaml`. Non collegato di default a `NOCT_SMTP_HOST` — resta una
  scelta esplicita quale dei due usare. In locale l'equivalente è il
  servizio `smtp-relay` di `compose.yaml`.
