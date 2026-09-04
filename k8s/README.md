# Notturni – manifest Kubernetes

Primo draft dei manifest per un singolo nodo K3s (vedi
[ROADMAP.md](../ROADMAP.md#3-architettura-stack-e-infrastruttura)). Richiede:
Longhorn (storage class `longhorn`), Traefik come IngressController, cert-manager
con un `ClusterIssuer` chiamato `letsencrypt-prod`.

## Setup

```bash
cp secret.example.yaml secret.yaml
# modificare secret.yaml con valori reali — non committarlo (già in .gitignore)

kubectl apply -k .
```

## Note

- `backend.yaml` / `frontend.yaml` / `admin.yaml` referenziano immagini locali
  (`notturni-backend:latest`, `notturni-frontend:latest`,
  `notturni-admin:latest`); vanno sostituite con un riferimento a registro una
  volta disponibile un flusso di build/push.
- `admin.yaml` è il pannello di amministrazione: app Next.js separata da
  `frontend/` (vedi `frontend/admin/`), esposta su un host dedicato
  (`admin.notturni.eu`) in `ingress.yaml`, non su un path di `notturni.eu`.
- `redis.yaml` e `rabbitmq.yaml` non hanno persistenza in questo primo draft.
- `ingress.yaml` gestisce un solo host path-based; il routing per sottodominio/blog
  e per dominio custom utente è demandato a un lavoro successivo (vedi
  [ROADMAP.md](../ROADMAP.md#3-architettura-stack-e-infrastruttura)).
- MinIO non è esposto pubblicamente da questi manifest (nessuna regola Ingress
  dedicata): `NOCT_MINIO_PUBLIC_URL` in `configmap.yaml` è un placeholder,
  senza un'esposizione reale gli avatar caricati non saranno raggiungibili
  dall'esterno del cluster.
- `NEXT_PUBLIC_API_URL` (frontend e admin) viene inglobato nel bundle in fase
  di build dell'immagine (`docker build --build-arg NEXT_PUBLIC_API_URL=...`),
  non letto a runtime: non basta un env/ConfigMap sul Deployment, l'immagine
  va ricostruita con l'URL pubblico reale dell'API prima del deploy. Lo
  stesso vale per `NEXT_PUBLIC_ADMIN_URL` (solo frontend): l'URL dell'app
  admin usato dal link "Amministrazione" della dashboard.
- I worker (`app/workers/post_backup_consumer.py`, `email_otp_consumer.py`)
  non hanno ancora un Deployment dedicato in questi manifest — vedi
  `compose.yaml` per l'equivalente locale funzionante; senza il worker di
  backup, i post non vengono replicati su S3 anche se l'accodamento su
  RabbitMQ continua a funzionare (i messaggi restano in coda).
