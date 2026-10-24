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
| 2 | Core infrastruttura | storage, cache, bus (NATS + outbox), mail, markdown, config; health; `noct-ctl` |
| 3 | Core auth | utenti, sessioni, JWT, API token, ruoli, audit, bootstrap super admin |
| 4 | Core contenuti | `blogs`, `posts`, media, permalink |
| 5 | Moduli, ondata 1 | comments, taxonomy, pages, discovery, publications, notes, fragments, link-preview |
| 6 | Moduli, ondata 2 | auth-mfa, auth-sso, newsletter, gdpr, reports, custom-domains, image-moderation |
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
