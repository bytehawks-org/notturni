# Rewrite in Rust — analisi e piano (v2, greenfield)

Branch: `migration/rust`. Il progetto non è in produzione: il codice Python è una
POC e diventa **specifica di riferimento**, non codice da portare riga per riga.
Nessuno strangler/proxy, nessuna migrazione di dati, nessun vincolo di
compatibilità dell'API (il frontend si adegua). Il frontend Next.js resta;
`moderation/` (PyTorch) resta Python per ora (vedi §8).

## 1. Cosa cambia rispetto a v1

| Tema | v1 | v2 |
|---|---|---|
| Strategia | strangler con reverse proxy | rewrite pulito, Python in `legacy/` solo come riferimento |
| DB | baseline sqlx + adozione DB esistenti | schema **dichiarativo per modulo**, nessun legacy |
| API | handler scritti a mano | **rotte dichiarate** in `api.toml`, handler Rust solo per la logica |
| Struttura | crate per area | **core** sempre attivo + **moduli** attivabili |
| Broker | RabbitMQ (`lapin`) | **NATS JetStream** dietro trait (§6) |

## 2. Approccio dichiarativo: cosa è realistico

Rust è tipizzato staticamente: un file di config **non può** sostituire gli handler
di logica (feed con filtri dinamici, newsletter, MFA...). Tre livelli possibili:

| Livello | Descrizione | Giudizio |
|---|---|---|
| A. Interprete a runtime | rotte e tabelle lette all'avvio, handler generici | perde type-safety e compile-time check; ok solo per CRUD puro |
| **B. Codegen a build-time** (`build.rs`) | `api.toml`/`db.toml` validati e trasformati in router axum, DTO serde, validazione, OpenAPI, schema desiderato | **consigliato**: errori in compilazione/CI, zero costo runtime |
| C. Plugin dinamici (dylib/WASM) | moduli caricati a runtime da terzi | fuori scope; rivalutabile (WASM) dopo la 1.0 |

Scelta: **B + toggle a runtime**. Ogni modulo è compilato nel binario (feature Cargo per
chi vuole un binario più piccolo), ma `NOCT_MODULES=...` (o `platform_config`)
decide quali montare ed eseguire all'avvio.

Formato: **TOML** (`module.toml`, `api.toml`, `db.toml`) invece di `.conf`: serde-nativo,
supporto editor/JSON Schema (taplo), nessun parser custom da mantenere.

### Cosa è dichiarativo e cosa no

| Dichiarativo | Resta codice Rust |
|---|---|
| Path, metodo, policy auth/ruolo, rate limit, schema request/response, paginazione, CRUD semplice | Logica di dominio, query complesse (feed, ricerca), invio mail, rendering |
| Tabelle, colonne, indici, FK, enum, default | Backfill dati, trasformazioni, migrazioni distruttive |
| Eventi pubblicati/sottoscritti, job e policy di retry | Corpo dell'handler del job/evento |
| Schema di configurazione del modulo (env, default, tipi) | — |

Regola anti-DSL-creep: se serve un'espressione condizionale nel `.toml`, è logica → va in Rust.

### Esempio (modulo `newsletter`, abbreviato)

```toml
# module.toml
[module]
name = "newsletter"
kind = "optional"            # core | optional
default_enabled = false
requires = ["core.auth", "core.blogs", "core.mail"]

[config]
batch_size = { type = "int", default = 100, env = "NOCT_NEWSLETTER_BATCH" }

[events]
subscribes = ["post.published"]
publishes  = ["newsletter.campaign.sent"]

[jobs.send_campaign]
subject = "newsletter.send"
max_deliver = 5
backoff = ["10s", "1m", "10m"]
```

```toml
# api.toml
[[route]]
id = "subscribe"
method = "POST"
path = "/blogs/{slug}/newsletter/subscribers"
auth = "public"              # public | user | api_token | platform_role:admin | blog_role:owner
rate_limit = { key = "ip", max = 5, window = "1m" }
request  = { email = "email!", locale = "locale?" }
response = "Subscriber"
handler  = "subscribe"       # funzione nel crate; omesso con kind = "crud"
```

```toml
# db.toml
[[table]]
name = "newsletter_subscribers"
columns = [
  { name = "id",      type = "uuid", pk = true, default = "gen_random_uuid()" },
  { name = "blog_id", type = "uuid", ref = "blogs.id", on_delete = "cascade" },
  { name = "email",   type = "text", null = false },
  { name = "locale",  type = "text", null = true },
]
indexes = [{ columns = ["blog_id", "email"], unique = true }]
```

### Migrazioni "trasparenti": cosa è sicuro automatizzare

Applicare in automatico un diff di schema su dati reali è il punto più rischioso.
Proposta: il motore genera **file SQL versionati e con checksum** (committati e
rivedibili), li applica all'avvio sotto advisory lock, registra tutto in
`noct_migrations(module, version, checksum)`.

| Cambiamento | Comportamento |
|---|---|
| Nuova tabella, nuova colonna nullable/con default, nuovo indice, nuovo enum value | **automatico** |
| `renamed_from = "vecchio"` dichiarato | automatico (rename) |
| Drop colonna/tabella, cambio tipo, `NOT NULL` senza default, restringimento | **bloccato**: richiede migrazione SQL esplicita in `migrations/` del modulo + flag `NOCT_ALLOW_DESTRUCTIVE` |
| Checksum diverso da quello registrato | avvio rifiutato (drift) |
| Modulo disattivato | nessun drop: tabelle e dati restano; rotte non montate |

Vincoli di dipendenza: i moduli possono referenziare tabelle del core o di moduli in
`requires`; **il core non referenzia mai tabelle di moduli**. Ordine di migrazione =
ordinamento topologico del grafo `requires`.

Query layer: con schema modulare le macro `sqlx::query!` (verifica a compile-time)
sono scomode (schema dipende dai moduli attivi). Si usa `sqlx::query_as` runtime +
test d'integrazione con tutti i moduli attivi (testcontainers). Strumenti a
disposizione: `noct-ctl validate` (CI: manifest, grafo dipendenze, conflitti di
path/tabella), `noct-ctl migrate plan|apply`, `noct-ctl openapi`.

## 3. Core vs moduli

Criterio: è core ciò senza cui nessun altro componente funziona o ciò che definisce
l'entità centrale del prodotto (utente, blog, post). Il resto è modulo.

| Core (sempre attivo) | Modulo (attivabile) |
|---|---|
| `config`, `module-registry`, motore migrazioni, `core` (errori, tipi, `Patch<T>`) | `comments` (+ moderazione commenti) |
| `db`, `storage` (S3/locale), `cache`/rate limit, `bus` (NATS), `mail` (trasporto), `markdown` | `reports` (segnalazioni contenuti) |
| `auth`: utenti, password, JWT/sessioni, API token, ruoli piattaforma | `auth-mfa` (TOTP, email OTP), `auth-sso` (per provider) |
| `authz`: ruoli per blog (Autore/Co-Autore/Revisore/Mediatore) | `newsletter` (+ digest) |
| `audit` (scrittura transazionale) | `discovery` (feed portale, trending, ricerca, interessi) |
| `platform_config`, `health`, admin di base | `taxonomy` (tag, categorie) |
| `blogs` (CRUD, membri, inviti, branding, quote) | `publications` (serie), `bibliography`, `notes`, `fragments` |
| `posts` (CRUD, stati, permalink, media) | `pages` (pagine statiche), `link-preview` |
| | `gdpr` (export/cancellazione), `custom-domains` |
| | `image-moderation` (client verso `moderation/`) |
| | `ops-backup`, `audit-maintenance` (job periodici) |
| | futuri: `federation-atproto`, `federation-activitypub`, `likes` |

Installazione `solo`: core + pochi moduli, senza discovery/newsletter/sso. Portale
`notturni.eu`: tutti i moduli. Da validare con te: `blogs`/`posts` come core
(proposta) e `taxonomy` come modulo.

## 4. Layout repository

```
legacy/backend/            # Python, riferimento, rimosso a fine rewrite
crates/                    # core: noct-config, noct-core, noct-db, noct-migrate,
                           #   noct-registry, noct-codegen (build.rs helper),
                           #   noct-storage, noct-cache, noct-bus, noct-mail,
                           #   noct-markdown, noct-auth, noct-authz, noct-audit,
                           #   noct-blogs, noct-posts
modules/<nome>/            # module.toml, api.toml, db.toml, migrations/, src/, tests/
bins/noct-api, bins/noct-worker (subcommand: jobs dei moduli attivi), bins/noct-ctl
```

Un solo binario API e un solo binario worker (stessa immagine, ruoli diversi):
Deployment K8s separati, nessun requisito su arch (x86_64/ARM64, `rustls`).

## 5. Storage

Trait `Storage` (`put/get/delete/presign/public_url`), implementazioni `S3`
(`aws-sdk-s3`, endpoint custom da env, path-style, credenziali esplicite o default
chain, `NOCT_S3_REGION` opzionale) e `LocalFs` (servito dall'API via `ServeDir`);
selezione con `NOCT_STORAGE_BACKEND`. Come prima, Garage dietro il proxy dei
prefissi pubblici.

## 6. RabbitMQ vs NATS

Uso attuale: 3 code di lavoro a basso volume (OTP email, backup post, newsletter) +
job periodici in loop. Nuove esigenze con l'architettura a moduli: **eventi tra
moduli** (`post.published` → newsletter/discovery/cache frontend) e, in futuro,
consegna federata con retry (AT Protocol/ActivityPub).

| Criterio | RabbitMQ | NATS JetStream |
|---|---|---|
| Footprint | Erlang, centinaia di MB; nel compose c'è già un workaround per il cookie Erlang su Podman rootless | binario singolo, decine di MB |
| Client Rust | `lapin` (community) | `async-nats` (ufficiale) |
| Work queue, ack, retry con backoff | sì | sì (pull consumer, `max_deliver`, `nak` con delay) |
| DLQ | nativa | via advisory `max_deliver` + republish (da implementare) |
| Pub/sub tra moduli | exchange/binding | subject gerarchici (`post.published`) — naturale per i moduli |
| Cluster / K8s | operator, più pesante | cluster semplice, adatto da single node a HA |
| Extra | — | KV e Object store (candidati a sostituire Redis in seguito) |
| Licenza | MPL-2.0 (Broadcom) | Apache-2.0 (CNCF) |

**Raccomandazione: sì, NATS JetStream**, dietro i trait `EventBus` e `JobQueue`
(il codice dei moduli non vede mai il client). Punti da tenere presenti:
* **Outbox transazionale** in Postgres (tabella `noct_outbox` scritta nella stessa transazione della scrittura di dominio, relay verso NATS): evita eventi persi/duplicati.
* I job periodici (`backup`, `audit-maintenance`) restano scheduler interno del worker, non messaggi.
* Verificare sulla versione scelta: scheduled/delayed delivery e TTL per-key del KV.
* Alternativa per `solo`: implementazione del trait basata su Postgres (`SKIP LOCKED` + `LISTEN/NOTIFY`), senza broker. Non nel primo giro; il trait la rende possibile.
* Redis: si tiene dietro il trait `Cache` (rate limit, lock); valutare in Fase 7 NATS KV o implementazione in-memory per `solo`.

## 7. Fasi

| # | Fase | Output / DoD |
|---|---|---|
| 0 | Decisioni | ✅ prese (§9) |
| 1 | **Spike verticale** ✅ (§11) | `backend/` → `legacy/backend/`, workspace, parser+validator manifest, motore migrazioni (additivo), codegen rotte→axum+OpenAPI, **un modulo reale piccolo** (`pages` o `taxonomy`) end-to-end con DB vero. **Go/no-go** sull'approccio dichiarativo |
| 2 | Core infrastruttura ✅ (§12) | storage, cache, bus (NATS + outbox), mail, markdown, config; health; `noct-ctl` |
| 3 | Core auth ✅ (§13) | utenti, sessioni, JWT, API token, ruoli, audit, bootstrap super admin |
| 4 | Core contenuti ✅ (§14) | `blogs`, `posts`, media, permalink, punti di estensione per i moduli |
| 5 | Moduli, ondata 1 | divisa in tre blocchi, uno per sessione (§15): **5a** ✅ taxonomy, publications, pages, link_preview · **5b** ✅ notes, fragments, comments · **5c** ✅ discovery (feed, ricerca, directory, follow, panoramica, letture, menzioni), interests, viste «tutti i miei blog» · **5d** profilo utente (vedi §15) |
| 6 | Moduli, ondata 2 | divisa in tre blocchi (§16): **6a** ✅ auth_mfa, auth_sso · **6b** newsletter, reports, custom_domains · **6c** gdpr, image_moderation |
| 7 | Operazioni | worker, ops-backup, audit-maintenance, Dockerfile (cargo-chef, multi-arch), compose, k8s, CI (fmt, clippy, test, deny, audit, CodeQL, SBOM), documentazione |
| 8 | Chiusura | rimozione `legacy/`, benchmark, valutazione `moderation` in Rust (ONNX/`ort`) |

Ogni fase = un blocco/sessione. La Fase 1 è deliberatamente piccola e decide se
l'investimento nel DSL è giustificato prima di scrivere il resto.

## 8. Moderazione immagini

Resta il servizio Python (microservizio già isolato, fail open). Rivalutare in Fase 8
la conversione del modello in ONNX con `ort`; `candle` è più rischioso per parità.

## 9. Decisioni prese

| # | Tema | Decisione |
|---|---|---|
| 1 | API | Compatibilità con il frontend attuale (`/api/v1`, `API.md` come baseline). Durante il porting si tiene `todo/API_CHANGES.md` con le modifiche *suggerite* (prestazioni, coerenza, N+1, paginazione) da approvare caso per caso: nessuna rottura silenziosa |
| 2 | Core | Come da §3, con **utenti** esplicitamente nel core: entità utente, profilo, visibilità, sessioni, cambio email/password, ruoli piattaforma. Modulo `interests` e `gdpr` restano separati |
| 3 | Toggle moduli | Sia all'avvio sia **a caldo** (§10) |
| 4 | Broker | NATS JetStream, unico broker, dietro `EventBus`/`JobQueue` |
| 5 | Legacy | `backend/` → `legacy/backend/` come primo passo della Fase 1 (nello stesso commit della creazione del workspace, per non lasciare compose/k8s a metà) |

## 10. Attivazione a caldo dei moduli

Stato desiderato in tabella core `noct_modules(name, enabled, enabled_at, enabled_by)`;
all'avvio `NOCT_MODULES` fornisce il valore iniziale (seed), poi vale il DB.

Flusso di attivazione (admin → `POST /api/v1/admin/modules/{name}/enable`):
1. verifica `requires` (moduli e versioni) e che il modulo sia compilato nel binario;
2. migrazione dello schema del modulo sotto advisory lock (solo additiva, come §2);
3. registrazione di sottoscrizioni evento e consumer job JetStream;
4. pubblicazione dell'evento `noct.modules.changed` su NATS;
5. ogni replica dell'API ricostruisce il router dei moduli attivi e lo scambia
   atomicamente (`ArcSwap<Router>` dietro un servizio `tower` unico): le richieste
   in corso terminano sul router vecchio, nessun downtime.

Disattivazione: stesso flusso al contrario (rotte smontate → 404/`module_disabled`,
consumer fermati con drain, **dati e tabelle intatti**). Si rifiuta se un altro
modulo attivo ne dipende. Il core non dipende mai da un modulo, quindi è sempre sicuro.

Punti di attenzione: audit di ogni toggle; solo Super Admin; la route di
amministrazione dei moduli è core e non disattivabile; migrazioni distruttive mai
a caldo; in cluster l'attivazione è completata solo quando tutte le repliche
confermano (ack su subject dedicato) — altrimenti stato `pending` visibile in admin.

## 11. Fase 1 — esito dello spike (go/no-go)

**Esito: GO.** Il sistema dichiarativo regge su un caso reale (categorie per-blog, con
contratto della v1 invariato) e su un modulo di riferimento (`sandbox`) che esercita
tutte le funzioni: CRUD generato, enum Postgres, PATCH tri-stato, default del DB,
rate limit, configurazione da env, handler `custom`, policy per ruolo di piattaforma.

| Misura | Python (v1) | Dichiarativo |
|---|---|---|
| Categorie: endpoint + dominio + modello | 143 + 23 + 23 righe, più la migrazione Alembic | `api.toml` 66 + `db.toml` 10 + `module.toml` 7 righe, **0 righe di Rust**; migrazione SQL generata (14 righe) |
| Rebuild dopo una modifica di `api.toml` | n/a | ~2 s |
| Validazione dei manifest | a runtime, dai test | a build-time e in CI (`noct-ctl validate`), errori raccolti tutti insieme |

Test: 45 (manifest 15, migrazioni 12 su Postgres reale, codegen 6, API end-to-end 10 su
Postgres reale, runtime 2). `clippy -D warnings` e `fmt` puliti. Verifica dal vivo con il
binario `noct-api` e `curl` (creazione, duplicato 409, slug non valido 400, lista,
rate limit 429 con `Retry-After`, PATCH `{"body": null}`).

### Cosa è stato realizzato

| Crate | Ruolo |
|---|---|
| `noct-manifest` | parsing, normalizzazione e validazione di `module/api/db.toml`; grafo dipendenze; configurazione; OpenAPI 3.1 |
| `noct-migrate` | diff schema → SQL, file versionati con checksum, applicazione con advisory lock, `verify_live` |
| `noct-codegen` | `build.rs`: DTO + validazione + handler/router axum + sorgenti incorporati |
| `noct-core` | `ApiError`, `Config`, `AppState`, autorizzazione (`enforce`), rate limit, `Patch<T>`, `Registry` |
| `noct-users`, `noct-blogs` | moduli **core** (solo tabelle: utenti, blog, membership) |
| `modules/taxonomy` | modulo opzionale reale (categorie) |
| `modules/sandbox` | modulo di riferimento/banco di prova (non attivo di default) |
| `noct-bundle` | elenco dei moduli compilati (feature Cargo) |
| `bins/noct-api`, `bins/noct-ctl` | server (migra all'avvio, `NOCT_AUTO_MIGRATE=false` per saltare) e CLI |

### Scostamenti dal piano

* `noct-config` non è un crate a parte: la configurazione di piattaforma sta in `noct-core` (`Config`); quella dei moduli è dichiarata nei loro `module.toml`.
* Migrazioni: non "diff a ogni avvio" ma **file SQL generati e committati** + `schema.lock.json` (istantanea), come da §2. Il drift fra `db.toml` e lock fa fallire `noct-ctl validate` e l'avvio.
* Query: runtime (`query_as`/`QueryBuilder`), nessuna macro `query!`, come previsto.

### Limiti noti / da fare nelle fasi successive

* **Autenticazione provvisoria**: `NOCT_DEV_AUTH=1` accetta `Authorization: Bearer dev:<uuid-utente>`; JWT, sessioni e API token arrivano con il crate auth (Fase 3). Un solo punto da sostituire: `noct_core::authenticate`.
* **Attivazione a caldo** (§10) non ancora implementata: oggi i moduli si scelgono all'avvio (`NOCT_MODULES`). Il `Registry` espone già `resolve_enabled`/`router`/`migrate` per costruirla.
* **Rate limit in memoria** (un solo processo); applicato prima della policy (un anonimo può esaurire il proprio bucket IP anche su rotte protette). Versione condivisa in Fase 2 (`noct-cache`).
* Eventi e job (`[events]`, `[jobs]`) sono validati ma non ancora eseguiti (Fase 2, NATS).
* Migrazioni: solo cambiamenti additivi automatici (tabelle, enum e valori, colonne nullable/con default, indici, unique, rename con `renamed_from`); il resto è **bloccato** con motivazione e richiede SQL a mano + `noct-ctl migrate lock`.
* `ON CONFLICT`/upsert, filtri dinamici (feed), join e paginazione non sono esprimibili nei `kind` CRUD: andranno come handler `custom`.
* Non ancora: CORS, tracing HTTP, `/health` con dettagli, CI Rust, Dockerfile/compose/k8s per il nuovo binario (Fase 7).

### Guida rapida

```bash
# Postgres di prova (qualsiasi 16+); i test lo usano con un database per test
export NOCT_TEST_DATABASE_URL=postgres://postgres@127.0.0.1:5432/postgres
cargo test --workspace --all-features
cargo clippy --workspace --all-features --all-targets -- -D warnings

# ciclo di sviluppo di un modulo
$EDITOR modules/taxonomy/db.toml modules/taxonomy/api.toml
cargo run -p noct-ctl -- migrate plan                # cosa cambierebbe
cargo run -p noct-ctl -- migrate generate            # scrive migrations/NNNN_*.sql + schema.lock.json (da rivedere e committare)
cargo run -p noct-ctl -- validate                    # manifest + drift (per la CI)
NOCT_DATABASE_URL=postgres://... cargo run -p noct-ctl -- migrate dev   # genera e applica da disco

# avvio
NOCT_DATABASE_URL=postgres://... NOCT_MODULES=taxonomy NOCT_DEV_AUTH=1 cargo run -p noct-api
curl localhost:8000/api/v1/openapi.json
```

Nuovo modulo: copiare `modules/sandbox` (o `taxonomy` se non servono handler), rinominare,
aggiungerlo a `Cargo.toml` (workspace + `noct-bundle`) e a `noct-bundle::all()`.

## 12. Fase 2 — infrastruttura del core (completata)

Ogni servizio sta dietro un trait, ha un'implementazione di produzione e una in memoria/locale,
e un test di *contratto* eseguito identico su entrambe (cambiare backend non cambia il comportamento).

| Crate | Trait | Backend | Note |
|---|---|---|---|
| `noct-storage` | `Storage` | S3-compatibile (`aws-sdk-s3`: endpoint sempre configurabile, path-style, checksum `when_required`, credenziali esplicite o catena di default, `LocationConstraint` fuori da us-east-1) · filesystem locale | chiavi validate allo stesso modo (no `..`, `//`, assolute); URL pubblici e presigned; il backend locale è servito dall'API su `/storage/<bucket>` **solo per avatars e content** (audit resta privato) |
| `noct-cache` | `Cache` | Redis (script Lua atomici, `ConnectionManager`) · memoria | contatori a finestra (rate limit), chiavi con TTL, lock con token (rilascio sicuro); il rate limit è **fail open** se la cache è giù |
| `noct-bus` | `Bus` | NATS JetStream · memoria | eventi (fan-out, `deliver new`) e job (work queue); retry con backoff, dead letter, deduplicazione per id; `max_deliver + 1` così un crash all'ultimo tentativo finisce comunque in DLQ |
| `noct-events` (modulo core) | — | tabella `noct_outbox` | outbox **transazionale**: `outbox::enqueue(&mut tx, …)`; `Relay` con `LISTEN/NOTIFY` + polling, `SKIP LOCKED`, purge oraria |
| `noct-mail` | `Mailer` | SMTP (`lettre`, STARTTLS o chiaro) · memoria · "non configurato" (errore esplicito) | verificato con un server SMTP finto su socket reale |
| `noct-markdown` | — | `pulldown-cmark` + `ammonia` | stessa whitelist della v1 (tag, attributi, schemi `http/https/mailto`, `rel="noopener noreferrer"`) |

### Eventi e job dichiarativi

* `module.toml`: `[events] publishes/subscribes` e `[jobs.<nome>]` (subject, `max_deliver`, `backoff`) ora **vengono eseguiti**. Il codegen collega ogni voce a una funzione del modulo: job → `jobs::<nome>`, evento → `events::on_<subject con _>` (firma `async fn(AppState, Message) -> Result<(), HandlerError>`; `Retry` riprova, `Permanent` va in dead letter).
* `api.toml`: `emits = "<subject>"` su `create`/`update`/`delete` pubblica l'evento nella **stessa transazione** della scrittura (busta `{module, blog_id, actor_id, data}`); se la scrittura fallisce o va in rollback (es. 409) non resta nessun evento. Il validatore richiede `emits` ⊆ `publishes` e `requires = ["events"]`.
* Esecuzione: `noct-api` fa girare relay e consumer nel proprio processo se non c'è NATS (`NOCT_WORKER_INLINE`, default); con NATS si avviano uno o più `noct-worker`. Verificato dal vivo: API senza worker inline + processo `noct-worker` separato → evento via NATS → handler → Redis.

### Configurazione (nomi compatibili con lo stack attuale)

| Variabile | Default | Effetto |
|---|---|---|
| `NOCT_DATABASE_URL` (o `NOCT_POSTGRES_*`) | — | database |
| `NOCT_STORAGE_BACKEND` | `localstorage` | `s3` \| `localstorage` |
| `NOCT_S3_ENDPOINT_URL`, `_REGION`, `_ACCESS_KEY_ID`, `_SECRET_ACCESS_KEY`, `_PUBLIC_URL` | — | S3-compatible; senza chiavi esplicite si usa la catena di default AWS |
| `NOCT_S3_BUCKET_AVATARS` / `_CONTENT` / `_AUDIT` | `avatars` / `notturni-content` / `notturni-audit` | creati all'avvio se mancano |
| `NOCT_LOCAL_STORAGE_BASE_PATH`, `NOCT_LOCAL_STORAGE_PUBLIC_URL` | `./data/storage`, `http://localhost:8000/storage` | backend locale |
| `NOCT_REDIS_URL` (o `NOCT_REDIS_HOST/PORT/DB`) | assente = cache in memoria | Redis |
| `NOCT_NATS_URL`, `NOCT_NATS_PREFIX` | assente = bus in memoria; `noct` | NATS JetStream |
| `NOCT_SMTP_HOST/PORT/USER/PASSWORD/USE_TLS/FROM_EMAIL` | assente = invii in errore | SMTP |
| `NOCT_WORKER_INLINE` | `true` senza NATS, `false` con NATS | relay + consumer nel processo API |
| `NOCT_AUTO_MIGRATE` | `true` | migrazioni dei moduli attivi all'avvio |

Endpoint: `GET /api/v1/health` (liveness + DB), `GET /api/v1/health/ready` (database, storage, cache, bus; 503 se uno è giù, dettagli solo nei log).
`noct-ctl`: `outbox` (code in attesa/pubblicate, riga bloccata), `dead-letters`.

### Test (76 in totale, verdi su 3 esecuzioni consecutive)

Richiedono i servizi reali, indicati da variabili d'ambiente: `NOCT_TEST_DATABASE_URL` (Postgres),
`NOCT_TEST_REDIS_URL`, `NOCT_TEST_NATS_URL` (server con `-js`), `NOCT_TEST_S3_ENDPOINT` (qualsiasi S3-compatible
con credenziali `test`/`test`: Garage, moto, MinIO). Il compose del progetto ha già tutti i servizi.

### Limiti noti / prossime fasi

* Bootstrap di Garage (layout, chiave, bucket con alias globale, website access: oggi `legacy/backend/scripts/garage_bootstrap.py`) non portato: va in `noct-ctl` con il packaging (Fase 7). I bucket vengono creati via API S3, ma Garage richiede l'alias globale per le letture pubbliche.
* Backup su S3 esterno e manutenzione dell'audit (job periodici) restano alla Fase 7 (scheduler del worker).
* Il worker esegue consumer e relay ma non ancora job periodici; nessuna metrica/tracing OpenTelemetry (Fase 9).
* `Mailer` e `Storage` non sono ancora usati da un modulo di prodotto (arrivano con auth/OTP e media).
* RabbitMQ è sostituito da NATS: nessuna migrazione di messaggi (il progetto non è in produzione).

## 13. Fase 3 — autenticazione (completata)

### Regola: nessun codice `unsafe`

* `Cargo.toml` del workspace: `[workspace.lints.rust] unsafe_code = "forbid"`, ereditato da **ogni** crate (`[lints] workspace = true`). `forbid` non si aggira con `#[allow]` e vale anche per il codice generato dai manifest (è incluso nel crate che lo ospita).
* `crates/noct-core/tests/no_unsafe.rs` lo verifica: lint presente nel workspace, ogni crate lo eredita, nessun sorgente contiene la parola `unsafe` fuori da stringhe e commenti (con un test dello scanner stesso).
* Limite onesto: la regola copre **il nostro codice**. Le dipendenze (tokio, rustls/ring, aws-sdk, sqlx...) contengono `unsafe` per natura. Per questo le primitive crittografiche sono scelte in Rust sicuro quando possibile (Argon2, HMAC, SHA-2 di RustCrypto; JWT scritto in ~100 righe sicure invece di una libreria con backend nativo). In CI (Fase 7) si aggiungono `cargo geiger` (report degli `unsafe` nel grafo) e `cargo deny`/`cargo vet`.

### Crate e moduli

| Elemento | Contenuto |
|---|---|
| `noct-security` (libreria, nessun I/O) | Argon2id (hash PHC compatibili con argon2-cffi), JWT HS256 rigido (solo `HS256`, firma a tempo costante, `exp`/`type` obbligatori; **firma byte-identica a PyJWT**, verificato con un token reale), token opachi (`noct_`, `noct_sess_`), OTP derivati da HMAC, cookie con validazione anti-iniezione, validazione di username/password/email |
| `noct-audit` (modulo core) | tabella `audit_log` append-only senza FK; `record()` nella **stessa transazione** dell'azione |
| `noct-auth` (modulo core) | tabelle `user_sessions`, `api_tokens`, `password_reset_codes`; 12 rotte; job `send_password_reset`; hook di avvio per config e Super Admin |
| `noct-users` | +`hashed_password`, `credentials_changed_at`, `mfa_enabled`, `display_name`, `ui_locale`: la migrazione `0002` è stata **generata dal diff** (prima evoluzione additiva reale) |
| `noct-core` | `authenticate` reale (JWT + API token), policy `token` / `user_or_token`, `Access.token`, `RequestMeta` (IP, user-agent), hook di avvio, rotte `custom` con risposta `raw` e accesso a query/header/cookie |

### Rotte (contratto v1 mantenuto, esclusi MFA e SSO)

`POST /auth/register` · `GET /auth/username-available` · `POST /auth/login` · `POST /auth/refresh` · `POST /auth/logout` · `GET /auth/me` · `POST /auth/password/forgot` · `POST /auth/password/reset` · `POST /users/me/password` · `POST|GET /tokens` · `DELETE /tokens/{id}`.

### Decisioni di sicurezza

* **Sessione**: access JWT (15 min) nel corpo; refresh opaco in cookie `httpOnly` con `Path=/api/v1/auth` (in DB solo l'hash); **rotation** con `UPDATE … WHERE revoked_at IS NULL RETURNING` atomico (5 refresh concorrenti → uno solo riesce, provato); CSRF double-submit con confronto a tempo costante su refresh e logout.
* **Cambio e reset password**: cancellano tutte le sessioni e impostano `credentials_changed_at`, che invalida anche gli access token già emessi. *Correzione rispetto alla v1*: `iat` è in secondi interi, quindi il confronto avviene al secondo; così un login subito dopo il cambio non viene respinto (finestra residua ≤ 1 s sul token precedente, documentata nel codice).
* **OTP di reset derivato, non memorizzato**: `HMAC(chiave del server, scopo ‖ id)` a 6 cifre. Il database tiene solo id e scadenza; il job `auth.password_reset.send` porta solo l'id e rigenera il codice per spedirlo → il codice non compare mai in tabelle, outbox, NATS o dead letter (verificato nei test). Vale solo l'ultimo codice richiesto; scade in 10 minuti; monouso (`FOR UPDATE`).
* **Anti-enumerazione**: `forgot` risponde sempre 202; login con utente inesistente esegue una verifica Argon2 fittizia (stesso costo); stessa risposta per password errata e utente sconosciuto; stesso messaggio per ogni causa di reset fallito.
* **Rate limit** (cache condivisa): login 20/IP e 5/email ogni 5 min, reset 20/IP e 5/email, registrazione 30/IP ogni 10 min, cambio password 10/utente ogni 5 min, disponibilità username 30/IP al minuto.
* **IP del client**: si prende il valore di `X-Forwarded-For` scritto dall'ultimo hop fidato (`NOCT_TRUSTED_PROXIES`, default 1 = Traefik), mai il primo (falsificabile); senza proxy fidati o con meno valori del previsto si usa l'IP del peer TCP, che un middleware scrive in un header interno **scartando sempre** quello del client. *Corretto un difetto della Fase 1*, che si fidava del primo valore.
* **Argon2 fuori dal runtime** (`spawn_blocking`); password massimo 1024 byte (anti-DoS dell'hashing).
* **Audit**: `auth.register`, `auth.login`, `auth.login_failed` (in transazione propria, così resta anche se l'azione fallisce; mai la password), `auth.password_reset`, `user.password_change`, `api_token.created/revoked`, `auth.super_admin_bootstrap`. Attore: utente, token core/utente, sistema, anonimo.
* **Super Admin** da `NOCT_SUPER_ADMIN_USERNAME/EMAIL/PASSWORD`: idempotente, validato (username, email, password policy) e **mai fatale** (un conflitto di provisioning non blocca l'avvio); una configurazione dei cookie o delle modalità sbagliata invece **ferma** l'avvio con un messaggio chiaro.
* **Modalità** `NOCT_REGISTRATION_MODE` (open | invite | closed) e `NOCT_DEPLOYMENT_MODE` (platform | solo: il primo utente è Super Admin e poi la registrazione si chiude, con lock advisory contro le corse — provato con 6 registrazioni concorrenti).
* **API token**: hash sha256, prefisso visibile, mostrati una volta; `core` (nessun utente) e `user` rigorosamente separati (un token non vede né revoca quelli dell'altro tipo); ultimo uso aggiornato al massimo una volta al minuto; scaduti, revocati o con proprietario disattivato → 401. `noct-ctl token create --name …` emette token core (serve solo `NOCT_DATABASE_URL`).

### Configurazione nuova

| Variabile | Default | |
|---|---|---|
| `NOCT_JWT_SECRET` | **obbligatoria**, ≥ 32 byte | firma i JWT e deriva gli OTP; l'avvio rifiuta valori mancanti o corti |
| `NOCT_JWT_ACCESS_TOKEN_TTL_MINUTES` / `NOCT_JWT_REFRESH_TOKEN_TTL_DAYS` | 15 / 30 | |
| `NOCT_TRUSTED_PROXIES` | 1 | hop fidati davanti all'API |
| `NOCT_SESSION_COOKIE_SECURE` / `_SAMESITE` / `_DOMAIN` | true / lax / — | |
| `NOCT_REGISTRATION_MODE`, `NOCT_DEPLOYMENT_MODE` | open, platform | |
| `NOCT_SUPER_ADMIN_USERNAME/EMAIL/PASSWORD` | — | opzionali, insieme |

Rimosso `NOCT_DEV_AUTH` (l'autenticazione di sviluppo della Fase 1).

### Test e verifica

140 test in tutto, verdi su 3 esecuzioni consecutive; `clippy -D warnings` e `fmt` puliti. 30 sui primitivi (`noct-security`), 23 end-to-end sull'autenticazione (registrazione, sessioni, rotation e riuso, CSRF, cambio/reset password, token, IP falsificato, solo-mode concorrente, Super Admin, configurazioni errate) più quelli già esistenti. Verifica dal vivo con `noct-api` e `curl`: registrazione, login con cookie, refresh con cookie jar e CSRF, 403 senza CSRF, reset via email SMTP reale (codice letto dal messaggio), token core via `noct-ctl`.

### Limiti noti / da fare

* **MFA e SSO** sono moduli opzionali (Fase 6): serve un punto d'innesto nel login (challenge dopo la password) da progettare con quel modulo; per ora `mfa_enabled` è solo un flag riservato.
* Lettura dell'audit, ruoli e sospensione utenti, rotte admin: Fase 7 (admin). Nessun trigger che impedisca UPDATE/DELETE su `audit_log` (il manifest non esprime trigger): la retention sarà un job dedicato.
* Il riuso di un refresh token già ruotato rifiuta la richiesta ma **non** revoca le altre sessioni (come la v1: una doppia richiesta legittima da due schede non deve disconnettere l'utente). Da rivalutare con il frontend.
* La coerenza `owner_type`/`user_id` dei token è garantita dal codice, non da un CHECK del database.
* Le email di sistema sono testo semplice in italiano; i template e la localizzazione arrivano con il modulo newsletter/mail.

## 14. Fase 4 — contenuti del core (completata)

### Perimetro

Core: **blog** (dettaglio, modifica, collaboratori, inviti, pausa, cancellazione con tolleranza, trasferimento, configurazione di presentazione), **post** (bozza → revisione → pubblicazione/pianificazione, traduzioni, tag e hashtag, media e link citati, permalink, ricerca nel blog, bibliografie di media e link), **immagini** (upload per l'editor, libreria media, copertina, favicon, quota di spazio, moderazione fail open) e **impostazioni di piattaforma** (riga unica, solo lettura per ora). Cinque crate: `noct-platform`, `noct-blogs`, `noct-posts`, `noct-media`, `noct-http` (chiamate in uscita).

**Non ancora**, rimandato alle fasi indicate perché appartiene a un modulo: directory pubblica `GET /blogs`, follow/follower, panoramica, conteggio letture, `mentionable-users` (→ `discovery`, F5); categorie del post (→ `taxonomy`), note e bibliografia delle note (→ `notes`), pubblicazioni, commenti e relativi campi (F5); newsletter e `custom_domain` (F6); export ZIP del blog, cancellazione definitiva allo scadere dei 30 giorni, backup dei post su storage (F7, consumer degli eventi già pubblicati).

### Punti di estensione (`EntityExtension`)

Il core possiede le colonne essenziali di `blogs` e `posts`; ciò che è di un modulo vive in tabelle del modulo. Per mantenere un solo `PATCH /posts/{id}` e un solo `PostOut` come in v1, un modulo dichiara `[lifecycle] extensions = true` e fornisce `extensions::all()`:

* `apply(conn, ctx, input)`: legge dal corpo le chiavi di `fields()` e scrive **nella stessa transazione** del core (un errore annulla tutto: provato, nessun residuo di post, tag o eventi);
* `enrich(db, ids)`: aggiunge i campi alle risposte **in blocco** (una query per modulo per elenco, mai una per riga).

Un modulo spento non registra nulla: i suoi campi non compaiono e se arrivano nel corpo vengono ignorati (come v1 con i campi sconosciuti). Due moduli non possono dichiarare la stessa chiave (verificato all'avvio). Il modulo `sandbox` ne è l'esempio completo (default, ereditarietà alla traduzione, validazione).

### Decisioni

* **Eventi al posto delle chiamate dirette**: `posts.post.saved` (con il testo, per il backup) e `posts.post.published` (per la newsletter) sono scritti in outbox nella transazione del salvataggio. In v1 erano pubblicazioni fire-and-forget dopo il commit e un broker spento le perdeva.
* **Rotte `upload`**: attributo `upload = <byte>` in `api.toml` (alternativo a `request`); il corpo arriva grezzo in `ctx.body`, con limite per rotta e documentazione OpenAPI multipart. Il corpo JSON assente vale `{}` (il frontend invia `publish` senza corpo).
* **Upload di immagini**: oltre al `Content-Type` dichiarato si controlla l'intestazione reale del file (PNG/JPEG/WEBP/GIF): un HTML o SVG travestito da immagine è respinto. Chiavi di storage invariate (`{site}/userdata/{proprietario}/{blog}/media/…`, `favicons/{blog}/…`).
* **Quota per blog**: somma di `media_files.size_bytes` (v1 contava gli oggetti su storage, backup compresi) con `platform_config.max_blog_storage_mb`; 413 oltre.
* **Concorrenza**: creazione blog serializzata per utente (`FOR UPDATE` sull'utente: 8 richieste contemporanee → esattamente 5 blog); accettazione inviti e trasferimento con lock di riga; cancellazione idempotente anche sotto concorrenza.
* **Nome dell'autore** ricalcolato a ogni lettura (alias di membership → nome imposto dal blog → preferenza di profilo), come in v1; nuove colonne `users.first_name/last_name/avatar_object_key/post_author_name_style` (l'upload dell'avatar e il profilo arrivano con la fase utenti).
* **Manifest**: nuovi tipi di colonna `text[]` e `double precision`, nuovi tipi di campo `string[]` e `float`; migrazioni generate dal diff (users 0003, platform 0001, blogs 0002, posts 0001, media 0001).
* **Parser Markdown**: le regex di media/link sono riscritte per il motore `regex` (tempo lineare, nessun ReDoS: provato con 40 000 ripetizioni patologiche); niente lookbehind né gruppi atomici.

### Configurazione nuova

`NOCT_SITE_SLUG` (default `notturni`), `NOCT_FRONTEND_REVALIDATE_URL` + `NOCT_REVALIDATE_SECRET` (invalidazione della cache Next.js, fail open), `NOCT_MODERATION_SERVICE_URL` (servizio `moderation/`, fail open).

### Test e verifica

Nuovi: 9 su blog, 8 su post, 6 su media, 3 sulle estensioni (end-to-end con Postgres vero), più unit test su regole di dominio, upload, tag/hashtag, estrazione media/link, client HTTP e manifest. Verifica dal vivo con `noct-api` e `curl`: registrazione, creazione blog, post, pubblicazione, lettura anonima, upload multipart reale (e rifiuto di un PNG dichiarato JPEG), file servito dallo storage locale, configurazione con colore troppo saturo respinta, OpenAPI multipart.

### Limiti noti / da fare

* Gli ordinamenti non specificati in v1 sono ora deterministici (vedi `todo/API_CHANGES.md`).
* `platform_config` ha solo le colonne del core; le rotte di amministrazione (Fase 7) e le impostazioni dei moduli (SSO, commenti anonimi, newsletter di piattaforma, interessi, sigilli) arriveranno con i rispettivi moduli.
* La coerenza `slug ≥ 4 caratteri` non è un CHECK del database ma una regola applicativa (la lista dei nomi riservati è configurabile).
* La moderazione e il revalidate usano `reqwest` (rustls); `cargo geiger`/`deny` in CI (Fase 7) dovranno includerlo.

## 15. Fase 5 — moduli, ondata 1

### Divisione in blocchi

| Blocco | Moduli | Perché insieme |
|---|---|---|
| **5a** ✅ | `taxonomy`, `publications`, `pages`, `link_preview` | si agganciano a blog/post solo con `EntityExtension` o sono autonomi: nessuna modifica al core |
| **5b** ✅ | `notes`, `fragments`, `comments` | toccano il contenuto del post (marcatori nel testo, note a piè di pagina, commenti con moderazione e modalità per post) e condividono regole di sanificazione |
| **5c** ✅ | `discovery`, `interests` | letture aggregate trasversali (feed, ricerca globale, directory dei blog, follow, panoramica, letture): dipendono da tutti i campi dei moduli precedenti e sono quelle dove contano le prestazioni |

### 5a — completato

* **`taxonomy`** (già nello spike): ora estende il post. `category_id` tri-stato (assente non tocca, `null` rimuove) in create/update/traduzione, `category_id` e `category` in uscita (una query per elenco). Tabella `post_categories` con cascade: cancellare la categoria lascia il post senza. Categoria di un altro blog o non UUID → 400 e nessun post creato. La traduzione eredita la categoria dell'originale se non ne indica una.
* **`publications`**: CRUD per blog (`name` = segmento URL, risolto anche per id), indice dei capitoli (ai lettori solo i pubblicati, a chi scrive anche bozze e pianificati con `is_public=false`), ordine esplicito con `PUT …/order` sotto lock di riga (i non elencati seguono in ordine cronologico), conteggi totali/pubblicati. Estensione del post: `publication_id`, `publication {id,name,title}`, `chapter_order`; cambiare pubblicazione azzera la posizione; la traduzione resta nella pubblicazione dell'originale. Tabelle `publications` e `post_publications`.
* **`pages`**: pagine statiche dei blog (attivabili per blog con il campo `static_pages_enabled`, aggiunto al blog da un'estensione) e del sito (gestione riservata ad `amministratore`/`super_admin`; bozze visibili solo a loro). Due tabelle con lo stesso schema (`blog_pages`, `platform_pages`) invece di una con `blog_id` nullo: l'unicità di slug e lingua vale senza indici parziali, che il manifest non esprime. Traduzioni per gruppo, audit `page.created|updated|deleted`, revalidate dei tag del frontend (anche del vecchio slug se cambia). Creazione di pagine e traduzioni 403 a funzione spenta; modifica e rimozione restano possibili.
* **`link_preview`** (nome con `_`: i nomi modulo ammettono solo minuscole, cifre e `_`): `GET /link-preview?url=`, pubblico, 30 richieste/minuto per IP. Fetch lato server rigido contro SSRF: solo http/https, l'host è risolto una volta sola e **la connessione va a quell'indirizzo verificato** (`ClientBuilder::resolve`: nessun secondo DNS, quindi nessun rebinding), blocco di loopback, reti private, link-local, CGNAT, multicast, riservati, documentazione, IPv4-mapped e NAT64, nessun proxy di sistema, nessun redirect seguito, timeout 8 s, corpo fermato a `</head` o 3 MB. Cache a due livelli (Redis 6 h → tabella `link_preview_cache` deduplicata per `sha256(url)`: valida 7 giorni se con dati, 6 ore se vuota). `NOCT_LINK_PREVIEW_ALLOW_PRIVATE` (default `false`) serve solo a prove locali e test.
* **Manifest**: nessuna novità al linguaggio; migrazioni generate dal diff (taxonomy 0002, publications 0001, pages 0001, link_preview 0001).

### Test e verifica

13 test end-to-end nuovi (`bins/noct-api/tests/content_modules.rs`: categoria tri-stato/ereditata/validata, modulo spento, CRUD e visibilità delle pubblicazioni, ordine dei capitoli e tri-stato sul post, pagine di blog e di sito, flag del blog, anteprime con un server HTTP reale, cache calda/persistente/scadenza, redirect non seguiti) più 4 unit test (IP bloccati, URL ammessi, parsing Open Graph, regex lineare su input patologico). Suite completa 210 test verdi, `clippy -D warnings` e `fmt` puliti, `noct-ctl validate` senza drift (12 moduli, 80 rotte, 25 tabelle). Verifica dal vivo con `noct-api` e `curl`: categoria e pubblicazione sul post, pubblicazione, indice dei capitoli, pagina di blog, `POST /pages` negato a un utente, anteprima Open Graph di un server locale, `file://` respinto, OpenAPI con le nuove rotte.

### Limiti noti / da fare

* Le pagine di sito non hanno `DELETE` (come in v1).
* Il fetch dell'anteprima non decodifica le entità HTML (`&amp;`) nei meta tag, come in v1.
* La cache calda dell'anteprima non è invalidata alla modifica di un post: scade da sola.

### 5b — completato

* **`notes`**: libreria note del blog (`blog_notes`) e note a piè di pagina dei post (`post_notes`). Estensione di `posts`: `notes` in ingresso (assente o `null` = invariate, elenco anche vuoto = sostituisce, nella stessa transazione del salvataggio) e in uscita. Ogni nota del post si aggancia alla nota di libreria con lo stesso testo normalizzato (minuscolo, ASCII, apostrofi ignorati) o la crea; i campi bibliografici si riportano **solo alla creazione**, una nota esistente non si sovrascrive mai. Un lock advisory per blog serializza i salvataggi: sei post concorrenti con la stessa nota nuova producono **un** solo record (provato). Rotte: libreria con «citata in» e possibili duplicati (primi 30 caratteri normalizzati), crea/modifica (il testo si propaga ai post)/elimina (409 se citata)/unisci, export `.bib`, import BibTeX (parser a tempo lineare, tetto 200 000 caratteri, URL non http(s) scartati, voci già presenti saltate), bibliografia pubblica del blog e vista personale `/users/me/bibliography`.
* **`fragments`**: frammenti di testo salvati dai lettori. Solo su un post pubblicato e visibile (404 altrimenti), massimo 15 % del testo, salvataggio idempotente anche sotto concorrenza (`ON CONFLICT DO NOTHING` su `(utente, post, hash del testo)`: l'hash evita il limite di un indice btree su testi lunghi). Raccolta personale, visibilità privata/pubblica modificabile, cancellazione.
* **`comments`**: commenti con risposte. Registrati: pubblicati subito; anonimi (solo in modalità `everyone`): captcha Turnstile verificato lato server (fail closed) e sempre in moderazione. Modalità per blog (`comments_mode`, `comments_auto_close_days`) e override per post (tri-stato), `effective_comments_mode` calcolato; tutti campi aggiunti da estensioni di `blogs` e `posts`. Moderazione (proprietario, mediatore, moderatori/amministratori di piattaforma): coda per post e per blog, approva/rifiuta, segnalazione ai moderatori di piattaforma con nota, blocco dell'autore (utente o hash sha256 dell'email, mai l'email in chiaro; un solo blocco per autore anche con moderatori concorrenti) e sblocco; ogni azione è nell'audit. `NOCT_TURNSTILE_SECRET_KEY` (e `NOCT_TURNSTILE_VERIFY_URL`, per i test): senza chiave i commenti aperti a tutti non si possono attivare.
* **Core**: `ExtCtx` ora porta `state` (configurazione dei moduli e servizi), così un'estensione può validare contro la configurazione, come fa `comments` con il captcha.
* **Manifest**: nessuna novità al linguaggio; migrazioni generate dal diff (notes, fragments, comments 0001).

Test e verifica: 12 test end-to-end (`bins/noct-api/tests/engagement_modules.rs`: note validate/agganciate/sostituite, concorrenza sulla libreria, permessi e CRUD della libreria con merge e BibTeX, bibliografie, frammenti con doppio click in parallelo, commenti registrati/anonimi con un server Turnstile finto, modalità per post e chiusura automatica, interruttore di piattaforma, moderazione/segnalazione/blocco/sblocco, visibilità del post, moduli spenti) e 8 unit test (regole delle note, BibTeX, modalità effettiva, hash e validazione email, quota dei frammenti). Suite completa 232 test verdi, `clippy -D warnings` e `fmt` puliti, `noct-ctl validate` senza drift (15 moduli, 104 rotte, 33 tabelle). Verifica dal vivo con `noct-api` e `curl`: nota nel post, commento di un registrato, anonimo respinto in modalità `members`, bibliografia pubblica, export BibTeX, modalità `everyone` rifiutata senza captcha, OpenAPI con le nuove rotte.

Limiti noti / da fare: le rotte di amministrazione dei commenti (`/admin/comments`, impostazione `anonymous_comments_allowed`) arrivano con le operazioni (F7), come gli altri endpoint `/admin/*`; fino ad allora l'interruttore di piattaforma è la tabella `comments_platform_settings` (assente = consentiti). Il captcha non è verificabile end-to-end senza chiavi Cloudflare reali: i test usano un server finto. Le viste aggregate «tutti i miei blog» di post, media e link (`/users/me/posts|media|links`) appartengono a `discovery` (5c); quelle di pubblicazioni e bibliografia sono già nei rispettivi moduli (pubblicazioni: da aggiungere in 5c insieme alle altre, perché condividono la stessa query dei blog dell'utente).

### 5c — completato

* **`discovery`**: feed del portale (`/feed/posts` con `locale`, `tag`, `category`, `following`, paginazione; `/feed/locales`; `/feed/trending`), ricerca globale `/search/posts`, directory `GET /blogs` (solo blog pubblici e indicizzabili, ordinamento `active|new|followers`, conteggi di post e follower, `owner_id` mai esposto), follow di blog e di utenti con relativi elenchi e `/users/me/follow-stats`, panoramica del blog (conteggi, letture degli ultimi 30 giorni, spazio, limite), `mentionable-users` e conteggio letture (`POST /posts/{id}/read`: nessun cookie né dato del lettore, un solo `+1` per IP e post ogni 6 ore tramite il rate limiter, accettato ma non contato oltre). Tabelle `user_follows`, `blog_follows`, `post_reads_daily`.
* **`interests`**: `GET /interests` da una riga unica `interests_settings` (modifica con le rotte admin, F7).
* **Viste «tutti i miei blog»** nei moduli proprietari invece che in un modulo trasversale: `/users/me/posts` e `/users/me/links` in `posts`, `/users/me/media` in `media`, `/users/me/publications` in `publications` (proprietario o collaboratore, esclusi i blog in cancellazione). `/users/me/bibliography` e `/users/me/fragments` erano già in `notes` e `fragments`.
* **Core**: `AppState` conosce i moduli attivi (`module_enabled`), così un modulo che legge dati di uno opzionale lo fa solo se è attivo (il filtro `category` del feed e i conteggi dei commenti della panoramica); nuovo tipo di colonna `date` nel manifest.
* **Privacy**: il feed «seguiti» include i post di un utente seguito solo se sono firmati con il suo username, mai quelli con un alias (in v1 comparivano e rivelavano di chi erano a chi seguiva la persona).

Test e verifica: 10 test end-to-end (`bins/noct-api/tests/discovery_modules.rs`: feed e filtri, lingue e tendenze, «seguiti» e alias, ricerca, directory, follow idempotenti e statistiche, smettere di seguire un blog diventato privato, letture e panoramica, panoramica senza il modulo commenti, menzioni, viste dell'autore, interessi). Suite completa 242 test verdi, `clippy -D warnings` e `fmt` puliti, `noct-ctl validate` senza drift (17 moduli, 125 rotte, 37 tabelle). Verifica dal vivo con `noct-api` e `curl`: feed, lingue, tendenze, ricerca, directory, follow, lettura, panoramica, statistiche, `/users/me/posts`.

Limiti noti / da fare: l'**endpoint del profilo utente** non è ancora stato portato e diventa il blocco **5d**: `GET /users` (directory utenti con filtro per interesse), `GET/PATCH /users/me` e `/users/{username}`, avatar, link social, cambio email in tre passi, dominio verificato, `/users/{username}/blogs|posts|comments`, esportazione dati e cancellazione dell'account (queste ultime due toccano il modulo `gdpr`, F6). La scelta degli interessi sul profilo e il filtro della directory utenti arrivano lì. Il conteggio dello spazio nella panoramica è la somma di `media_files.size_bytes` (come la quota), non il conteggio degli oggetti su storage.

## 16. Fase 6 — moduli, ondata 2

### Divisione in blocchi

| Blocco | Moduli | Perché insieme |
|---|---|---|
| **6a** ✅ | `auth_mfa`, `auth_sso` | entrambi toccano il login: un secondo fattore e un provider esterno che deve rispettarlo |
| **6b** | `newsletter`, `reports`, `custom_domains` | consumano gli eventi e i campi già pubblicati dal core (`posts.post.published`, blog) e hanno rotte di amministrazione proprie |
| **6c** | `gdpr`, `image_moderation` | trattano dati personali e media di tutti gli altri moduli: vanno scritti per ultimi, quando c'è tutto da esportare e cancellare |

### 6a — completato

* **Punto di aggancio del login (`LoginGate`)**: dopo la verifica delle credenziali (password o provider esterno) ogni gate dei moduli attivi può fermare l'accesso e rispondere con una sfida; superata la sfida, `completed()` permette a un altro gate di concludere ciò che aspettava. Dichiarato con `[lifecycle] login_gates = true` e `gates::all()`, come le estensioni di blog/post. Il JWT supporta ora claim aggiuntivi (`encode_with`, i claim standard non si sovrascrivono); `noct_auth::session` espone emissione e cookie di sessione ai moduli che completano un login.
* **`auth_mfa`**: TOTP (RFC 6238 scritto qui con HMAC-SHA1, verificato sui vettori dell'RFC; tolleranza di un passo; **lo stesso codice non si riusa**: `totp_last_step` aggiornato in modo atomico) con QR come SVG inline generato nel backend; codice via email *derivato* (come il reset della password: mai memorizzato, mai nell'outbox né sul bus), monouso, vale solo l'ultimo richiesto. Il login risponde `mfa_required/method/challenge`; `POST /auth/mfa/verify` completa ed emette la sessione. Il segreto di una nuova configurazione resta «in attesa» finché non si conferma: rigenerarlo non interrompe l'accesso con quello in uso (in v1 sì). Limiti di tentativi per IP e per account; la sfida è legata al metodo attuale (cambiato il metodo, non vale più). Audit `auth.mfa_enabled|mfa_disabled|login`.
* **`auth_sso`** (Google, Microsoft, GitHub, LinkedIn): `GET /auth/sso/{provider}/login` → redirect con **state e PKCE (S256) legati al browser da un cookie httpOnly**; il callback scambia il codice direttamente col provider e legge lo userinfo. Account linking per email come in v1, ma **solo se il provider garantisce l'email** (`email_verified`; per GitHub l'email *primaria* verificata, mai quella pubblica del profilo); un account con MFA attiva si collega **dopo** la verifica del secondo fattore (il collegamento in sospeso viaggia nella sfida firmata e si completa con `completed()`). Un nuovo account si crea solo se le registrazioni sono aperte (in v1 l'SSO le aggirava). Errori sempre come redirect `/login?sso_error=1`, senza dettagli nell'URL. Tabelle `sso_identities`, `sso_settings` (provider consentiti; vuoto = tutti i configurati).
* **Configurazione nuova**: `NOCT_OAUTH_{GOOGLE,MICROSOFT,GITHUB,LINKEDIN}_CLIENT_{ID,SECRET}`, `NOCT_OAUTH_REDIRECT_BASE_URL`, `NOCT_FRONTEND_URL`; `NOCT_OAUTH_ENDPOINT_BASE` solo per prove con un provider finto.
* **Manifest**: nessuna novità al linguaggio; migrazioni `auth_mfa` e `auth_sso` 0001 generate dal diff.

Test e verifica: 14 test end-to-end (`bins/noct-api/tests/auth_modules.rs`: configurazione/conferma/login TOTP con replay respinto, sfide tipizzate/scadute/legate al metodo/utente disattivato, rigenerazione del segreto, limite di tentativi, MFA via email con codice derivato e monouso, disattivazione e audit, redirect SSO con state e PKCE, creazione e riconoscimento dell'utente con username adattato, callback forgiati o rotti, collegamento solo con email verificata, collegamento con MFA completato dopo il secondo fattore, GitHub con email primaria, registrazioni chiuse, moduli spenti — con un provider OAuth finto in un server TCP) e 10 unit test (RFC 6238, base32, finestra TOTP, `otpauth://`, endpoint e profili dei provider, PKCE dell'RFC 7636, username). Suite completa verde, `clippy -D warnings` e `fmt` puliti.

Limiti noti / da fare: l'impostazione `sso_providers` della piattaforma e le rotte di amministrazione arrivano con le operazioni (F7: oggi si scrive su `sso_settings`); il segreto TOTP è in chiaro nel database come in v1 (cifratura a riposo: hardening, F9); `POST /auth/mfa/disable` non chiede ancora un codice (cambierebbe il contratto, vedi `todo/API_CHANGES.md`). L'SSO non è verificabile end-to-end contro i provider reali senza credenziali: i test usano un provider finto.
