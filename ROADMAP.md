# Specifiche e stato di avanzamento

Questo documento è il riferimento pratico per capire **cosa prevede il progetto**,
**cosa è già stato implementato** e **cosa resta da fare**. Fa anche da workplan:
le righe segnate 🟡 e ⚪ sono, di fatto, i task aperti.

Il documento "sorgente" delle intenzioni di prodotto (visione, tono, esempi di
paragone, vincoli architetturali di partenza) resta ad uso interno del team/degli
agenti che generano il codice e non è linkato dalla documentazione pubblica:
questo file ne è la trascrizione verificabile, specifica per specifica, contro
lo stato reale del codice.

Legenda stato:

- ✅ **Implementato** — presente nel codice e coperto da test e/o verifica manuale.
- 🟡 **Parziale** — presente ma con limitazioni note, o infrastruttura pronta e non
  ancora collegata a logica applicativa.
- ⚪ **Da fare** — non ancora iniziato.

## 1. Prodotto e regole di dominio

| Specifica | Stato | Note / task residuo |
|---|---|---|
| Multiutente & multilingua, enfasi GDPR/EU | 🟡 | Multilingua implementato (vedi sotto). Nessuna funzionalità GDPR dedicata ancora (export/cancellazione dati account, registro consensi, informativa privacy strutturata oltre alla pagina statica libera). |
| Massimo 5 blog per utente | ✅ | `MAX_BLOGS_PER_USER` in `backend/app/domain/blog_rules.py`. |
| Nome autore in pubblico diverso dal nome utente reale | ✅ | Campo display name separato su blog/profilo. |
| 4 ruoli piattaforma: Super Admin, Amministratore, Moderatore, Utente (default Utente) | 🟡 | Enum `PlatformRole` completo (`backend/app/models/user.py`). Super Admin/Amministratore gestiscono utenti e pagine statiche. **Il ruolo piattaforma Moderatore è definito ma non ancora collegato a nessuna capacità reale** (nessun endpoint/permesso lo usa). |
| Username e password di Super Admin specificati alla creazione delle risorse (Podman/K8s) | ⚪ | Non implementato: oggi il Super Admin nasce o per auto-promozione del primo utente (`NOCT_DEPLOYMENT_MODE=solo`) o per `UPDATE` manuale a DB in modalità `platform`. Manca un vero bootstrap via env/secret a livello di deploy. |
| 4 ruoli blog: Autore (default), Co-Autore, Revisore, Mediatore | ✅ | Enum `BlogRole` (`backend/app/models/blog.py`), tutti e quattro collegati a capacità reali (`backend/app/domain/authorization.py`): scrittura, revisione (`pending_review`→`published`), moderazione commenti. |
| Dashboard utente user-friendly, moderna, responsiva | 🟡 | Prima versione costruita (Next.js + Tailwind, dashboard blog/post/profilo). Da raffinare con uso reale; nessun audit di accessibilità/responsività fatto oltre al testing manuale. |
| Dashboard di amministrazione con tutte le funzionalità di moderazione | 🟡 | Gestione utenti e pagine statiche implementate. **Manca un pannello di moderazione trasversale** (commenti su tutti i blog, segnalazioni, sospensione contenuti) — oggi la moderazione commenti è solo per-blog, ad opera del Mediatore di quel blog. |
| Editor pulito, focalizzato sui contenuti (rif. fika.bar) | ✅ | Editor Markdown con traduzioni, senza rendering lato backend. |
| Personalizzazione colori/tipografia/presentazione per blog | ✅ | Tabella `blog_configs`, JSON libero con i vincoli della sezione [2](#2-estetica). |
| Sistema di commenti con moderazione dell'autore | ✅ | Aperti di default solo a utenti registrati; il proprietario del blog può aprirli anche a non registrati con moderazione pre-pubblicazione obbligatoria. |
| Multilingua per post e pagine statiche | ✅ | Schema "famiglia di traduzioni" (`translation_group_id` + `locale` + slug per lingua). Il routing `/it/`, `/en/`, ... è lasciato al frontend: **il frontend pubblico multilingua/i18n-routing non è ancora costruito**, oggi si seleziona la lingua via parametro. |
| Pagine statiche del sito principale (Chi siamo, Contatti, Privacy) | ✅ | CRUD gestito da Amministratore/Super Admin. |
| Follow tra utenti e verso i blog | ✅ | |
| Profilo utente: bio, avatar, link social | ✅ | Avatar su MinIO/S3. |
| Gestione contenuti in Markdown, nessun rendering lato backend | ✅ | |
| UUID come chiave primaria per ogni entità | ✅ | |
| Media su S3 in `.../userdata/{user_uuid}/{blog_uuid}/media/...` | ✅ | Bucket con policy pubblica scoped al solo prefisso `media/*`. |
| Backup/fallback Markdown dei post su S3 ad ogni salvataggio | ✅ | Privato, via coda RabbitMQ + worker dedicato (`worker-post-backup`). |
| Stato post: draft → pending_review → published, con `published_at` per pianificazione | ✅ | Ruolo Revisore collegato alla transizione `pending_review`→`published`. |
| Hostname/FQDN e modalità di installazione via env | ✅ | `NOCT_INSTANCE_FQDN`, `NOCT_DEPLOYMENT_MODE=solo\|platform`. |

## 2. Estetica

| Specifica | Stato | Note / task residuo |
|---|---|---|
| Palette di massimo 5 colori | ✅ | Validato server-side in `backend/app/domain/blog_config.py`. |
| Massimo 3 font | ✅ | Idem. |
| Titoli in serif, testo/link in sans-serif | 🟡 | Applicato nel tema di default del frontend; non ancora imposto come vincolo quando l'utente personalizza la tipografia del proprio blog. |
| Toni calmi, non aggressivi | 🟡 | Scelta soggettiva applicata al tema di default; nessun vincolo automatico sulla palette scelta dall'utente oltre al numero massimo di colori. |

## 3. Architettura, stack e infrastruttura

| Specifica | Stato | Note / task residuo |
|---|---|---|
| Stack di base: Next.js, FastAPI, PostgreSQL 16+, Redis, RabbitMQ, MinIO | ✅ | Tutti i servizi presenti in `compose.yaml` e nei manifest `k8s/`. |
| Redis per rate limiting e lock distribuiti | 🟡 | Servizio deployato ma **non ancora usato da nessuna logica applicativa** — nessun rate limiting né lock distribuito implementato oggi. |
| Dockerfile/manifest agnostici rispetto all'architettura (x86_64/ARM64) | ✅ | Immagini multi-arch, nessuna dipendenza hard-coded da un'architettura. |
| Compatibilità S3 con endpoint custom iniettato (MinIO/AWS/Cloudflare) | ✅ | `backend/app/core/storage.py`, sempre via `ENDPOINT_URL` configurabile. |
| Prefisso env `NOCT_`/`noct_` | ✅ | |
| Clusterizzazione dei componenti in produzione | ⚪ | Manifest K8s attuali sono un primo draft a singolo nodo; nessun lavoro di clustering/HA fatto. |
| Traefik come ingress | 🟡 | `k8s/ingress.yaml` usa `ingressClassName: traefik`, ma è **routing path-based su un solo host** — manca l'`IngressRoute` dinamico per sottodominio-per-utente. |
| Longhorn come storage class | ✅ | Usato in `k8s/postgres.yaml` e `k8s/minio.yaml` (draft). |
| Backup su S3 esterno di database e MinIO | ⚪ | Non implementato: esiste solo il backup applicativo dei singoli post (`worker-post-backup`), non un backup infrastrutturale di Postgres/MinIO nel loro complesso. |
| Blog utente su `https://nomeutente.notturni.eu` | ⚪ | Non implementato: oggi i blog sono referenziati per slug via path/API, non c'è routing per sottodominio né nel backend né nel frontend né nei manifest K8s (annotato come lavoro futuro in `k8s/ingress.yaml`). |
| `notturni.eu`: raccolta articoli nella lingua dell'utente (stile dev.to) | ⚪ | Non implementato. |
| `blog.notturni.eu`: blog ufficiale della piattaforma | ⚪ | Non implementato (nessuna distinzione oggi tra blog "di piattaforma" e blog utente). |
| Blocco nome blog: minimo 4 caratteri alfanumerici (`-` ammesso) | ✅ | `backend/app/domain/blog_rules.py`. |
| Nomi di blog di 3 caratteri o meno riservati alla piattaforma | ✅ | Idem — bloccati oggi, nessuna funzionalità premium di sblocco ancora costruita. |
| Blacklist nomi riservati (blog, www, mail, admin, ...) | ✅ | `backend/app/domain/blog_rules.py`, più blacklist separata per gli username (`backend/app/domain/usernames.py`). |
| cert-manager / Let's Encrypt in produzione | 🟡 | Annotazione `cert-manager.io/cluster-issuer` presente in `k8s/ingress.yaml`, presuppone un `ClusterIssuer` già installato nel cluster: non fa parte dei manifest del progetto. |
| DNS wildcard `*.notturni.eu` | ⚪ | Prerequisito infrastrutturale esterno al repository, da configurare quando si affronta il routing per sottodominio. |
| Migrazione progressiva dei microservizi a Rust (orizzonte 3 anni) | ⚪ | Non iniziata, coerente con la roadmap di lungo periodo. |

## 4. Autenticazione e SSO

| Specifica | Stato | Note / task residuo |
|---|---|---|
| Login password + sessione JWT (access + refresh con rotation) | ✅ | |
| MFA TOTP (app authenticator) | ✅ | Via `pyotp`. |
| MFA Email OTP asincrono | 🟡 | Accodato su RabbitMQ e funzionante end-to-end lato coda; il consumer (`app/workers/email_otp_consumer.py`) è ancora un placeholder che **logga il codice invece di inviarlo** — nessun provider SMTP/transazionale collegato. |
| SSO Google/Microsoft/GitHub/LinkedIn (OAuth2/OIDC via Authlib) | 🟡 | Implementato a livello di codice; **non testabile end-to-end** senza credenziali OAuth reali per ciascun provider — verificato solo a livello di logica di dominio. |
| Account linking basato su email, con gate 2FA | ✅ | |
| Token API (`api_tokens`, prefisso `noct_`, hash sha256) | 🟡 | Tabella e logica di dominio pronte per token "core" (motore, machine-to-machine) e token "utente", ma **manca ancora l'interfaccia utente** per generare/revocare un proprio token: oggi è raggiungibile solo via API diretta o script di bootstrap. |
| GDPR: MFA/2FA come rafforzamento sicurezza account | ✅ | Vedi sopra; nessuna funzionalità GDPR-specifica oltre a questo (vedi riga dedicata in [1](#1-prodotto-e-regole-di-dominio)). |
| Sessione in `localStorage` | 🟡 | Scelta pragmatica per la fase attuale di sviluppo; da sostituire con cookie `httpOnly` + CSRF prima di un uso in produzione. |

## 5. Sviluppi futuri (non ancora iniziati)

Elencati nel blueprint come esplicitamente fuori dall'ambito attuale. Restano qui
come backlog, nell'ordine di priorità indicato:

| Specifica | Stato | Note |
|---|---|---|
| Federazione AT Protocol (Bluesky) | ⚪ | Priorità 1. Unico prerequisito già presente: UUID come chiave primaria su tutte le entità. |
| Federazione ActivityPub (Mastodon) | ⚪ | Priorità 2, dopo AT Protocol. Stack di federazione incompatibile con AT Protocol: richiederà un'implementazione separata. |
| Conteggio like, citazioni, condivisioni | ⚪ | |
| Dominio custom dell'utente associato al proprio blog | ⚪ | Dipende dal lavoro di routing per sottodominio (vedi [3](#3-architettura-stack-e-infrastruttura)). |

## Come usare questo documento

- Prima di iniziare un nuovo blocco di lavoro, controlla qui se è già presente
  (anche parzialmente) per evitare di duplicare cose fatte.
- Quando un task passa da 🟡/⚪ a ✅ (o viceversa, se si scopre una regressione),
  aggiorna la riga corrispondente nella stessa modifica che tocca il codice.
- Le righe 🟡 e ⚪ sono, nell'insieme, il backlog: per proporre un prossimo
  passo, si parte da lì.
