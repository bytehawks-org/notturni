# Notturni - Agentic Development Blueprint

* Ambiente di sviluppo: Fedora44 utilizzando Podman come sistema di containerizzazione degli applicativi.
* Ambiente di produzione: Kubernetes (probabilmente K3s su nodo singolo all'inizio, da espandere in cluster se il progetto avrà successo).

## 1. Documentazione del progetto — cosa guardare per cosa

Questo file contiene le istruzioni per l'agente. La visione di prodotto (cosa
deve fare la piattaforma) sta in [docs/ABSTRACT.md](docs/ABSTRACT.md). Lo stato
di avanzamento sta in ROADMAP.md, non qui.

Questo file non è mai citato dalla documentazione pubblica/tracciata (README,
docs/, ecc.). Se trovi una citazione in un `.md` tracciato, è un refuso da
correggere quando capita di passarci vicino.

| Serve... | Guarda in... |
| --- | --- |
| Se una specifica è già implementata, parziale o da fare | [ROADMAP.md](ROADMAP.md) — **controllalo sempre prima di iniziare un blocco di lavoro** |
| Visione di prodotto, regole di dominio, stack | [docs/ABSTRACT.md](docs/ABSTRACT.md) |
| Panoramica, struttura del repository | [README.md](README.md) |
| Cronologia delle modifiche | [CHANGELOG.md](CHANGELOG.md) |
| Primo avvio locale passo-passo, primo giro di test | [GETTING_STARTED.md](GETTING_STARTED.md) |
| Riferimento endpoint (richieste/risposte, autorizzazione) | [docs/API.md](docs/API.md), [docs/API_REFERENCE.md](docs/API_REFERENCE.md) |
| Struttura, sviluppo e test del backend | [docs/BACKEND.md](docs/BACKEND.md) |
| Variabili d'ambiente e configurazione | [docs/CONFIGURATION.md](docs/CONFIGURATION.md) |
| Installazione (Podman, K3s) | [docs/INSTALL.md](docs/INSTALL.md) |
| Setup frontend | [frontend/README.md](frontend/README.md) |
| Manifest Kubernetes | [k8s/README.md](k8s/README.md) |
| Servizio di moderazione immagini | [moderation/README.md](moderation/README.md) |

Questo file **non duplica** lo stato di avanzamento: in caso di discrepanza
fidati di ROADMAP.md.

## 2. Modalità di lavoro con l'agente (sessioni a blocchi)

Il lavoro su questo repository procede a blocchi piccoli e scoped, uno per
sessione/richiesta — non a "big bang" su più funzionalità insieme.

1. **Prima di iniziare un blocco**: leggi la riga rilevante in ROADMAP.md. Se
   è già ✅ non ripartire da zero: chiedi (o verifica nel codice) cosa manca
   davvero prima di scrivere qualsiasi cosa.
2. **Scope**: implementa solo il blocco esplicitamente richiesto in questa
   sessione. Non anticipare il blocco successivo — anche se è il prossimo
   🟡/⚪ più ovvio in ROADMAP.md — senza un'indicazione esplicita dell'utente.
   Chiudere una sessione ("per oggi ci fermiamo qui", aggiornare i doc e fare
   commit) non è un via libera a proseguire da soli sul prossimo blocco alla
   sessione successiva.
3. **Verifica prima di dichiarare fatto un blocco**, nell'ordine che si
   applica alla modifica (gli stessi comandi della CI, `.github/workflows/ci.yml`):
   * Backend (Rust):
     * `cargo fmt --all -- --check`
     * `cargo clippy --workspace --all-targets --locked -- -D warnings`
     * `cargo run --locked -q -p noct-ctl -- validate`
     * `cargo run --locked -q -p noct-ctl -- migrate plan` (e `apply` / `verify`
       **solo** su un DB locale/effimero, mai su staging o produzione)
     * `cargo run --locked -q -p noct-ctl -- openapi`
     * `cargo test --workspace --locked` (richiede PostgreSQL e NATS JetStream,
       vedi il workflow CI e [docs/BACKEND.md](docs/BACKEND.md#development))
     * `cargo deny` / `cargo audit` se cambiano le dipendenze
   * Frontend (in `frontend/`): `npx tsc --noEmit`, `npm run lint`, `npm run build`.
   * Se la modifica cambia comportamento a runtime (non solo tipi/test):
     rebuild del/i servizio/i coinvolto/i (`podman compose build --no-cache
     <servizio>`), `podman compose up -d --force-recreate`, verifica **live**
     via `curl` contro lo stack realmente in esecuzione. Non dichiarare un
     comportamento verificato sulla base del solo codice sorgente o dei soli
     test se è ragionevolmente veloce controllarlo dal vivo.
4. **Documentazione**, nella stessa modifica che tocca il codice, non in un
   passaggio successivo:
   * Aggiorna la riga corrispondente in ROADMAP.md (stato + nota).
   * Aggiorna [docs/API.md](docs/API.md) se cambiano endpoint, payload
     o risposta.
   * Aggiorna README.md solo se cambia la lista "Funzionalità implementate" a
     livello di onboarding — non per ogni dettaglio interno.
5. **Git**:
   * Messaggi di commit in italiano, che spiegano cosa è cambiato e perché,
     non solo un titolo.
   * `git add` mirato ai soli file effettivamente toccati dal blocco, mai
     `git add -A`/`git add .` alla cieca.
   * Nomi branch per nuove funzionalità: `feat/<slug-breve>`.
   * Commit e push solo quando l'utente lo chiede esplicitamente per quella
     sessione — non presumere che "vai avanti" implichi anche pubblicare.
6. **Gestione dei lavori estesi**: epiche e task stanno in `.aiops/`
   (non versionata, esclusa tramite `.git/info/exclude`, mai committata).
   Procedura: skill `aiops`.

## 3. Vincoli operativi permanenti

Da rispettare sempre, indipendentemente dal blocco in corso:

* **Non toccare mai il file `LICENSE`.**
* **Nessun file `.md` tracciato nel repository deve citare questo file** —
  è documentazione interna, non linkata pubblicamente.
* **Dockerfile e manifest agnostici rispetto all'architettura** (x86_64 in
  sviluppo, ARM64 in produzione): niente immagini di base con tag
  arch-specifico (es. `arm64v8/...`) fuori da un contesto che lo richieda
  esplicitamente. Non usarle negli esempi/script pensati per girare in locale.
* **Compatibilità S3**: ogni interazione con lo storage S3-compatible deve
  iniettare un endpoint custom configurabile via env — mai
  `localhost`/l'endpoint AWS hard-coded — per restare intercambiabile tra
  Garage locale e S3/R2 in produzione. Autenticazione via
  `NOCT_S3_ACCESS_KEY_ID`/`NOCT_S3_SECRET_ACCESS_KEY` se entrambe valorizzate,
  altrimenti ruolo AWS (default credential chain); `NOCT_S3_REGION`
  opzionale. In alternativa a S3, `NOCT_STORAGE_BACKEND=localstorage` scrive
  su filesystem locale, servito dal backend stesso — pensato per
  installazioni `solo` senza hardware/competenze per gestire uno storage
  S3-compatible. Vedi `crates/noct-storage` e `crates/noct-core/src/config.rs`.
* **Prefisso env**: variabili Podman/Kubernetes con prefisso `NOCT_`/`noct_`
  (maiuscolo o minuscolo a seconda del contesto d'uso).
* **Formato delle risposte in chat**: evitare intro o conclusioni
  riepilogative superflue; per soluzioni rapide dare lo snippet/comando
  essenziale, per soluzioni estese il file completo con i riferimenti
  necessari.
* **Login backend**: autentica per `email`, non per `username`
  (`POST /api/v1/auth/login`) — capita di sbagliarlo durante una verifica
  live.

## 4. Insidie note (da non re-scoprire ogni volta)

* **PATCH con campi tri-stato** (assente / esplicitamente `null` / valore):
  un `Option<T>` serde non distingue "omesso" da `null`. Il codice legge la
  mappa JSON grezza (vedi `tristate_bool` in
  `crates/noct-posts/src/handlers_impl.rs`): riusa lo stesso approccio per
  nuovi campi tri-stato.
* **Immagini "sensibili" e blur via CSS**: la regola
  `.sensitive-image-wrapper img { filter: blur(...) }` in
  `frontend/src/app/globals.css` sfoca **qualunque** immagine dentro quel
  wrapper, incondizionatamente, e il meccanismo di rivelazione (checkbox +
  overlay) esiste solo se il wrapper viene renderizzato insieme a quegli
  elementi (vedi `frontend/src/components/blog/SensitiveImage.tsx`).
  Applicare la classe wrapper **solo** quando l'immagine è davvero segnalata
  come sensibile — mai in modo incondizionato: non c'è un meccanismo di
  sblocco senza checkbox/overlay.
* **`cover_image_is_sensitive` nel PATCH dei post**: ha effetto solo se
  inviato **nella stessa richiesta** che include anche un nuovo
  `cover_image_url` — comportamento intenzionale (vedi
  `crates/noct-posts/src/handlers_impl.rs` e [docs/API.md](docs/API.md)), non
  un bug da correggere se sembra "ignorato" inviandolo da solo.

## 5. Prodotto e stack

Visione di prodotto, regole di dominio, estetica, autenticazione e sviluppi
futuri: [docs/ABSTRACT.md](docs/ABSTRACT.md). Non duplicarli qui.

Stack in una riga: workspace Cargo (`crates/`, axum, sqlx), PostgreSQL 16+,
Redis 7+, NATS JetStream, Garage/S3-compatible, frontend Next.js (App Router)
con Tailwind CSS; sviluppo con Podman (`compose.yaml`), produzione su K3s
(`k8s/`, ingress Traefik).

## 6. Ambiente locale

Lo stack locale è definito per intero in [compose.yaml](compose.yaml) (servizi:
`postgres`, `redis`, `nats`, `mailhog`, `smtp-relay`, `garage`,
`garage-bootstrap`, `s3-public`, `moderation`, `backend`, `worker`,
`frontend`) e si avvia con `podman compose up -d --build`. Non avviare i
singoli componenti a mano con `podman run`/`podman pod create`:
`compose.yaml` include healthcheck, dipendenze d'avvio (`depends_on` con
`condition`) e le variabili d'ambiente di rete interne al pod di compose che
una sequenza di `podman run` separati non replica.

Passo-passo completo: [GETTING_STARTED.md](GETTING_STARTED.md); installazione e
configurazione: [docs/INSTALL.md](docs/INSTALL.md),
[docs/CONFIGURATION.md](docs/CONFIGURATION.md).
