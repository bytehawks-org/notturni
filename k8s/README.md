# Notturni – manifest Kubernetes

Primo draft dei manifest per un singolo nodo K3s (vedi
[ROADMAP.md](../ROADMAP.md#3-architettura-stack-e-infrastruttura)). Richiede
una `StorageClass` e Traefik come IngressController (quest'ultimo già incluso
in una installazione K3s standard, salvo lo si sia disattivato
esplicitamente). `postgres.yaml`/`garage.yaml` puntano di default a
`storageClassName: local-path` (il provisioner incluso di serie in K3s,
adatto a un nodo singolo senza componenti aggiuntivi da installare); Longhorn
resta l'obiettivo per un futuro cluster multi-nodo, da riabilitare
sostituendo quel valore quando si passa a quel setup.

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

## Migrazione dei dati da MinIO a Garage

Solo per chi ha già un'istanza con dati su MinIO (l'immagine ufficiale non è
più pubblicata). I nomi dei bucket e le chiavi degli oggetti restano
invariati, quindi i riferimenti nel database (`object_key` degli avatar/media)
non cambiano: si copiano solo gli oggetti. **Non verificata contro un MinIO
reale** — il lato Garage è stato provato; provare prima su una copia.

1. Tenere in esecuzione il Deployment/pod MinIO esistente (con l'immagine già
   presente sul nodo), rinominandolo se serve (`minio-legacy`) per non
   confonderlo col nuovo, e applicare i nuovi manifest (`kubectl apply -k .`)
   con chiavi Garage generate come descritto sotto. Il backend riparte con
   Garage vuoto: bucket e layout li crea il bootstrap.
2. Copiare gli oggetti con `rclone` (da un pod/host che raggiunga entrambi):

   ```bash
   export RCLONE_CONFIG_OLD_TYPE=s3 RCLONE_CONFIG_OLD_PROVIDER=Minio \
          RCLONE_CONFIG_OLD_ENDPOINT=http://minio-legacy:9000 \
          RCLONE_CONFIG_OLD_ACCESS_KEY_ID=<vecchia chiave> RCLONE_CONFIG_OLD_SECRET_ACCESS_KEY=<vecchio secret>
   export RCLONE_CONFIG_NEW_TYPE=s3 RCLONE_CONFIG_NEW_PROVIDER=Other \
          RCLONE_CONFIG_NEW_ENDPOINT=http://garage:3900 RCLONE_CONFIG_NEW_REGION=garage \
          RCLONE_CONFIG_NEW_ACCESS_KEY_ID=<GK...> RCLONE_CONFIG_NEW_SECRET_ACCESS_KEY=<64 hex>
   for b in avatars notturni-content notturni-audit; do rclone copy OLD:$b NEW:$b --progress; done
   rclone check OLD:avatars NEW:avatars   # idem per gli altri bucket
   ```

3. Verificare un avatar e un media esistenti da `https://s3.notturni.eu/...`,
   poi rimuovere MinIO e il suo PVC (`minio-data`).

Nota: i backup infrastrutturali già scritti sotto il prefisso `minio/` del
bucket di backup restano dove sono; i nuovi usano `s3/`.

## Setup

```bash
cp secret.example.yaml secret.yaml
# modificare secret.yaml con valori reali — non committarlo (già in .gitignore)

kubectl apply -k .
```

**Aggiornare il tag delle immagini** (dopo un nuovo push su GHCR): un solo
posto, i tre `newTag` sotto `images:` in fondo a `kustomization.yaml` — non
i singoli manifest. `notturni-backend` copre da solo `backend.yaml` **e** i
tre worker **e** i due `CronJob` (`audit-maintenance.yaml`/`backup.yaml`),
tutti sullo stesso tag: kustomize sostituisce ogni `image:` che referenzia
quel nome esatto, ripetuto identico in più file. Poi `kubectl apply -k .`.

Per un primo test senza registro (immagini costruite localmente sul nodo
invece che pubblicate su GHCR): buildarle sulla stessa macchina del nodo
K3s (o importarle con `k3s ctr images import` se costruite altrove),
aggiornare i tre `newTag`/eventualmente `newName` in `kustomization.yaml`
di conseguenza, poi in `configmap.yaml` sostituire
`NOCT_CORS_ORIGINS`/`NOCT_OAUTH_REDIRECT_BASE_URL` con l'indirizzo davvero
raggiungibile (es. `http://<ip-nodo>`, vedi i commenti accanto a quelle
variabili) — e ricostruire l'immagine frontend con
`--build-arg NEXT_PUBLIC_API_URL=http://<ip-nodo>` (letto solo in fase di
build, non a runtime — vedi nota più sotto).

## Note

- Le immagini pubblicate su GHCR dal flusso di build/push in CI (vedi
  sezione "Setup" sopra per come aggiornarne il tag). `imagePullPolicy:
  IfNotPresent`: se il tag referenziato non è mai stato pubblicato (o non è
  presente localmente sul nodo per un test senza registro), il pod resta in
  `ImagePullBackOff`.
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
- `middleware-security-headers.yaml` (Traefik `Middleware`): HSTS/
  `X-Content-Type-Options: nosniff`/`X-Frame-Options`/`Referrer-Policy`
  minimi, applicato a entrambe le risorse sopra (annotazione su
  `ingress.yaml`, campo `middlewares` su `ingressroute.yaml`) — un solo
  posto da tenere aggiornato. **Non** la Content-Security-Policy: una CSP
  statica uguale per ogni richiesta non può includere un nonce, e
  `script-src 'self'` senza nonce/`unsafe-inline` blocca anche gli script
  inline che Next.js inietta per l'idratazione (non solo script di terze
  parti) — bug scoperto in produzione: pagina servita (200) ma nessun
  form/bottone rispondeva, login e registrazione inclusi. La CSP è ora
  generata per-richiesta con un nonce fresco in `frontend/src/proxy.ts`.
- Lo storage a oggetti è **Garage** (`garage.yaml`, nodo singolo, config in
  `garage.toml`). Un Garage appena avviato non è utilizzabile (nessun layout,
  nessuna chiave): l'initContainer `garage-bootstrap` di `backend.yaml`
  (`legacy/backend/scripts/garage_bootstrap.py`, idempotente) assegna il nodo al
  layout, importa la chiave applicativa e crea i bucket con alias globale.
  La chiave deve avere il formato Garage: `NOCT_S3_ACCESS_KEY_ID` (in
  `configmap.yaml`) = `GK` + 24 hex, `NOCT_S3_SECRET_ACCESS_KEY` (in
  `secret.yaml`) = 64 hex — `echo "GK$(openssl rand -hex 12)"` e
  `openssl rand -hex 32`; idem `NOCT_GARAGE_RPC_SECRET` (64 hex) e
  `NOCT_GARAGE_ADMIN_TOKEN`. I valori committati sono segnaposto.
- Garage **non implementa le bucket policy S3**: avatar e media si servono dal
  suo web endpoint tramite il proxy `s3-public.yaml` (nginx, config in
  `s3-public.conf.template`), che traduce `/{bucket}/{key}` in
  `Host: {bucket}` e inoltra solo GET/HEAD sui prefissi pubblici (bucket
  avatar; `.../userdata/*/*/media/*` del bucket contenuti — backup dei post
  e archivi di audit restano irraggiungibili). `garage.toml` e
  `s3-public.conf.template` sono copie di `garage/garage.toml` e
  `s3-public/default.conf.template` (usati da `compose.yaml`; kustomize non
  legge fuori da questa directory): `legacy/backend/tests/test_deploy_configs.py`
  verifica che non divergano.
- `ingressroute-s3.yaml` (`IngressRoute` Traefik) espone il solo proxy
  `s3-public` sull'host configurato in `NOCT_S3_PUBLIC_URL` (`configmap.yaml`,
  di default `s3.notturni.eu`; mai l'API S3 né l'admin API di Garage) —
  priorità esplicita più alta del match jolly di `ingressroute.yaml`,
  altrimenti quell'host ricadrebbe sul frontend (404, bug osservato in
  produzione: upload riuscito ma media irraggiungibili). Le scritture
  restano dietro le credenziali `NOCT_S3_ACCESS_KEY_ID`/`_SECRET_ACCESS_KEY`,
  solo server-side. `s3` è anche in `RESERVED_SUBDOMAINS`/`RESERVED_BLOG_SLUGS`
  (`frontend/src/proxy.ts`, `legacy/backend/app/domain/blog_rules.py`): un utente
  non può registrare un blog con quello slug.
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
  (`pg_dump`) e mirror di tutti i bucket S3 applicativi verso uno
  storage S3 **esterno** (`app/workers/backup.py`, distinto dal backup
  applicativo dei singoli post di `worker-post-backup`, che resta nello
  stesso bucket applicativo). Senza `NOCT_BACKUP_S3_BUCKET` configurato
  (placeholder in `configmap.yaml`) il job termina subito senza fare nulla —
  va puntato a un provider S3 realmente esterno e separato da quello dei
  contenuti prima della produzione. In locale l'equivalente è il servizio
  `worker-backup` di `compose.yaml` (stesso modulo con `--loop`, verso lo
  stesso Garage locale su un bucket dedicato solo per avere qualcosa di
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
