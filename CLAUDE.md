# Notturni - Agentic Development Blueprint

* Ambiente di sviluppo: Fedora44 utilizzando Podman come sistema di containerizzazione degli applicativi.
* Ambiente di produzione: Kubernetes (probabilmente K3s su nodo singolo all'inizio, da espandere in cluster se il progetto avrà successo).

## 1. Documentazione del progetto — cosa guardare per cosa

Questo file è la fonte **interna** della visione di prodotto: cosa deve fare
la piattaforma, non lo stato di avanzamento di cosa è già stato costruito.
Non è mai citato dalla documentazione pubblica/tracciata (README, API.md,
ecc.) — se lo trovi citato lì, è un refuso da correggere quando capita di
passarci vicino, non un pattern da ripetere (vedi [3](#3-vincoli-operativi-permanenti)).

Per tutto il resto, la fonte giusta dipende da cosa serve:

| Serve... | Guarda in... |
| --- | --- |
| Se una specifica è già implementata, parziale o da fare | [ROADMAP.md](ROADMAP.md) — **controllalo sempre prima di iniziare un blocco di lavoro** |
| Panoramica, stack, struttura del repository | [README.md](README.md) |
| Primo avvio locale passo-passo, primo giro di test | [GETTING_STARTED.md](GETTING_STARTED.md) |
| Riferimento endpoint (richieste/risposte, autorizzazione) | [backend/API.md](backend/API.md) |
| Setup/test/migrazioni backend | [backend/README.md](backend/README.md) |
| Setup frontend | [frontend/README.md](frontend/README.md) |
| Manifest Kubernetes | [k8s/README.md](k8s/README.md) |
| Servizio di moderazione immagini | [moderation/README.md](moderation/README.md) |

Questo file **non duplica** lo stato di avanzamento: se una frase qui sotto
descrive qualcosa come fatto, in caso di dubbio o discrepanza fidati di
ROADMAP.md, non di questo file.

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
   applica alla modifica:
   * Backend: `python -m pytest` in `backend/` (vedi [backend/README.md](backend/README.md#test)).
   * Frontend: `tsc`, `eslint`, `npm run build`.
   * Se la modifica cambia comportamento a runtime (non solo tipi/test):
     rebuild del/i servizio/i coinvolto/i (`podman compose build --no-cache
     <servizio>`), `podman compose up -d --force-recreate`, verifica **live**
     via `curl` contro lo stack realmente in esecuzione. Non dichiarare un
     comportamento verificato sulla base del solo codice sorgente o dei soli
     test se è ragionevolmente veloce controllarlo dal vivo.
4. **Documentazione**, nella stessa modifica che tocca il codice, non in un
   passaggio successivo:
   * Aggiorna la riga corrispondente in ROADMAP.md (stato + nota).
   * Aggiorna [backend/API.md](backend/API.md) se cambiano endpoint, payload
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

## 3. Vincoli operativi permanenti

Da rispettare sempre, indipendentemente dal blocco in corso:

* **Non toccare mai il file `LICENSE`.**
* **Nessun file `.md` tracciato nel repository deve citare `CLAUDE.md`** — è
  documentazione interna, non linkata pubblicamente (lo stesso ROADMAP.md lo
  richiama in apertura). Alcune citazioni residue da prima di questa regola
  esistono ancora in `backend/API.md`, `ROADMAP.md` e `moderation/README.md`:
  vanno ripulite in un blocco dedicato quando richiesto esplicitamente, non
  corrette di riflesso durante un blocco non correlato.
* **Dockerfile e manifest agnostici rispetto all'architettura** (x86_64 in
  sviluppo, ARM64 in produzione): niente immagini di base con tag
  arch-specifico (es. `arm64v8/...`) fuori da un contesto che lo richieda
  esplicitamente.
* **Compatibilità S3**: ogni interazione con lo storage S3-compatible
  (`boto3`) deve iniettare un endpoint custom configurabile via env — mai
  `localhost`/l'endpoint AWS hard-coded — per restare intercambiabile tra
  MinIO locale e S3/R2 in produzione. Autenticazione via
  `NOCT_S3_ACCESS_KEY_ID`/`NOCT_S3_SECRET_ACCESS_KEY` se entrambe valorizzate,
  altrimenti ruolo AWS (default credential chain di boto3); `NOCT_S3_REGION`
  opzionale. In alternativa a S3, `NOCT_STORAGE_BACKEND=localstorage` scrive
  su filesystem locale, servito dal backend stesso — pensato per
  installazioni `solo` senza hardware/competenze per gestire uno storage
  S3-compatible. Vedi `backend/app/core/storage.py`.
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

* **SQLAlchemy async e relazioni**: un accesso lazy a una relazione fuori dal
  contesto della sessione async (es. dopo il `commit`, in un punto in cui
  l'oggetto non è stato ricaricato con `refresh`/eager load) fa fallire con
  `MissingGreenlet`. Nei path di scrittura con FK/many-to-many, ricaricare o
  fare eager load esplicito prima di serializzare la risposta.
* **PATCH con campi tri-stato** (assente / esplicitamente `null` / valore):
  usare `model_fields_set` di Pydantic v2 per distinguere "campo omesso dal
  payload" da "campo inviato come `null`" — non basta un default `None` sul
  modello.
* **Immagini "sensibili" e blur via CSS**: la regola
  `.sensitive-image-wrapper img { filter: blur(...) }` in `globals.css`
  sfoca **qualunque** immagine dentro quel wrapper, incondizionatamente, e il
  meccanismo di rivelazione (checkbox + overlay) esiste solo se il wrapper
  viene renderizzato insieme a quegli elementi. Applicare la classe wrapper
  **solo** quando l'immagine è davvero segnalata come sensibile — mai in modo
  incondizionato con l'idea di "tanto se non è sensibile non si vede
  l'effetto": non c'è un meccanismo di sblocco senza checkbox/overlay.
* **`cover_image_is_sensitive` nel PATCH dei post**: ha effetto solo se
  inviato **nella stessa richiesta** che include anche un nuovo
  `cover_image_url` — comportamento intenzionale, documentato nel codice e in
  `backend/API.md`, non un bug da correggere se sembra "ignorato" inviandolo
  da solo.
* **Filtri del feed** (tag, categoria, locale, ...): seguono tutti lo stesso
  pattern di join in `backend/app/api/v1/feed.py` — un nuovo filtro va
  aggiunto lì con lo stesso stile (`stmt.join(...).where(...)` condizionale),
  non con un endpoint/query separata.

## 5. Abstract del prodotto

Piattaforma opensource di microblogging e newsletter. Multiutente & multilingua con enfasi sul rispetto delle regolamentazioni GDPR e con un occhio alla visione EU centrica.
Il sistema dovrà prevedere la possibilità di creare fino ad un massimo di 5 blog/pubblicazioni per ogni utente.
Il nome dell'autore che apparirà nel blog e negli articoli potrà essere diverso dal nome dell'utente vero e proprio per garantire privacy e riservatezza.
Esisteranno 4 tipi di utenti: Super Admin, Amministratore, Moderatore e Utente. Il ruolo predefinito sarà "Utente". In fase di creazione delle risorse (Podman o Kubernetes) dovrà essere specificato un username e una password di "Super Admin".
Il sistema dovrà prevedere dei ruoli per gli utenti: Autore (default per Utente), Co-Autore, Revisore e Mediatore. Questi ruoli sono specifici per gli utenti e i blog non per la piattaforma come quelli descritti sopra.
La dashboard utente dovrà essere user-friendly, moderna e responsiva per essere utilizzata sia da PC che da dispositivi mobile.
La dashboard di amministrazione dovrà prevedere tutte le possibili funzionalità che una piattaforma del genere richiede per la parte amministrativa/moderazione.
L'editor dovrà essere pulito e focalizzato sui contenuti, testuali e visuali. Prendere ad esempio il portale fika.bar.
Possibilità di personalizzazione di colori, tipografia e presentazione dei contenuti da parte dell'utente, entro i vincoli della sezione Estetica qui sotto (palette e tipografia).
Prevedere un sistema di commenti e gestione di questi da parte dell'autore del post/blog. I commenti saranno possibili solo agli utenti registrati (di default). Il proprietario del blog può decidere se renderli aperti anche a chi non registrato ma sarà applicata una moderazione prima della pubblicazione.

Stato di avanzamento di ogni singolo punto qui sopra (implementato /
parziale / da fare, con dettagli di dove e come): [ROADMAP.md § 1](ROADMAP.md#1-prodotto-e-regole-di-dominio).

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

Stato di avanzamento: [ROADMAP.md § 2](ROADMAP.md#2-estetica).

### Sviluppi futuri da considerare ma non implementare adesso

Prevedere la possibilità di federare la piattaforma con dei suoi cloni e con altre istanze — priorità: prima AT Protocol (Bluesky), poi ActivityPub (Mastodon). Sono due stack di federazione incompatibili tra loro: richiederanno implementazioni separate. Le entità del database usano già UUID come chiave primaria, base ragionevole in vista di questo lavoro futuro, ma non è stato fatto altro in questa direzione.
Prevedere un meccanismo di conteggio dei "like", delle "citazioni" e delle "condivisioni".
Prevedere la possibilità di associare un dominio dell'utente al proprio blog/pubblicazione.

Backlog dettagliato: [ROADMAP.md § 5](ROADMAP.md#5-sviluppi-futuri-non-ancora-iniziati).

## 6. System Architecture & Stack

Anche se inizialmente sarà un sistema locale e successivamente su un singolo nodo Kubernetes prevedere la clusterizzazione dei componenti necessari.
In questa fase iniziale si predilige Python per la parte di backend. La roadmap dei prossimi 3 anni prevede già la sostituzione dei microservizi con applicativi scritti in Rust.
Come ingress verrà utilizzato Traefik.
Per la parte storage, anche su nodo singolo, si prevede di utilizzare comunque Longhorn.
Prevedere un backup su storage S3 esterno delle risorse fondamentali quali database e MinIO.
I blog/pubblicazioni dell'utente saranno nella forma `https://nomeutente.notturni.eu`.
All'indirizzo `https://notturni.eu` ci sarà una sorta di raccolta degli articoli nella lingua dell'utente come avviene sulla piattaforma dev.to. All'indirizzo `https://blog.notturni.eu` ci sarà il blog vero e proprio della piattaforma su cui comunicare news e aggiornamenti.
Prevedere un blocco al nome dei blog: devono essere di minimo 4 caratteri alfanumerici (può essere utilizzato il simbolo -). I nomi di 3 caratteri o meno sono riservati alla piattaforma (futura funzionalità premium per i sostenitori).
Definire una lista di parole da considerare blacklist per gli utenti in fase di registrazione di un blog (blog, www, mail, journal, api, admin, monitor, stats, status, ecc.).

**Architettura target:** sviluppo locale x86_64, produzione ARM64 — vedi il
vincolo Dockerfile/manifest agnostici in [3](#3-vincoli-operativi-permanenti).
Non usare immagini di base con tag arch-specifico (es. `arm64v8/...`) negli
esempi/script pensati per girare in locale.
**Container Engine (Local):** Podman su Fedora Linux (Rootless networking, no K8s).
**Frontend:** Next.js (App Router, SSR), Tailwind CSS.
**Backend:** Python 3.12+, FastAPI, Uvicorn (ASGI).
**Database:** PostgreSQL 16+ (Estensione `pgcrypto` o `uuid-ossp`).
**Cache & State:** Redis 7+ (Rate limiting, lock distribuiti).
**Message Broker:** RabbitMQ (Code asincrone, AMQP).
**Object Storage:** MinIO (S3-compatible, drop-in replacement per AWS S3 / Cloudflare R2).

In produzione i certificati saranno gestiti tramite cert-manager di Let's Encrypt.
Sarà presente un record dns *.notturni.eu che punterà al server o al bilanciatore dei server di produzione.

Stato di avanzamento: [ROADMAP.md § 3](ROADMAP.md#3-architettura-stack-e-infrastruttura).

## 7. Authentication & SSO Flow

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

* **Token API (accesso diretto, non tramite editor/admin):**
  * Tabella `api_tokens`, valore opaco con prefisso `noct_`, solo l'hash sha256 è persistito.
  * `owner_type` distingue token del motore core (nessun utente associato, uso machine-to-machine) da token utente (previsti per il futuro: permettere di interfacciarsi con l'API senza passare da editor o admin del proprio blog).

Stato di avanzamento (incluse le limitazioni note: SSO non testabile
end-to-end senza credenziali OAuth reali, consumer email OTP ancora
placeholder) ed endpoint: [ROADMAP.md § 4](ROADMAP.md#4-autenticazione-e-sso)
e [backend/API.md](backend/API.md).

## 8. Ambiente locale

Lo stack locale (Postgres, Redis, RabbitMQ, MinIO, servizio di moderazione,
backend, worker, frontend) è definito per intero in [compose.yaml](compose.yaml)
e si avvia con `podman compose up -d --build`. Non avviare i singoli
componenti a mano con `podman run`/`podman pod create`: `compose.yaml` include
healthcheck, dipendenze d'avvio (`depends_on` con `condition`) e le variabili
d'ambiente di rete interne al pod di compose che una sequenza di `podman run`
separati non replica.

Passo-passo completo (prerequisiti, configurazione, migrazioni, primo utente,
primo giro di test via interfaccia e via API diretta, problemi comuni):
[GETTING_STARTED.md](GETTING_STARTED.md).
