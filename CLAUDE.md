# Notturni - Agentic Development Blueprint

* Ambiente di sviluppo: Fedora44 utilizzando Podman come sistema di containerizzazione degli applicativi.
* Ambiente di produzione: Kubernetes (probabilmente K3s su non singolo all'inizio da espandere in cluster se il progetto avrà successo).

## 1. Abstract del prodotto

Piattaforma opensource di microblogging e newsletter. Multiutente & multilingua con enfasi sul rispetto delle regolamentazioni GDPR e con un occhio alla visione EU centrica.
Il sistema dovrà prevedere la possibilità di creare fino ad un massimo di 5 blog/pubblicazioni per ogni utente.
Il nome dell'autore che apparirà nel blog e negli articoli potrà essere diverso dal nome dell'utente vero e proprio per garantire privacy e riservatezza.
Esisteranno 4 tipi di utenti: Super Admin, Amministratore, Moderatore e Utente. Il ruolo predefinito sarà "Utente". In fase di creazione delle risorse (Podman o Kubernetes) dovrà essere specificato un username e una password di "Super Admin".
Il sistema dovrà prevedere dei ruoli per gli utenti: Autore (default per Utente), Co-Autore, Revisore e Mediatore. Questi ruoli sono specifici per gli utenti e i blog non per la piattaforma come quelli descritti sopra.
La dashboard utente dovrà essere user-friendly, moderna e responsiva per essere utilizzata sia da PC che da dispositivi mobile.
La dashboard di amministrazione dovrà prevedere tutti le possibili funzionalità che una piattaforma del genere richiede per la parte amministrativa/moderazione.
L'editor dovrà essere pulito e focalizzato sui contenuti, testuali e visuali. Prendere ad esempio il portale fika.bar.
Possibilità di personalizzazione di colori, tipografia e presentazione dei contenuti da parte dell'utente: implementata (tabella `blog_configs`, JSON libero con i vincoli di palette/tipografia della sezione Estetica qui sotto).
Prevedere un sistema di commenti e gestione di questi da parte dell'autore del post/blog. I commenti saranno possibili solo agli utenti registrati (di default). Il proprietario del blog può decidere se renderli aperti anche a chi non registrato ma sarà applicata una moderazione prima della pubblicazione.

Implementato: multilingua per post e pagine statiche (schema a "famiglia di traduzioni", locale ISO 639-1, slug traducibile per lingua — la struttura di path `/it/`, `/en/`, ecc. è una convenzione di routing lasciata al frontend, il backend non la impone); pagine statiche del sito principale (Chi siamo, Contatti, Privacy) gestite da Amministratore/Super Admin; follow tra utenti e verso i blog; profilo utente con bio, avatar (upload su MinIO/S3) e link social; configurazione di aspetto per blog (`blog_configs`); gestione utenti lato Amministrazione (ruolo, attivazione — l'assegnazione di ruoli di amministrazione è riservata al Super Admin). Endpoint in `backend/API.md`.

Frontend: dashboard autore (blog, editor post con traduzioni, moderazione commenti, aspetto del blog, profilo/MFA) e area amministrativa (pagine statiche, utenti), entrambe con guardia di autenticazione/ruolo. Tema chiaro/scuro/automatico (quest'ultimo basato su alba/tramonto della posizione dell'utente, calcolati interamente lato client — la posizione non è mai salvata né inviata al backend) — vedi `frontend/README.md`. Sessione tenuta in `localStorage`: scelta pragmatica da rafforzare (cookie `httpOnly` + CSRF) prima di un uso in produzione.

Gestione contenuti: implementato. Ogni entità nel database usa un UUID come chiave primaria. I post sono salvati in Markdown (nessun rendering lato backend). Workflow di stato completo: `draft` → `pending_review` (ruolo Revisore, finalmente collegato a una capacità reale) → `published`, con `published_at` usato anche per pianificare la pubblicazione nel futuro. Storage S3: media incorporabili nei post (pubblici) e una copia di backup/fallback del Markdown ad ogni salvataggio (privata, via coda RabbitMQ + worker dedicato), entrambi sotto `s3://{bucket}/{site_slug}/userdata/{user_uuid}/{blog_uuid}/...`. Self-hosting: hostname/FQDN dell'istanza e modalità di installazione configurabili via env (`NOCT_INSTANCE_FQDN`, `NOCT_DEPLOYMENT_MODE=solo|platform` — "solo" per un singolo proprietario, con auto-promozione a Super Admin del primo utente e registrazione chiusa dopo). Dettagli in `backend/API.md`.

### Idea di fondo del progetto è la seguente

***Permettere all'utente di esprimersi nella forma più naturale possibile: la parola. Permettere all'utente di esprimersi nella sicurezza e nella ricchezza della propria lingua. Nessuna intelligenza artificiale, solo una tastiera che separa i pensieri da un'interfaccia pulita, amichevole e accogliente sullo schermo***

### Esempi di paragone possono essere presi i seguenti

* medium.com
* fika.bar
* tumblr.com

### Estetica del prodotto

* Pulizia, colori non aggressivi che ispirino calma e amicizia.
* Tipografia elegante e moderna: titoli in serif, testo e link in sans-serif
* Utilizzare una palette di colori di massimo 5 colori
* Utilizzare al massimo 3 font

### Sviluppi futuri da considerare ma non implementare adesso

Prevedere la possibilità di federare la piattaforma con dei suoi cloni e con altre istanze — priorità: prima AT Protocol (Bluesky), poi ActivityPub (Mastodon). Sono due stack di federazione incompatibili tra loro: richiederanno implementazioni separate. Le entità del database hanno già UUID come chiave primaria (implementato) come base ragionevole in vista di questo lavoro futuro, ma non è stato fatto altro in questa direzione.
Prevedere un meccanismo di conteggio dei "like", delle "citazioni" e delle "condivisioni".
Prevedere la possibilità di associare un dominio dell'utente al proprio blog/pubblicazione.

## 2. System Architecture & Stack

Anche se inizialmente sarà un sistema locale e successivamente su un singolo nodo Kubernetes prevedere la clusterizzazione dei componenti necessari.
In questa fase iniziale si predilige Python per la parte di backend. La roadmap dei prossimi 3 anni prevede già la sostituzione dei microservizi con applicativi scritti in Rust.
Come ingress verrà utilizzato Traefik.
Per la parte storage, anche su nodo singolo, si prevede di utilizzare comunque Longhorn.
Prevedere un backup su storage S3 esterno delle risorse fondamentali quali database e MinIO.
I blog/pubblicazioni dell'utente saranno nella forma https://nomeutente.notturni.eu.
All'indirizzo https://notturni.eu ci sarà una sorta di raccolta degli articoli nella lingua dell'utente come avviene sulla piattaforma dev.to. All'indirizzo https://blog.notturni.eu ci sarà il blog vero e proprio della piattaforma su cui comunicare news e aggiornamenti.
Prevedere un blocco al nome dei blog: devono essere di minimo 4 caratteri alfanumerici (può essere utilizzato il simbolo -). I nomi di 3 caratteri o meno sono riservati alla piattaforma (futura funzionalità premium per i sostenitori).
Definire una lista di parole da considerare blacklist per gli utenti in fase di registrazione di un blog (blog, www, mail, journal, api, admin, monitor, stats, status, ecc.).

**Architettura Target:** Locale: x86_64, produzione: ARM64.
**Container Engine (Local):** Podman su Fedora Linux (Rootless networking, no K8s).
**Frontend:** Next.js (App Router, SSR), Tailwind CSS.
**Backend:** Python 3.12+, FastAPI, Uvicorn (ASGI).
**Database:** PostgreSQL 16+ (Estensione `pgcrypto` o `uuid-ossp`).
**Cache & State:** Redis 7+ (Rate limiting, lock distribuiti).
**Message Broker:** RabbitMQ (Code asincrone, AMQP).
**Object Storage:** MinIO (S3-compatible, drop-in replacement per AWS S3 / Cloudflare R2).

In produzione i certificati saranno gestiti tramite cert-manager di Let's Encrypt.
Sarà presente un record dns *.notturni.eu che punterà al server o al bilanciatore dei server di produzione.

## 3. Authentication & SSO Flow

L'identity provider è gestito centralmente da FastAPI, fungendo da Master per lo schema relazionale.

* **Multi-Factor Authentication (MFA):**
  * Supporto TOTP (Authenticator App) generato via `pyotp`.
  * Supporto Email OTP asincrono, inviato accodando il task su RabbitMQ.

* **Single Sign-On (SSO):**
  * Provider attivi: Google, Microsoft, GitHub, LinkedIn.
  * Gestione flow OAuth2/OIDC tramite `Authlib`.

* **Account Linking (Join):**
  * Risoluzione conflitti basata su `email`.
  * Se l'email proveniente dal payload SSO corrisponde a un utente esistente (creato manualmente o tramite altro provider), il sistema richiede la verifica 2FA (se configurata sull'account master) prima di associare il nuovo `provider_id` (es. `github_id=foo`) all'entità nel database.
  * Implementato: login password + sessione JWT/refresh, MFA TOTP ed Email OTP, account linking SSO con gate 2FA. Endpoint ed esempi in `backend/API.md`. Limitazione nota: il flow SSO non è testabile end-to-end senza credenziali OAuth reali per ciascun provider; l'invio email dell'OTP è accodato su RabbitMQ ma non collegato a un provider SMTP reale (consumer placeholder in `app/workers/email_otp_consumer.py`).

* **Token API (accesso diretto, non tramite editor/admin):**
  * Tabella `api_tokens`, valore opaco con prefisso `noct_`, solo l'hash sha256 è persistito.
  * `owner_type` distingue token del motore core (nessun utente associato, uso machine-to-machine) da token utente (previsti per il futuro: permettere di interfacciarsi con l'API senza passare da editor o admin del proprio blog).
  * Dettagli ed endpoint in `backend/API.md`.

## 4. Agentic Prompting Instructions

Linee guida operative per l'agente AI incaricato della generazione del codice:
**Architettura:** I `Dockerfile` o i manifest devono agnostici all'architettura utilizzata.
**S3 Compatibility:** Le interazioni con lo storage (es. tramite `boto3` o S3 SDK) devono iniettare sempre endpoint custom (`ENDPOINT_URL=http://localhost:9000`) per garantire la compatibilità trasparente tra MinIO locale e AWS/Cloudflare in produzione.
**Formato Output:** Evitare commenti discorsivi, intro o conclusioni riepilogative. Fornire snippet/comandi essenziali per Soluzioni Rapide, e file completi con costrutti/referenze per Soluzioni Estese.
**Prefisso/Namespace:** Per variabili ed environment di ambiente Podman/Kubernetes utilizzare il prefisso NOCT_/noct_ (utilizzare il case in base al caso di utilizzo).

## 5. Local Environment Setup (Podman)

Inizializzazione rapida dei layer infrastrutturali su Fedora 44 tramite Pod condiviso.

```bash
# Creazione Pod per port forwarding e networking localhost
podman pod create --name notturni-pod -p 8000:8000 -p 3000:3000 -p 5432:5432 -p 6379:6379 -p 5672:5672 -p 15672:15672 -p 9000:9000 -p 9001:9001

# 1. PostgreSQL
podman run -d --pod notturni-pod --name postgres-db -e POSTGRES_USER=admin -e POSTGRES_PASSWORD=foo -e POSTGRES_DB=notturni arm64v8/postgres:16-alpine

# 2. Redis
podman run -d --pod notturni-pod --name redis-cache arm64v8/redis:7-alpine

# 3. RabbitMQ
podman run -d --pod notturni-pod --name rabbit-mq arm64v8/rabbitmq:3-management-alpine

# 4. MinIO
podman run -d --pod notturni-pod --name minio-storage -e MINIO_ROOT_USER=admin -e MINIO_ROOT_PASSWORD=foobarfoobar minio/minio server /data --console-address ":9001"