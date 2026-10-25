# Configurazione: variabili d'ambiente

Riferimento di tutte le variabili `NOCT_*` (più le due `NEXT_PUBLIC_*` del frontend). Il modello è `.env.example`; con
`podman compose up` molti valori sono sovrascritti da `compose.yaml` con i nomi dei servizi interni, in Kubernetes vivono
in `k8s/configmap.yaml` (valori non sensibili) e `k8s/secret.yaml` (segreti).

- La colonna **Ambito** indica chi legge la variabile: `core` (piattaforma, `crates/noct-core/src/config.rs`) o il modulo che la
  dichiara nella sezione `[config]` del suo `module.toml`. Un nome appartiene a un solo modulo.
- **Segreto**: non committare, non loggare; in Kubernetes va nel `Secret`. `scripts/gen-env.sh` genera i valori casuali.
- Un valore vuoto equivale a variabile assente.
- `cargo run -p noct-ctl -- modules` elenca i moduli compilati; `noct-ctl validate` verifica manifest e schema.
- Il backend rifiuta all'avvio i segnaposto di `.env.example` (`NOCT_JWT_SECRET`, password del Super Admin).
- Il modulo di prova `sandbox` (`NOCT_SANDBOX_*`) esiste solo nei test e non è documentato qui.


## Identità dell'istanza

| Variabile | Ambito | Default | Obbligatoria | Descrizione | Esempio |
|---|---|---|---|---|---|
| `NOCT_INSTANCE_FQDN` | core | `notturni.eu` | no | Dominio pubblico: base dei link nelle email e degli identificatori di federazione, dominio email del sigillo di verifica. | `notturni.example.com` |
| `NOCT_SITE_SLUG` | core | `notturni` | no | Namespace dei path su S3: evita collisioni tra più installazioni sullo stesso bucket. Solo minuscole, cifre, `-`. | `staging` |
| `NOCT_DEPLOYMENT_MODE` | auth | `platform` | no | `platform` = multiutente; `solo` = un solo proprietario, il primo utente diventa Super Admin e la registrazione si chiude. | `solo` |
| `NOCT_REGISTRATION_MODE` | auth | `open` | no | Valore **iniziale** (prima installazione) di `platform_config`: `open` · `invite` · `closed`. Poi si cambia da `/admin/config`. | `invite` |
| `NOCT_DEFAULT_LOCALE` | auth | `it` | no | Lingua iniziale dell'interfaccia (`it` · `en`); poi da `/admin/config`. | `en` |
| `NOCT_DEFAULT_INTERESTS` | interests | elenco di 16 voci | no | JSON con l'elenco iniziale degli interessi; vuoto = predefinito. | `[{"key":"music","translations":{"it":"Musica","en":"Music"}}]` |
| `NOCT_EXPERIMENTAL_FEATURES` | core | vuota (tutte spente) | no | **Sperimentale, opt-in.** Elenco (virgola) di `wiki` e/o `docs`: abilita la creazione di Spazi di quel tipo e la relativa modalità di visualizzazione. Spente, la piattaforma offre solo blog (l'API risponde `400` e l'interfaccia non propone i tipi). Spegnerle dopo non cancella né nasconde gli Spazi già creati. Un nome sconosciuto impedisce l'avvio. In Kubernetes vive nel ConfigMap `notturni-config`; serve il riavvio di backend e worker. | `wiki,docs` |
| `NOCT_MODULES` | core | moduli di default | no | Moduli opzionali attivi alla **prima** installazione (virgola); poi vale lo stato nel database, modificabile da `/admin/modules`. | `wiki,git_sync,newsletter` |
| `NOCT_BIND_ADDR` | core | `0.0.0.0:8000` | no | Indirizzo di ascolto dell'API. | `127.0.0.1:8000` |
| `NOCT_TRUSTED_PROXIES` | core | `1` | no | Quanti proxy fidati stanno davanti all'API (IP reale in rate limit e audit). | `2` |
| `NOCT_APP_ENV` | k8s | — | no | Etichetta d'ambiente presente nel ConfigMap (`production`); non risulta letta dal codice. | `production` |

## Primo Super Admin

| Variabile | Ambito | Default | Obbligatoria | Descrizione | Esempio |
|---|---|---|---|---|---|
| `NOCT_SUPER_ADMIN_USERNAME` | auth | — | no | Creato all'avvio se non esiste (idempotente). Se una delle tre variabili manca, la creazione è disattivata. | `superadmin` |
| `NOCT_SUPER_ADMIN_EMAIL` | auth | — | no | Email del Super Admin (il login avviene per email). | `admin@example.com` |
| `NOCT_SUPER_ADMIN_PASSWORD` | auth | — | no | **Segreto.** Il backend rifiuta il segnaposto di `.env.example`. | `<password lunga e casuale>` |

## Segreti e sessioni

| Variabile | Ambito | Default | Obbligatoria | Descrizione | Esempio |
|---|---|---|---|---|---|
| `NOCT_JWT_SECRET` | core | — | **sì** | **Segreto.** Firma di JWT, OTP derivati e link firmati: almeno 32 byte casuali. Il backend rifiuta valori corti o il segnaposto. | `openssl rand -base64 48` |
| `NOCT_JWT_ACCESS_TOKEN_TTL_MINUTES` | core | `15` | no | Durata dell'access token. | `15` |
| `NOCT_JWT_REFRESH_TOKEN_TTL_DAYS` | auth | `30` | no | Durata del refresh token (cookie di sessione). | `30` |
| `NOCT_SECRETS_KEY` | auth_mfa | derivata da `NOCT_JWT_SECRET` | consigliata | **Segreto.** Chiave dedicata per cifrare a riposo i segreti TOTP **e le credenziali della sincronizzazione Git (token HTTPS e chiavi SSH di deploy)**. Se vuota si deriva dal JWT: ruotare il JWT renderebbe illeggibili i valori già salvati. In produzione impostarla. | `openssl rand -base64 32` |
| `NOCT_MFA_DISABLE_REQUIRES_CODE` | auth_mfa | `true` | no | Per disattivare l'MFA serve un codice valido. `false` = comportamento della v1. | `true` |
| `NOCT_SESSION_COOKIE_SECURE` | auth | `true` | no | Attributo `Secure` del cookie di refresh. `false` solo in sviluppo su http. | `false` |
| `NOCT_SESSION_COOKIE_SAMESITE` | auth | `lax` | no | Attributo `SameSite` del cookie. | `lax` |
| `NOCT_SESSION_COOKIE_DOMAIN` | auth | — | no | Solo se frontend e backend condividono un dominio genitore. | `.notturni.example.com` |

## Browser: CORS e frontend

| Variabile | Ambito | Default | Obbligatoria | Descrizione | Esempio |
|---|---|---|---|---|---|
| `NOCT_CORS_ORIGINS` | core | `http://localhost:3000` | no | Origini ammesse (virgola) per le chiamate dal browser; deve includere ogni origine da cui il frontend è servito. Un'origine assente non riceve né invia il cookie. | `https://notturni.example.com` |
| `NOCT_CORS_ORIGIN_REGEX` | core | — | no | Wildcard aggiuntiva per i sottodomini per-blog; il match è sull'intera origine. | `https://([a-z0-9-]+\.)?notturni\.example\.com` |
| `NOCT_FRONTEND_REVALIDATE_URL` | core | — | no | Invalidazione on-demand della cache SSR dopo una modifica a contenuti pubblici. Vuoto = solo cache a tempo. | `http://frontend:3000/api/revalidate` |
| `NOCT_REVALIDATE_SECRET` | core + frontend | — | no | **Segreto.** Deve coincidere tra backend e frontend. | `openssl rand -base64 32` |
| `NOCT_BACKEND_INTERNAL_URL` | frontend | `NEXT_PUBLIC_API_URL`, poi `http://localhost:8000` | consigliata | URL interno del backend usato dal rendering lato server (evita di uscire dalla rete del cluster). | `http://backend:8000` |
| `NEXT_PUBLIC_API_URL` | frontend | `http://localhost:8000` | sì in produzione (build) | URL pubblico dell'API usato dal browser (incorporato nel build). | `https://api.example.com` |
| `NEXT_PUBLIC_SITE_URL` | frontend | `http://localhost:3000` | sì in produzione (build) | URL pubblico del sito (link canonici, sitemap). | `https://notturni.example.com` |

## Database

| Variabile | Ambito | Default | Obbligatoria | Descrizione | Esempio |
|---|---|---|---|---|---|
| `NOCT_DATABASE_URL` | core | — | sì (o le `NOCT_POSTGRES_*`) | URL unico; ha la precedenza sulle variabili separate. | `postgres://utente:password@postgres:5432/notturni` |
| `NOCT_POSTGRES_USER` | core | — | sì (se manca l'URL) | Utente. | `admin` |
| `NOCT_POSTGRES_PASSWORD` | core | vuota | no | **Segreto.** | `<password>` |
| `NOCT_POSTGRES_DB` | core | — | sì (se manca l'URL) | Nome del database. | `notturni` |
| `NOCT_POSTGRES_HOST` | core | `localhost` | no | Host. | `postgres` |
| `NOCT_POSTGRES_PORT` | core | `5432` | no | Porta. | `5432` |
| `NOCT_AUTO_MIGRATE` | api / worker | `true` | no | Applica le migrazioni dei moduli attivi all'avvio (advisory lock, sicuro con più repliche). `false` = le applica altro (`noct-ctl migrate apply`); sul worker è `false` nei manifest. | `false` |

## Bus di messaggi e cache

| Variabile | Ambito | Default | Obbligatoria | Descrizione | Esempio |
|---|---|---|---|---|---|
| `NOCT_NATS_URL` | core | — (bus in memoria) | no | NATS JetStream per eventi e job. Vuoto = in memoria (solo sviluppo, un processo). Con NATS i consumer girano in `noct-worker`. | `nats://nats:4222` |
| `NOCT_NATS_PREFIX` | core | `noct` | no | Prefisso di stream e subject. | `noct` |
| `NOCT_WORKER_INLINE` | core | `true` senza NATS, altrimenti `false` | no | Esegue i consumer dentro il processo API. | `false` |
| `NOCT_REDIS_URL` | core | — | no | URL unico di Redis (rate limit, cache, lock). Ha la precedenza su host/porta. | `redis://redis:6379/0` |
| `NOCT_REDIS_HOST` | core | — (cache in memoria) | no | Host Redis; vuoto = in memoria. | `redis` |
| `NOCT_REDIS_PORT` | core | `6379` | no | Porta. | `6379` |
| `NOCT_REDIS_DB` | core | `0` | no | Numero del database. | `0` |

## Storage a oggetti

| Variabile | Ambito | Default | Obbligatoria | Descrizione | Esempio |
|---|---|---|---|---|---|
| `NOCT_STORAGE_BACKEND` | core | `localstorage` | no | `s3` (Garage, AWS S3, R2…) oppure `localstorage` (filesystem, servito dal backend su `/storage`, per installazioni `solo`). | `s3` |
| `NOCT_S3_ENDPOINT_URL` | core | — | con `s3` | Endpoint dell'API S3, mai fisso nel codice. | `http://garage:3900` |
| `NOCT_S3_PUBLIC_URL` | core | — | con `s3` | URL pubblico degli oggetti (avatar, media), servito dal proxy `s3-public`. Su AWS S3 vero non coincide con l'endpoint. | `https://media.example.com` |
| `NOCT_S3_REGION` | core | — | no | Garage valida la regione: deve coincidere con `s3_region` di `garage.toml`. | `garage` |
| `NOCT_S3_ACCESS_KEY_ID` | core | — (ruolo AWS) | no | Con Garage: `GK` + 24 hex. Entrambe vuote = default credential chain dell'SDK. | `GK0123456789abcdef01234567` |
| `NOCT_S3_SECRET_ACCESS_KEY` | core | — | no | **Segreto.** 64 hex con Garage. | `openssl rand -hex 32` |
| `NOCT_S3_BUCKET_AVATARS` | core | `avatars` | no | Bucket degli avatar (pubblico). | `avatars` |
| `NOCT_S3_BUCKET_CONTENT` | core | `notturni-content` | no | Media incorporati nei post e copia del Markdown dei post. | `notturni-content` |
| `NOCT_S3_BUCKET_AUDIT` | core | `notturni-audit` | no | Archivi del registro di audit (privato). | `notturni-audit` |
| `NOCT_S3_PUBLIC_ACCESS` | compose | `website` | no | Modalità di esposizione pubblica dei bucket di Garage (solo `compose.yaml`). | `website` |
| `NOCT_LOCAL_STORAGE_BASE_PATH` | core | `./data/storage` | no | Directory dello storage locale (nell'immagine: `/data/storage`). | `/data/storage` |
| `NOCT_LOCAL_STORAGE_PUBLIC_URL` | core | `http://localhost:8000/storage` | no | URL pubblico dello storage locale. | `https://api.example.com/storage` |
| `NOCT_GARAGE_RPC_SECRET` | garage | — | sì (con Garage) | **Segreto.** 64 hex, RPC del daemon. | `openssl rand -hex 32` |
| `NOCT_GARAGE_ADMIN_TOKEN` | garage | — | sì (con Garage) | **Segreto.** Token dell'admin API (porta 3903, non pubblicata). | `openssl rand -base64 32` |
| `NOCT_GARAGE_ADMIN_URL` | `noct-ctl` | `http://garage:3903` | no | Endpoint admin usato da `garage-bootstrap`. | `http://garage:3903` |
| `NOCT_GARAGE_NODE_CAPACITY_BYTES` | `noct-ctl` | `1073741824` | no | Capacità assegnata al nodo Garage al bootstrap. | `10737418240` |
| `NOCT_GARAGE_WEB_UPSTREAM` | s3-public | `garage:3902` | no | Upstream web di Garage per il proxy `s3-public`. | `garage:3902` |

## Backup e audit

| Variabile | Ambito | Default | Obbligatoria | Descrizione | Esempio |
|---|---|---|---|---|---|
| `NOCT_BACKUP_S3_BUCKET` | backup | — (backup disattivato) | no | Bucket dello storage **esterno**. Senza, resta solo la copia Markdown dei post. | `notturni-backup` |
| `NOCT_BACKUP_S3_ENDPOINT_URL` | backup | — | no | Endpoint S3 esterno. | `https://s3.example.com` |
| `NOCT_BACKUP_S3_REGION` | backup | — | no | Regione. | `eu-central-1` |
| `NOCT_BACKUP_S3_ACCESS_KEY_ID` | backup | — | no | Access key. | `<key id>` |
| `NOCT_BACKUP_S3_SECRET_ACCESS_KEY` | backup | — | no | **Segreto.** | `<secret>` |
| `NOCT_AUDIT_ARCHIVE_ENABLED` | audit | `true` | no | Archivia su storage le settimane ISO concluse del registro prima di cancellarle (la conservazione in giorni è in `/admin/config`). | `true` |

## Email, moderazione, captcha

| Variabile | Ambito | Default | Obbligatoria | Descrizione | Esempio |
|---|---|---|---|---|---|
| `NOCT_SMTP_HOST` | core | — (email non inviate) | no | Server SMTP per MFA, reset password, newsletter. Vuoto = le email non partono. | `smtp.example.com` |
| `NOCT_SMTP_PORT` | core | `587` | no | Porta SMTP. | `587` |
| `NOCT_SMTP_USER` | core | — | no | Utente SMTP. | `no-reply@example.com` |
| `NOCT_SMTP_PASSWORD` | core | — | no | **Segreto.** | `<password>` |
| `NOCT_SMTP_USE_TLS` | core | `true` | no | TLS verso l'SMTP. | `true` |
| `NOCT_SMTP_FROM_EMAIL` | core | `no-reply@notturni.eu` | no | Mittente. | `no-reply@example.com` |
| `NOCT_SMTP_RELAY_HOST` | smtp-relay | — (invio diretto) | no | Smarthost del servizio `smtp-relay` containerizzato; vuoto = invio diretto (servono SPF/DKIM/rDNS). | `smtp.example.com` |
| `NOCT_SMTP_RELAY_USERNAME` | smtp-relay | — | no | Credenziali dello smarthost. | `relay-user` |
| `NOCT_SMTP_RELAY_PASSWORD` | smtp-relay | — | no | **Segreto.** | `<password>` |
| `NOCT_MODERATION_SERVICE_URL` | core | — (nessuna classificazione) | no | Servizio di moderazione immagini. | `http://moderation:8100` |
| `NOCT_TURNSTILE_SITE_KEY` | core | — | no | Site key pubblica di Cloudflare Turnstile (letta dal frontend da `GET /api/v1/config`). | `0x4AAAA…` |
| `NOCT_TURNSTILE_SECRET_KEY` | comments | — | no | **Segreto.** Senza chiavi, la modalità «commenti aperti a tutti» non è selezionabile. | `0x4AAAA…` |
| `NOCT_TURNSTILE_VERIFY_URL` | comments | endpoint Cloudflare | no | Cambia solo nei test. | — |

## Accesso con provider esterni (auth_sso)

| Variabile | Ambito | Default | Obbligatoria | Descrizione | Esempio |
|---|---|---|---|---|---|
| `NOCT_OAUTH_REDIRECT_BASE_URL` | auth_sso | `http://localhost:8000` | no | Origine pubblica del backend, base del `redirect_uri` registrato presso i provider. | `https://api.example.com` |
| `NOCT_FRONTEND_URL` | auth_sso | `http://localhost:3000` | no | Origine del frontend, dove l'utente torna dopo il login. | `https://notturni.example.com` |
| `NOCT_OAUTH_GOOGLE_CLIENT_ID / _CLIENT_SECRET` | auth_sso | vuote | no | Abilita Google se valorizzate (secret: **segreto**). | `<client id>` |
| `NOCT_OAUTH_MICROSOFT_CLIENT_ID / _CLIENT_SECRET` | auth_sso | vuote | no | Abilita Microsoft. | `<client id>` |
| `NOCT_OAUTH_GITHUB_CLIENT_ID / _CLIENT_SECRET` | auth_sso | vuote | no | Abilita GitHub. | `<client id>` |
| `NOCT_OAUTH_LINKEDIN_CLIENT_ID / _CLIENT_SECRET` | auth_sso | vuote | no | Abilita LinkedIn. | `<client id>` |
| `NOCT_OAUTH_ENDPOINT_BASE` | auth_sso | vuota | no | Solo prove e test: gli endpoint di ogni provider diventano `<base>/<provider>/…`. | — |

## Altri moduli

| Variabile | Ambito | Default | Obbligatoria | Descrizione | Esempio |
|---|---|---|---|---|---|
| `NOCT_DNS_SERVER` | custom_domains | — (resolver di sistema) | no | Server DNS per la verifica TXT dei domini personali. | `1.1.1.1:53` |
| `NOCT_DNS_TIMEOUT_MS` | custom_domains | `5000` | no | Timeout della verifica DNS. | `5000` |
| `NOCT_LINK_PREVIEW_ALLOW_PRIVATE` | link_preview | `false` | no | Consente anteprime verso reti private. **Disattiva la difesa SSRF**: solo sviluppo. | `false` |
| `NOCT_GIT_SYNC_ALLOW_PRIVATE` | git_sync | `false` | no | Consente repository su host privati (HTTPS e SSH) e `file://`. **Disattiva la difesa SSRF**: solo sviluppo e test. | `false` |
| `NOCT_GIT_SYNC_GIT_BIN` | git_sync | `git` | no | Eseguibile `git` (presente nell'immagine). | `/usr/bin/git` |
| `NOCT_GIT_SYNC_SSH_BIN` | git_sync | `ssh` | no | Client SSH usato da `git` (pacchetto `openssh-client`, presente nell'immagine). Cambia solo per percorsi non standard e nei test. | `/usr/bin/ssh` |
| `NOCT_GIT_SYNC_KEYSCAN_BIN` | git_sync | `ssh-keyscan` | no | Scanner che rileva la host key SSH alla configurazione, per fissarla. Cambia solo per percorsi non standard e nei test. | `/usr/bin/ssh-keyscan` |

## Configurazione a caldo (non è ambiente)

Alcuni valori nascono dall'ambiente solo alla prima installazione e poi vivono nel database, modificabili senza riavvio:
modalità di registrazione, lingua predefinita, conservazione dell'audit (`/admin/config`) e moduli attivi (`/admin/modules`,
solo Super Admin). Cambiare la variabile dopo la prima installazione **non** ha effetto su questi.

## Esempi minimi

**Sviluppo locale, tutto in memoria** (nessun servizio esterno):

```dotenv
NOCT_POSTGRES_USER=admin
NOCT_POSTGRES_PASSWORD=foo
NOCT_POSTGRES_DB=notturni
NOCT_JWT_SECRET=<openssl rand -base64 48>
NOCT_STORAGE_BACKEND=localstorage
NOCT_SESSION_COOKIE_SECURE=false
```

**Produzione (K3s)**: oltre ai segreti, impostare almeno `NOCT_INSTANCE_FQDN`, `NOCT_CORS_ORIGINS`/`NOCT_CORS_ORIGIN_REGEX`,
`NOCT_SECRETS_KEY`, `NOCT_NATS_URL`, `NOCT_REDIS_URL`, le variabili `NOCT_S3_*`, SMTP e `NOCT_BACKUP_S3_*` verso un provider
realmente esterno.

## Sicurezza e GDPR/NIS2

- `NOCT_SECRETS_KEY`: custodirla fuori dal cluster (è la chiave dei segreti TOTP e delle credenziali Git: token e chiavi SSH); perderla rende illeggibili
  quei valori.
- `NOCT_EXPERIMENTAL_FEATURES`: wiki e docs non hanno ancora la maturità del resto della piattaforma; attivarle è una scelta consapevole di chi installa.
- `NOCT_LINK_PREVIEW_ALLOW_PRIVATE` e `NOCT_GIT_SYNC_ALLOW_PRIVATE` sono interruttori di sviluppo: in produzione devono restare `false`.
- `NOCT_BACKUP_S3_*`: il backup contiene dati personali; il provider esterno va nel registro dei trattamenti (preferibilmente UE).
- Il refresh token viaggia in un cookie: in https lasciare `NOCT_SESSION_COOKIE_SECURE=true`.
