# Notturni

Piattaforma di microblogging opensource, multilingua, EU centrica & GDPR compliance.

Questo file documenta la struttura del repository e come avviare l'ambiente di
sviluppo — per una guida passo passo al primo avvio locale e un primo giro di
test, vedi [GETTING_STARTED.md](GETTING_STARTED.md) (installazioni complete: Podman/Docker, K3s, VM Debian/Ubuntu e Rocky/Alma in [docs/INSTALL.md](docs/INSTALL.md)); per l'elenco completo
delle specifiche di prodotto e il loro stato di avanzamento, vedi
[ROADMAP.md](ROADMAP.md).

## Stack

- **Frontend:** Next.js (App Router), Tailwind CSS
- **Backend:** Rust (workspace Cargo, `unsafe` vietato), axum, sqlx. Moduli dichiarativi
  (`module.toml`/`api.toml`/`db.toml`) da cui si generano DTO, rotte e migrazioni; core e moduli
  opzionali attivabili a caldo — vedi [docs/BACKEND.md](docs/BACKEND.md)
- **Autenticazione:** password (Argon2) + sessioni JWT, MFA (TOTP cifrato a riposo/email), SSO
  OAuth2/OIDC, API token per accesso diretto/motore core
- **Database:** PostgreSQL 16+
- **Cache & State:** Redis 7+ (rate limit, lock)
- **Message Broker:** NATS JetStream (eventi e job, outbox transazionale nel database)
- **Object Storage:** backend S3-compatible (SDK AWS per Rust) (Garage in locale e in produzione, scelta primaria del progetto — configurabile verso qualunque endpoint S3-compatible/AWS S3 in produzione, con auth via access key o ruolo AWS) oppure filesystem locale (`NOCT_STORAGE_BACKEND=localstorage`, alternativa per installazioni `solo` senza storage S3)
- **Dev locale:** Podman (rootless, senza Kubernetes)
- **Produzione:** Kubernetes (K3s), Traefik (ingress), Longhorn (storage), cert-manager

## Funzionalità implementate

- **Auth completa:** registrazione/login con password (policy minima 10
  caratteri), password dimenticata (reset via codice email, revoca tutte le
  sessioni attive), sessioni JWT (access + refresh con rotation), MFA via
  TOTP o email, SSO (Google/Microsoft/GitHub/LinkedIn, non ancora esposto in
  UI in attesa di un test end-to-end con credenziali reali) con account
  linking e gate 2FA.
- **Blog, post e commenti:** CRUD con le regole di dominio del blueprint (slug,
  limite 5 blog/utente, moderazione commenti). Post in Markdown, con workflow
  di stato completo (bozza → in revisione → pubblicato, con pianificazione
  della pubblicazione) e ruolo Revisore collegato a una capacità reale. Ogni
  blog ha sottotitolo e descrizione breve e una **visibilità** (pubblica /
  solo iscritti alla piattaforma / privata come diario del solo proprietario).
- **Collaboratori del blog:** il proprietario invita altri utenti come
  co-autore o mediatore; l'invito diventa una membership solo dopo che
  l'invitato lo accetta dalla propria dashboard. Ogni collaboratore può
  scegliere un alias con cui firmare i post su quel blog.
- **Editor WYSIWYG (Tiptap):** titoli, grassetto/corsivo/barrato/codice/link,
  citazioni, elenchi, tabelle, immagini con testo ALT, immagine di copertina,
  `@menzioni` con autocomplete che diventano link al profilo citato
  (disattivabili per blog), note a piè di pagina; esperienza senza bordi,
  ariosa, ispirata a fika.bar. Il contenuto resta salvato in Markdown,
  nessun rendering lato backend. Immagini in stile Bluesky (pillola ALT e
  rimozione in sovraimpressione, sfocate con click-per-rivelare già
  nell'editor se segnalate sensibili, un modal per scegliere manualmente
  l'avviso — Suggestivo/Nudità/Esplicito/Contenuto sensibile). Incollare un
  link da solo aggiunge anche una card di anteprima (titolo, descrizione,
  immagine) sotto il testo del link.
- **Note, media e link: bibliografia automatica del blog.** Ogni post ha un
  elenco di note a piè di pagina, mostrate numerate in fondo alla pagina
  pubblica del post con il testo come tooltip sul riferimento. L'editor
  inserisce una nota da un modal (stesso stile dell'avviso sui contenuti
  sensibili delle immagini): solo il testo libero è obbligatorio, dietro un
  toggle si possono aggiungere titolo, autore, ISBN, DOI e pagina per
  bibliografie strutturate, mostrati poi in fondo al post e in bibliografia.
  Una pagina `/{blog}/bibliografia` raccoglie tutte le note dei post
  pubblicati, deduplicate, con i post che le citano; allo stesso modo
  `/{blog}/media` e `/{blog}/link` raccolgono le immagini e i link citati nel
  corpo dei post,
  con la data di pubblicazione di ciascuna citazione.
- **Frammenti:** il lettore può selezionare con il mouse una porzione di
  testo di un post pubblicato (max 15%) e salvarla in una raccolta personale
  unificata (`/dashboard/frammenti`, con autore, titolo del post, data e
  link al post originale). Il frammento salvato viene ri-evidenziato in
  ciano tenue ad ogni lettura successiva del post, indipendentemente da come
  ci si è arrivati.
- **Tag e categorie sui post:** tag liberi (massimo 5, da campo dedicato e/o
  `#hashtag` nel testo) e categorie per-blog definite in anticipo dagli
  autori (al più una per post) — usati anche per filtrare il feed della
  homepage.
- **Moderazione automatica delle immagini:** servizio self-hosted separato
  (`moderation/`, modello open-source per rilevare nudità/contenuti
  sensibili) chiamato ad ogni upload; nessuna immagine lascia
  l'infrastruttura. Le immagini segnalate restano sfocate finché non ci si
  clicca sopra, sia nel contenuto del post sia in copertina. "Fail open":
  un problema del servizio di moderazione non blocca mai l'upload.
- **Permalink leggibili e homepage aggregata:** ogni post è raggiungibile
  anche senza sottodominio, con un permalink stile WordPress
  (`/{blog}/{slug}`, senza UUID nell'URL). La homepage della
  piattaforma mostra il feed cronologico di tutti i blog, con una sezione
  "di tendenza" basata sui tag più usati.
- **Media e backup su S3:** immagini incorporabili nei post (pubbliche) e una
  copia di backup del Markdown di ogni post pubblicato (privata), entrambe
  sotto `s3://{bucket}/{site_slug}/userdata/{user}/{blog}/...`; ogni giorno
  anche `pg_dump` e una copia di tutti i bucket verso uno storage S3
  esterno (modulo `backup`).
- **Moduli a caldo:** i moduli opzionali (commenti, newsletter, MFA, SSO,
  GDPR, ...) si attivano e disattivano dall'amministrazione senza riavviare
  il backend; le tabelle di un modulo spento restano intatte.
- **Multilingua (i18n):** post e pagine statiche traducibili (locale + slug per
  lingua, con selettore delle lingue di fallback del profilo invece di
  scrivere la sigla a mano); la struttura di path `/it/`, `/en/`, ... è una
  convenzione lasciata al routing pubblico del frontend (non ancora
  costruito: oggi il frontend seleziona la lingua via parametro, non via
  path).
- **Pagine statiche:** Chi siamo, Contatti, Privacy, ecc. del sito
  principale, gestite da Amministratore/Super Admin (sempre attive,
  permalink `/p/{slug}`); estese anche ai singoli blog come feature
  opt-in per il proprietario (disattiva di default), stessa interfaccia di
  editing, permalink `/{blog}/pagina/{slug}`.
- **Aspetto personalizzabile per blog:** palette/tipografia/layout in JSON
  libero (`blog_configs`), con i vincoli del blueprint (max 5 colori, max 3
  font); nome pubblico predefinito per gli autori/co-autori del blog;
  immagine di copertina (banner della home pubblica) e favicon dedicata,
  entrambe facoltative.
- **Profilo utente:** bio estesa (nome, cognome, paese, lingua madre e lingue
  di fallback), username modificabile (al massimo un cambio ogni 5 giorni;
  citato ovunque per id, non per stringa: un cambio consentito si riflette
  subito su post, commenti e autocomplete `@menzioni`, verificato in tempo
  reale in fase di registrazione), cambio password da loggati (vecchia +
  nuova password, revoca tutte le sessioni attive, oltre al recupero via
  email già esistente), cambio email con
  doppia verifica (codice alla vecchia casella e poi alla nuova), alias
  pubblico globale alternativo allo username, scelta di cosa mostrare come
  nome autore sui propri post (username, nome e cognome, o alias), avatar
  (upload su Garage/S3), link social fissi con icona monocromatica da un file
  di configurazione facilmente editabile. **Verifica del profilo:** dominio
  personalizzato verificato tramite record DNS (stile Bluesky) assegna un
  sigillo di verifica bronzo (argento/oro/blu riservati per il futuro); ID
  fediverse (AT Protocol + ActivityPub) creati come placeholder e mostrati
  nel profilo, in vista di una federazione non ancora attiva.
- **Follow:** utenti che seguono altri utenti o blog — anche in forma
  anonima, seguendo un blog che si presenta con un alias diverso dal nome di
  chi lo gestisce, la cui identità reale non compare mai fuori dalle pagine
  di gestione del proprietario. Chi possiede un blog vede, solo nel proprio
  profilo, il totale dei follower sommato tra username e alias, oltre al
  conteggio separato per ciascuna entità.
- **Footer di piattaforma:** 3 colonne + barra inferiore in Markdown libero
  (immagini/link inclusi), gestite da un Super Admin, mostrate su ogni pagina
  pubblica — di piattaforma e di ogni blog. Le colonne 1/2 sono un default
  sovrascrivibile per singolo blog dal suo proprietario; colonna 3 e barra
  inferiore restano sempre e solo di piattaforma.
- **Amministrazione:** sezioni sotto `/admin`, visibili solo ad
  Amministratore/Super Admin, con un campo di ricerca in ogni sezione —
  Utenti (ruolo, attivazione; l'assegnazione dei ruoli di amministrazione è
  riservata al Super Admin; nascosta in modalità `solo`), Pagine statiche
  (stesso editor Tiptap dei post), Tutti i blog (elenco piattaforma con
  proprietario/visibilità, sospensione di un blog — blocca lettura e
  scrittura pubbliche, anche per il proprietario), Moderazione (elenco di
  tutti i post della piattaforma, nascondi/mostra il singolo post
  indipendentemente dal suo stato).
- **API token:** accesso diretto per il motore core, predisposto per il futuro
  utilizzo diretto da parte degli utenti.
- **Frontend:** interfaccia autore (dashboard, editor, profilo), tema
  chiaro/scuro/automatico (alba-tramonto in base alla posizione, calcolata
  solo lato client), interfaccia in italiano e inglese (`next-intl`,
  selettore nell'header e nel profilo — la lingua dei contenuti è a parte),
  palette, tipografia e layout personalizzati del blog applicati alle sue
  pagine pubbliche.
- **Pubblicazioni:** serie di post come capitoli sotto `/{blog}/pub/{nome}`,
  con indice automatico, ordine esplicito e navigazione tra capitoli.
- **RSS/Atom:** feed cronologico di piattaforma (`/feed.xml`, `/atom.xml`) e
  per ogni blog (`/{blog}/feed.xml`, `/{blog}/atom.xml`), con autodiscovery.
- **Newsletter:** iscrizione pubblica con doppio opt-in via email (per singolo
  blog o al digest di piattaforma), notifica automatica ad ogni post
  pubblicato (disattivabile per blog) e campagne manuali dalla dashboard,
  disiscrizione e cancellazione dati senza bisogno di login.
- **SEO:** canonical, Open Graph/Twitter card e dati strutturati JSON-LD
  (`BlogPosting`) su tutte le pagine pubbliche (post, home di piattaforma e
  di blog, bibliografia/media/link, pagine statiche, profilo utente),
  `sitemap.xml`/`robots.txt` generati dinamicamente e coerenti con l'opt-in
  crawler per blog/post.
- **Gestione del blog:** panoramica con letture aggregate (senza cookie),
  libreria media e libreria note (tipo, DOI, duplicati, BibTeX), coda
  commenti con bloccati e segnalazioni, pausa/trasferimento/export ZIP/
  cancellazione con 30 giorni di tolleranza. Viste aggregate nella sidebar
  della dashboard (Post, Media, Pubblicazioni, Link, Bibliografia) su
  **tutti** i blog dell'utente insieme, ciascun elemento etichettato con il
  blog di provenienza.
- **Amministrazione estesa:** segnalazioni dai lettori con pannello e nota di
  audit obbligatoria, impostazioni di piattaforma persistite, coda richieste
  GDPR con seconda approvazione, lingua dell'interfaccia (it/en).
- **Self-hosting:** hostname/FQDN e modalità di installazione configurabili
  (`NOCT_INSTANCE_FQDN`, `NOCT_DEPLOYMENT_MODE=solo|platform`) — "solo" per un
  blog/sito personale a singolo proprietario (il primo utente registrato
  diventa Super Admin, la registrazione si chiude dopo), "platform" per lo
  stack multiutente descritto nel resto di questo documento. Ogni entità nel
  database ha un UUID come chiave primaria, prerequisito per un'eventuale
  federazione futura (vedi [ROADMAP.md](ROADMAP.md#5-sviluppi-futuri-non-ancora-iniziati),
  ATProto in prima battuta).

Dettagli, esempi di richiesta/risposta e limitazioni note (SSO non testabile
end-to-end senza credenziali reali) sono in [docs/API.md](docs/API.md). Per lo stato di
avanzamento di ogni funzionalità, incluse quelle ancora parziali o non
iniziate, vedi [ROADMAP.md](ROADMAP.md).

## Cosa resta da fare

In sintesi, ad alto livello (l'elenco completo, specifica per specifica, è in
[ROADMAP.md](ROADMAP.md)):

- **Routing per sottodominio** (`https://nomeutente.notturni.eu`) e dominio
  custom associato al blog — oggi ogni post è comunque raggiungibile senza
  sottodominio, via permalink su `notturni.eu`.
- **Registro dei consensi** GDPR (export, cancellazione con coda e seconda
  approvazione, cancellazione blog con tolleranza sono già presenti).
- **Lock distribuiti** su Redis (il rate limiting invece è già in uso, su
  login e anteprima link) e **clusterizzazione** dei componenti in produzione
  (oggi Kubernetes a nodo singolo).
- **Conteggio di like, citazioni e condivisioni**, e **federazione** con altre
  istanze/piattaforme (AT Protocol/Bluesky in prima battuta, poi
  ActivityPub/Mastodon) — esplicitamente fuori dall'ambito attuale, con
  l'UUID come chiave primaria già in posto in vista di questo lavoro futuro.

## Struttura del repository

```text
.
├── bins/                # noct-api (API HTTP), noct-worker (job e consumer), noct-ctl (strumenti)
├── crates/               # core del backend Rust: registry, runtime, auth, utenti, profili, blog, post, media,
│                           audit, amministrazione, bus, cache, storage, mail, markdown, sicurezza, codegen...
├── modules/              # moduli opzionali: commenti, newsletter, MFA, SSO, GDPR, segnalazioni, scoperta,
│                           pagine, backup... (ciascuno con module.toml, api.toml, db.toml)
├── docs/                 # API.md (contratto HTTP) e BACKEND.md (sviluppo e operazioni del backend)
├── moderation/          # microservizio FastAPI di moderazione automatica delle immagini
│   ├── app/                # modello self-hosted (Falconsai/nsfw_image_detection via transformers)
│   └── Dockerfile           # pesi del modello inclusi nell'immagine in fase di build
├── frontend/            # applicazione Next.js (pubblica + dashboard + amministrazione)
│   ├── src/
│   │   ├── app/            # login/register, dashboard (sezioni personali), admin (pagine/utenti/blog/moderazione), homepage, pagina pubblica del post e del blog, profilo pubblico
│   │   ├── lib/             # client API, sessione (auth-context), tema (theme-context, sun.ts), Markdown/social/lingue
│   │   └── components/       # UI condivisa (Button, Card, SearchInput, ...), editor/ (Tiptap, tag, categorie, copertina, traduzioni), ThemeToggle
│   └── Dockerfile
├── Dockerfile           # immagine del backend (cargo-chef, multi-arch): noct-api, noct-worker, noct-ctl
├── k8s/                  # manifest Kubernetes
├── garage/, s3-public/  # configurazione di Garage e del proxy delle letture pubbliche
├── compose.yaml         # stack locale via Podman/Docker compose (include il servizio moderation)
├── deny.toml            # cargo-deny: licenze, fonti, vulnerabilità
└── .env.example          # variabili d'ambiente (prefisso NOCT_)
```

## Avvio rapido in locale

```bash
cp .env.example .env
podman compose up -d --build
```

Servizi esposti:

- Frontend (dashboard e amministrazione incluse): <http://localhost:3000>
- Backend (OpenAPI): <http://localhost:8000/api/v1/openapi.json>
- Backend (health): <http://localhost:8000/api/v1/health>, readiness: <http://localhost:8000/api/v1/health/ready>
- Moderazione immagini (health): <http://localhost:8100/health>
- Email di sviluppo (MailHog): <http://localhost:8025>
- Storage S3 (Garage): API <http://localhost:3900>, letture pubbliche <http://localhost:9000>

Le migrazioni dei moduli attivi si applicano da sole all'avvio (advisory lock: sicuro con più repliche;
`NOCT_AUTO_MIGRATE=false` per applicarle altrove con `noct-ctl migrate apply`) e il Super Admin si
crea da `NOCT_SUPER_ADMIN_*`.

Se `podman compose` non è disponibile (richiede `podman-compose` o il plugin
`docker-compose`), vedi [GETTING_STARTED.md](GETTING_STARTED.md#prerequisiti)
per le alternative, oppure avvia backend/frontend in locale seguendo
[docs/BACKEND.md](docs/BACKEND.md) e [frontend/README.md](frontend/README.md).

## Schema del database

Ogni modulo dichiara le proprie tabelle in `db.toml`; `noct-ctl migrate generate` ne ricava le
migrazioni SQL (`crates/*/migrations/`, `modules/*/migrations/`) e aggiorna `schema.lock.json`. Le
modifiche additive (tabelle, colonne con default o nullable, indici, enum) sono automatiche; quelle
distruttive richiedono SQL scritto a mano. `noct-ctl validate` verifica in CI che manifest, lock e
migrazioni non divergano.

## API e autenticazione

Riferimento di tutti gli endpoint, i due meccanismi di autenticazione
(sessione utente vs API token), formato del token e come ottenere il primo:
[docs/API.md](docs/API.md). Lo schema OpenAPI, sempre allineato ai moduli attivi, è su
`/api/v1/openapi.json`.

Elenco per modulo di tutte le rotte (verbo, accesso, scopo): [docs/API_REFERENCE.md](docs/API_REFERENCE.md).
Tutte le variabili d'ambiente, con default ed esempi: [docs/CONFIGURATION.md](docs/CONFIGURATION.md).

## Test

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo deny check
```

I test end-to-end girano su Postgres veri (un database per test, creato e distrutto dal test stesso) e
usano Redis, NATS JetStream e un S3 finto dove il caso lo richiede: variabili `NOCT_TEST_*` e servizi di
prova in [docs/BACKEND.md](docs/BACKEND.md#sviluppo). Frontend: `npx tsc --noEmit`, `npx eslint`,
`npm run build` in `frontend/`.

## CI e sicurezza

Quattro GitHub Actions in [.github/workflows/](.github/workflows/):

- [ci.yml](.github/workflows/ci.yml) — a ogni push e pull request: `cargo fmt`, `cargo clippy` con i
  warning come errori, coerenza di manifest e migrazioni (`noct-ctl validate`, `migrate plan/apply/verify`),
  l'intera suite di test su Postgres, Redis, NATS e un S3 finto, licenze e fonti delle dipendenze
  (`cargo-deny`) e vulnerabilità note (RustSec, `cargo-audit`).
- [codeql.yml](.github/workflows/codeql.yml) — SAST (analisi statica) via CodeQL su backend (Rust),
  moderazione (Python) e frontend (TypeScript/JavaScript). Report nella tab **Security → Code scanning**
  del repository, non come artifact. Su push/PR verso `main`/`develop`, avvio manuale e schedule settimanale.
- [sbom.yml](.github/workflows/sbom.yml) — SBOM (Software Bill of Materials,
  formato CycloneDX): un inventario delle dipendenze, non un'analisi di
  vulnerabilità. Un livello dai sorgenti (`Cargo.lock`, frontend, moderazione, sempre
  veloce) e uno dalle immagini container effettivamente costruite
  (backend/frontend). Report come artifact del workflow. Stessi trigger di
  `codeql.yml`.
- [dast.yml](.github/workflows/dast.yml) — DAST (analisi dinamica): avvia
  l'intero stack da `compose.yaml` (con Docker invece di Podman — nessuna
  sintassi specifica di Podman nel file) ed esegue OWASP ZAP contro le pagine
  pubbliche del frontend e l'API del backend (da `/api/v1/openapi.json`). Più
  pesante degli altri: solo avvio manuale e schedule settimanale, mai su
  ogni push/PR. Report come artifact del workflow.
- [publish-images.yml](.github/workflows/publish-images.yml) — immagini multi-arch (amd64 e arm64) di
  backend, frontend e moderazione su GHCR.

SAST, SBOM e DAST non fanno fallire la build in presenza di segnalazioni (`fail_action:
false`/nessun controllo di soglia): producono un report da consultare, non un gate che blocca il merge —
da rivedere se in futuro si vorrà imporre una soglia. `ci.yml` invece blocca.

## Backend e frontend in locale (senza container)

Vedi [docs/BACKEND.md](docs/BACKEND.md) e [frontend/README.md](frontend/README.md).

## Kubernetes

Vedi [k8s/README.md](k8s/README.md).

## Specifiche e roadmap

Tabella di tutte le specifiche di prodotto con il relativo stato di
avanzamento (implementato / parziale / da fare) e il backlog dei prossimi
sviluppi: [ROADMAP.md](ROADMAP.md).
