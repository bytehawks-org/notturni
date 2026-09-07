# Notturni – manifest Kubernetes

Primo draft dei manifest per un singolo nodo K3s (vedi
[ROADMAP.md](../ROADMAP.md#3-architettura-stack-e-infrastruttura)). Richiede:
Longhorn (storage class `longhorn`), Traefik come IngressController (entrambi
già inclusi in una installazione K3s standard, salvo li si sia disattivati
esplicitamente).

`ingress.yaml`, nella versione attuale, serve solo **http** e senza host
fisso — pensato per un primo test senza dominio reale né cert-manager ancora
installati (si accede via IP del nodo). **cert-manager con un
`ClusterIssuer`** (es. `letsencrypt-prod`) **serve solo quando si passa a un
dominio reale in https** — vedi il commento in cima a `ingress.yaml` per
cosa aggiungere a quel punto (annotazione + blocco `tls`), e ricordarsi di
riportare `NOCT_SESSION_COOKIE_SECURE` a `"true"` in `configmap.yaml` nello
stesso momento (i due vanno sempre cambiati insieme).

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
- `ingress.yaml` gestisce un solo host path-based; il routing per sottodominio/blog
  e per dominio custom utente è demandato a un lavoro successivo (vedi
  [ROADMAP.md](../ROADMAP.md#3-architettura-stack-e-infrastruttura)).
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
- I worker consumer di coda (`app/workers/post_backup_consumer.py`,
  `email_otp_consumer.py`) non hanno ancora un Deployment dedicato in questi
  manifest — vedi `compose.yaml` per l'equivalente locale funzionante; senza
  il worker di backup, i post non vengono replicati su S3 anche se
  l'accodamento su RabbitMQ continua a funzionare (i messaggi restano in coda).
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
