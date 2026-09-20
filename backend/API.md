# Notturni API

Riferimento degli endpoint esposti dal backend. La documentazione interattiva
generata da FastAPI (schema OpenAPI sempre aggiornato al codice) resta
disponibile su `/docs` e `/openapi.json`; questo file aggiunge il contesto
(uso previsto, chi lo consuma, esempi, regole di business) che l'OpenAPI da
solo non dà.

Gli endpoint qui sotto sono usati dal frontend (login, gestione
blog/post/commenti/pagine, profilo, follow — vedi `frontend/README.md`) più
l'accesso diretto all'API (motore core, script, futuri client di terze
parti). Per lo stato di avanzamento di ogni funzionalità, vedi
[ROADMAP.md](../ROADMAP.md).

## Multilingua (i18n)

Post e pagine statiche condividono lo stesso schema di traduzione: ogni riga
(un post, una pagina) ha un `locale` (codice ISO 639-1 di 2 lettere: `it`,
`en`, `de`, ...) e un `translation_group_id` — le righe con lo stesso
`translation_group_id` sono traduzioni dello stesso contenuto logico, ognuna
con il proprio slug (lo slug può essere tradotto anch'esso, es.
`il-mio-post` in italiano e `my-post` in inglese).

Non c'è un endpoint dedicato "lista lingue disponibili": il frontend è
pensato per instradare `/it/...`, `/en/...`, `/de/...` a livello di routing
(sia per notturni.eu sia per i singoli blog) e passare il locale estratto dal
path come query param (`?locale=it`) o parte del path (`/blogs/{slug}/posts`)
alle chiamate API corrispondenti — il backend non impone né riconosce da solo
la struttura `/it/...`/`/en/...`, è una convenzione lato frontend da
rispettare quando verrà costruito il routing.

Un blog ha un `default_locale` (usato quando si crea un post senza indicare
`locale` esplicitamente); non esiste invece un default_locale per il sito
principale — ogni pagina statica va creata esplicitamente per ogni lingua
supportata.

## Due meccanismi di autenticazione distinti

| Meccanismo           | Trasporto                                               | Endpoint che lo richiedono                                        | Ottenuto da                                 |
| --------------------- | ------------------------------------------------------ | -------------------------------------------------------------------- | ---------------------------------------------- |
| **Sessione utente**  | Access token JWT (15 min) via `Authorization: Bearer`; refresh token via cookie `httpOnly` | `/auth/me`, `/auth/mfa/*`, `/blogs`, `/posts`, `/comments`          | login (`/auth/login`, SSO)                  |
| **API token**        | Opaco, prefisso `noct_`, via `Authorization: Bearer`, nessuna scadenza di default    | `/tokens`                                                             | script di bootstrap o un token già valido    |

Il primo (sessione) rappresenta *chi sei* (un utente loggato dal browser o da
un client che ha fatto login); il secondo (API token) rappresenta *un accesso
diretto* — motore core oggi, in futuro anche utenti che vogliono
interfacciarsi con l'API senza passare da editor o admin del blog. Dettagli
sugli API token in fondo a questo file (sezione invariata rispetto alla
versione precedente).

### Sessione utente: access token in memoria, refresh token in cookie `httpOnly`

Il refresh token (`app/domain/auth.py::issue_session`) non è più restituito
nel corpo JSON: viaggia solo in un cookie `httpOnly` + `Secure` +
`SameSite`, impostato dal backend su `/auth/login`, `/auth/mfa/verify`,
`/auth/sso/{provider}/callback` e `/auth/refresh` (nome `noct_refresh_token`,
path ristretto a `/api/v1/auth` — il browser non lo invia sulle altre
richieste API). Attributi del cookie (`Secure`/`SameSite`/`Domain`)
configurabili via `NOCT_SESSION_COOKIE_*` (`.env.example`) — necessario per
ambienti senza TLS ancora attivo (K3s a inizio rollout, prima di
cert-manager) o con frontend/backend su domini diversi. L'access token resta
nel corpo JSON: il frontend non lo persiste (niente `localStorage`), lo tiene
solo in memoria e lo ripassa come prima via `Authorization: Bearer` — vedi
`frontend/src/lib/auth-context.tsx`.

Gli endpoint autenticati dal solo cookie (`/auth/refresh`, `/auth/logout`)
richiedono in più l'header `X-CSRF-Token`, uguale al valore del cookie
**non** `httpOnly` `noct_csrf_token` impostato insieme al refresh token
(pattern *double-submit*: un'origine estranea non può leggere quel cookie
per costruire l'header, anche se il browser gli allega comunque il cookie
stesso). Tutti gli altri endpoint che modificano stato restano autenticati
via `Authorization: Bearer` con l'access token, di per sé immune a CSRF —
un'origine estranea non può impostare quell'header su una richiesta
cross-site.

Un cambio password (`POST /users/me/password`, reset "password dimenticata")
cancella le sessioni (`UserSession`, quindi i refresh token) ma un access
token JWT già emesso resterebbe altrimenti valido fino al suo `exp` naturale
(15 minuti), essendo stateless. `User.credentials_changed_at` chiude questa
finestra: `get_current_user`/`get_current_user_optional` confrontano l'`iat`
del token con questo campo e rifiutano (401) ogni token emesso prima
dell'ultimo cambio password.

## Autenticazione utente (password, MFA, SSO)

### Registrazione e login con password

**`POST /api/v1/auth/register`**

```json
{"username": "mario", "email": "mario@example.com", "password": "..."}
```

→ `201`, utente creato (`platform_role=utente`, MFA disattiva). Nessun login
automatico: va fatto separatamente. `409` se username o email già in uso.
`400` se lo username non rispetta il formato (todo/USERS.md #1): minuscole,
cifre, `-` e `_` come separatori interni (mai a inizio/fine né ripetuti),
3–32 caratteri, e non in blacklist (`app/domain/usernames.py`). Lo username è
l'identificatore citabile come `@username` nei contenuti. `400` anche se la
password è più corta di 10 caratteri (`app/domain/passwords.py`, unico
requisito della policy — nessun vincolo su classi di caratteri): stesso
controllo applicato dal flusso "password dimenticata" più sotto.

Se `NOCT_DEPLOYMENT_MODE=solo` (installazione a singolo proprietario, es.
blog personale — vedi `.env.example`): il **primo** utente registrato diventa
automaticamente `super_admin` (nessun intervento manuale sul DB); ogni
registrazione successiva ritorna `409`. Con `NOCT_DEPLOYMENT_MODE=platform`
(default) il comportamento è quello multiutente descritto sopra, senza limiti.

**`POST /api/v1/auth/login`**

```json
{"email": "mario@example.com", "password": "..."}
```

→ `200`. Due forme possibili:

- MFA non attiva: sessione diretta. Il refresh token non è nel corpo della
  risposta: il backend lo imposta come cookie `httpOnly` (vedi sopra).
  ```json
  {"access_token": "...", "token_type": "bearer"}
  ```
- MFA attiva: richiede un secondo passaggio.
  ```json
  {"mfa_required": true, "method": "totp", "challenge": "..."}
  ```
  Se `method` è `"email"`, a questo punto è già stato accodato l'invio del
  codice (vedi dettagli Email OTP più sotto).

`401` su credenziali errate o utente disattivato. `429` oltre 20 tentativi/5
minuti dallo stesso IP o oltre 5 tentativi/5 minuti sulla stessa email
(protezione brute-force via Redis, `app/domain/rate_limit.py` — se Redis non
è raggiungibile il limite è semplicemente disattivato, fail open).

**`POST /api/v1/auth/mfa/verify`** — completa il login dopo una risposta
`mfa_required`.

```json
{"challenge": "...", "code": "123456"}
```

→ `200` con `access_token` come sopra (refresh token nel cookie). `401` se il
codice è sbagliato/scaduto o il challenge non è più valido (dura 5 minuti).
`429` oltre 20 tentativi/5 minuti dallo stesso IP o oltre 8 tentativi/5 minuti
sullo stesso soggetto del challenge (stessa protezione del login, applicata
anche a `mfa/totp/confirm` e `mfa/email/confirm` sotto — un codice a 6 cifre è
altrimenti indovinabile in un numero di tentativi gestibile).

**`POST /api/v1/auth/refresh`** — nessun corpo: il refresh token è letto dal
cookie `noct_refresh_token`, richiede l'header `X-CSRF-Token` (vedi sopra).

→ `200`, nuovo `access_token` e nuovo cookie di refresh (rotation: il
precedente viene revocato, riusarlo dà `401`). `401` senza cookie di sessione
o con refresh token scaduto/già rotato. `403` senza `X-CSRF-Token` valido.

**`POST /api/v1/auth/logout`** — nessun corpo, stesso cookie/header di sopra.

→ `204`. Revoca la sessione e cancella i cookie; idempotente (nessun errore
se la sessione era già revocata o il cookie assente). `403` senza
`X-CSRF-Token` valido.

**`GET /api/v1/auth/me`** — richiede sessione. Ritorna
`{id, username, email, mfa_enabled}`.

**`GET /api/v1/auth/username-available?username=...`** — pubblico, nessuna
sessione richiesta. Rate limit 30 richieste/minuto per IP. Verifica formato
(`app/domain/usernames.py::validate_username` — lunghezza, caratteri
ammessi, parole riservate) e unicità con la stessa query di
`register_user`:

```json
{"available": true, "reason": null}
{"available": false, "reason": "invalid_format"}
{"available": false, "reason": "taken"}
```

Pensato per il controllo dal vivo mentre l'utente digita in fase di
registrazione — non sostituisce la validazione di unicità fatta comunque a
`POST /auth/register` (race condition tra il check e il submit sempre
possibile, gestita lì).

### Cambio password da loggati

**`POST /api/v1/users/me/password`** — richiede sessione.

```json
{"current_password": "...", "new_password": "..."}
```

→ `200 {"status": "ok"}`. Verifica `current_password` contro l'hash
esistente (stessa funzione del login), applica la policy minima di 10
caratteri, e **revoca tutte le sessioni attive** dell'utente — stesso
principio di `POST /auth/password/reset` sopra (`app/domain/password_reset.py`),
qui applicato esplicitamente perché prima d'ora non esisteva alcun modo di
cambiare la password restando loggati, solo il reset via email (senza
vecchia password). La sessione corrente viene chiusa anch'essa: il client
deve rifare login con la nuova password. `400` se `current_password` è
sbagliata, se `new_password` non rispetta la policy minima, o se l'account
non ha una password impostata (utente collegato solo via SSO).

### Password dimenticata

**`POST /api/v1/auth/password/forgot`**

```json
{"email": "mario@example.com"}
```

→ **sempre** `202`, email esistente o no: la risposta non deve mai rendere
enumerabile quali indirizzi hanno un account. Se l'email corrisponde a un
utente attivo, accoda un codice a 6 cifre via email (stesso meccanismo
dell'OTP MFA/cambio email — RabbitMQ + `app/workers/email_otp_consumer.py`,
TTL 10 minuti). `429` oltre 20 tentativi/5 minuti dallo stesso IP o 5/5
minuti sulla stessa email.

**`POST /api/v1/auth/password/reset`**

```json
{"email": "mario@example.com", "code": "123456", "new_password": "..."}
```

→ `204`. Imposta la nuova password (stessa policy minima di 10 caratteri di
sopra) e **revoca tutte le sessioni attive** dell'utente (ogni refresh token
già emesso smette di funzionare, come per la cancellazione account —
`app/domain/gdpr.py`): un reset di password è tipicamente una risposta a un
account compromesso, non solo a una password dimenticata. `400` se il
codice è sbagliato/scaduto/già usato o se `new_password` non rispetta la
policy minima — stesso messaggio generico per email sconosciuta e codice
errato. `429` con gli stessi limiti di `password/forgot`.

### MFA — gestione (richiede una sessione attiva, cioè un login già fatto)

**`POST /api/v1/auth/mfa/totp/setup`** → genera un secret TOTP (non ancora
attivo) e lo ritorna insieme a un `provisioning_uri` (`otpauth://...`) e un
QR già pronto da mostrare (`qr_code_data_uri`, SVG generato interamente lato
backend — `qrcode`, `app/domain/mfa.py::totp_qr_code_data_uri` — mai da un
servizio di terze parti: il secret non deve lasciare il backend):

```json
{
  "secret": "BASE32...",
  "provisioning_uri": "otpauth://totp/Notturni:mario%40example.com?...",
  "qr_code_data_uri": "data:image/svg+xml;base64,..."
}
```

**`POST /api/v1/auth/mfa/totp/confirm`** — `{"code": "123456"}` → `204` e
attiva l'MFA (`mfa_enabled=true`, `mfa_method="totp"`). `400` se il codice non
corrisponde al secret generato dal setup.

**`POST /api/v1/auth/mfa/email/setup`** → `202`, genera e accoda (RabbitMQ)
l'invio di un codice all'email dell'utente (vedi dettagli Email OTP più sotto).

**`POST /api/v1/auth/mfa/email/confirm`** — `{"code": "123456"}` → `204` e
attiva l'MFA (`mfa_method="email"`). `400` se il codice non corrisponde o è
scaduto (validità 10 minuti).

**`POST /api/v1/auth/mfa/disable`** → `204`, disattiva l'MFA e cancella il
secret TOTP eventualmente salvato.

### SSO (Google, Microsoft, GitHub, LinkedIn)

**`GET /api/v1/auth/sso/{provider}/login`** — redirect all'authorize URL del
provider. `503` se il provider non ha credenziali configurate (vedi
`.env.example`, variabili `NOCT_OAUTH_*`).

**`GET /api/v1/auth/sso/{provider}/callback`** — riceve il redirect dal
provider, scambia il code, recupera l'userinfo e applica l'account linking:

- nessun utente con quell'identità collegata e nessun utente con quella email
  → ne crea uno nuovo (username derivato dalla parte locale dell'email);
- identità già collegata → login diretto sull'utente associato;
- utente esistente con la stessa email, **senza** MFA → collegamento
  immediato e login;
- utente esistente con la stessa email, **con** MFA attiva → non collega
  subito: ritorna `{"mfa_required": true, "method": ..., "challenge": ...}`
  come nel login normale. Il completamento del collegamento avviene dentro
  `/auth/mfa/verify`, che riconosce il challenge come "collegamento SSO in
  sospeso" e lo finalizza dopo la verifica del codice.

Risposta finale (login riuscito, senza MFA da verificare): stesso formato di
`/auth/login` (`access_token` nel corpo, refresh token nel cookie).

**Limitazione nota:** senza credenziali OAuth reali (client id/secret per
ciascun provider) il flow non è testabile end-to-end in questo ambiente di
sviluppo. La logica di account linking è invece testata direttamente (vedi
`app/domain/sso.py`); gli endpoint `/login` e `/callback` andranno verificati
con credenziali reali prima di un uso in produzione — in particolare per
LinkedIn, i cui dettagli esatti dell'endpoint userinfo potrebbero richiedere
aggiustamenti.

**Email OTP:** il codice viene generato, salvato (hash) e pubblicato su
RabbitMQ (coda `email_otp`); il consumer (`app/workers/email_otp_consumer.py`)
lo invia via SMTP (`app/core/mail.py`, solo `smtplib` di libreria standard —
`NOCT_SMTP_HOST`/`_PORT`/`_USER`/`_PASSWORD`/`_USE_TLS`/`_FROM_EMAIL`, vedi
`.env.example`). In locale punta al servizio `mailhog` di `compose.yaml`
(nessuna email reale in uscita, ispezionabile su `http://localhost:8025` o
`GET http://localhost:8025/api/v2/messages`); in produzione va sempre
puntato a un provider SMTP/transazionale reale. Senza `NOCT_SMTP_HOST`
configurato l'OTP resta solo loggato dal consumer (comodo per sviluppo senza
SMTP a disposizione, mai il caso in produzione). Un errore SMTP genuino
(host irraggiungibile, credenziali sbagliate) fa nack/requeue del messaggio.

## Blog

**`POST /api/v1/blogs`** — richiede sessione.

```json
{
  "slug": "il-mio-blog",
  "title": "Il mio blog",
  "default_locale": "it",
  "subtitle": "Appunti sparsi",
  "description": "Descrizione breve del blog, max 256 caratteri.",
  "visibility": "public",
  "default_author_display_name": "il-mio-username"
}
```

→ `201`. Applica le regole di dominio (vedi
[ROADMAP.md](../ROADMAP.md#1-prodotto-e-regole-di-dominio)): slug di almeno 4
caratteri alfanumerici/trattino, non in blacklist (vedi
`app/domain/blog_rules.py`), massimo 5 blog per utente. `default_locale` è
opzionale (default `it`). `subtitle` (max 64) e `description` (max 256) sono
opzionali. `default_author_display_name` è opzionale (il frontend lo
pre-compila con lo username di chi crea il blog, ma resta un campo libero,
modificabile in seguito con `PATCH`): se assente/vuoto il nome pubblico
ricade sulla preferenza di profilo di chi scrive (`username` di default).
`visibility` è opzionale (default `public`), valori:

- `public` — raggiungibile da chiunque, compare nel feed della homepage;
- `members` — pagine pubbliche del blog leggibili solo da un utente
  autenticato sulla piattaforma; escluso dal feed;
- `private` — diario: leggibile solo dal proprietario e dai collaboratori,
  **scrivibile dal solo proprietario** a prescindere dalle membership.

`400` se una regola non è rispettata, `409` se lo slug è già in uso.

`BlogOut` (risposta di tutti gli endpoint blog) include anche
`mentions_enabled` (bool, default `true`): se attivo, il frontend trasforma
le `@username` nel contenuto dei post in link al profilo citato — vedi
"Menzioni `@username`" nella sezione Post.

**`GET /api/v1/blogs`** — pubblico, nessuna sessione richiesta. Directory dei
blog pubblici indicizzabili: `visibility=public`, non sospesi
(`is_suspended=false`) e `search_indexing_enabled=true` — stesso criterio del
`robots.txt` generato, più restrittivo del semplice "pubblico" usato dal feed
dei post (che include anche i blog pubblici non indicizzabili). Parametri
`limit` (default 30, max 100) e `offset` per la paginazione. `q` cerca in
slug/titolo/sottotitolo (sottostringa), `locale` filtra per lingua
principale, `sort` è `active` (default: ultimo post pubblicato, poi
creazione), `new` (creazione) o `followers`. Ogni voce è un blog più
`post_count` (post effettivamente pubblicati), `follower_count` e
`last_published_at` — i conteggi delle card della directory (mockup 4a/4c).
`owner_id` sempre `null` in questa lista (nessun viewer autenticato).

**`GET /api/v1/blogs/{slug}/overview`** — richiede sessione: proprietario o
membro con qualunque ruolo (`403` altrimenti, `404` se il blog non esiste).
Conteggi per la tab Panoramica del blog (todo/UX_REDESIGN.md B1, mockup 5a):

```json
{
  "posts_total": 12, "posts_published": 9, "posts_scheduled": 1, "posts_draft": 1, "posts_in_review": 1,
  "followers": 214, "members": 2, "pending_comments": 3, "approved_comments": 40, "media": 18,
  "last_published_at": "2026-09-12T07:00:00Z"
}
```

`posts_scheduled` sono i `published` con `published_at` futuro; `media` è il
numero di immagini citate nei post (tabella `post_media`). In più (B2):
`reads_30d` — 30 voci `{day, reads}` (giorni UTC, quelli senza letture a 0),
`reads_total_30d`, `storage_bytes` — byte occupati su storage da media e
backup Markdown del blog (prefissi `userdata/{utente}/{blog}/` di
proprietario e collaboratori; `null` se lo storage non risponde, `0` se il
bucket non è mai stato creato) — e `storage_limit_mb`, il limite impostato
da un Super Admin (`platform_config.max_blog_storage_mb`), `null` se nessun
limite.

**`POST /api/v1/posts/{post_id}/read`** — pubblico, `204`, nessun corpo.
Conteggio letture aggregato per giorno (tabella `post_reads_daily`, mockup
5a): nessun cookie, nessuna sessione, nessun IP o identificativo del lettore
persistito — solo `+1` su (post, giorno UTC). Inviato dal browser
(`navigator.sendBeacon`) dopo qualche secondo sulla pagina pubblica del
post. Un limite per IP+post in Redis (1 ogni 6 ore, volatile) evita di
contare i reload ravvicinati: oltre il limite la richiesta risponde comunque
`204` senza contare. `404` per post non pubblicamente visibili (bozza,
pianificato, nascosto, blog non pubblico o sospeso).

**`GET /api/v1/blogs/mine`** — richiede sessione. Lista i blog di proprietà
dell'utente.

**`GET /api/v1/blogs/member-of`** — richiede sessione. Blog altrui su cui
l'utente ha una membership attiva (dopo aver accettato un invito). Ogni voce:
`{"blog": <BlogOut>, "role": "co_autore"|"mediatore"|..., "author_display_name": str|null}`.

**`GET /api/v1/blogs/{slug}`** — token opzionale. Dettaglio del blog. Segue
la `visibility`: un blog `members`/`private` non visibile all'utente
corrente (o anonimo) risponde `404` come se non esistesse. Lo stesso vale per
`GET .../config`, `GET .../categories` e per gli endpoint di lettura dei post
del blog (vedi sezione Post). Il campo `owner_id` della risposta vale
`null` per chiunque non sia il proprietario stesso: è l'unico campo che
punterebbe all'id reale di chi gestisce il blog, e un blog che si presenta
con un `default_author_display_name` diverso dallo username del proprietario
non deve permettere di risalirvi (vedi "Profilo utente e follow" più sotto).

**`PATCH /api/v1/blogs/{slug}`** — richiede sessione, solo il proprietario
(`403` altrimenti). Campi aggiornabili: `title`, `subtitle` / `description`
(`""` azzera, assente lascia invariato; `400` se oltre 64 / 256 caratteri),
`visibility` (`public` | `members` | `private`), `comments_mode`
(`everyone` | `members` | `closed` — default chi può commentare, vedi sezione
Commenti; `everyone` richiede `NOCT_TURNSTILE_SITE_KEY`/`_SECRET_KEY`
configurate sull'istanza, altrimenti `400`), `mentions_enabled` (bool —
trasforma le `@username` nei post in link, vedi sezione Post),
`search_indexing_enabled` / `ai_crawling_enabled` (bool, default `true` —
opt-out per crawler dei motori di ricerca e, separatamente, dei crawler
IA/LLM: vedi `GET /api/v1/seo/crawl-directives` sopra; escludere il blog
esclude anche tutti i suoi post, indipendentemente da un eventuale override
per singolo post),
`default_author_display_name` — nome pubblico degli autori sui post di
questo blog. Se valorizzato è **imposto** (nessun override per singolo
autore o post — todo/USERS.md #2), a meno che il collaboratore non abbia un
proprio alias di membership, che ha la precedenza. Stringa vuota `""` lo
azzera, assente lo lascia invariato.
`is_paused` (bool, B3 — mockup 5c "Pause this blog"): blog in pausa
volontaria, i lettori non vedono più post e pagine (il dettaglio
`GET /blogs/{slug}` resta leggibile con `is_paused=true` per mostrare la
pagina "in pausa"), sparisce da feed/directory/sitemap; proprietario e
collaboratori continuano a lavorarci dalla dashboard, nulla è cancellato.
`extra_locales` (lista di codici ISO 639-1, B3): lingue secondarie del blog
oltre a `default_locale`, informative (duplicati e lingua principale
scartati; `400` se un codice non è valido).

La risposta di `GET /blogs/{slug}` include anche `is_suspended`,
`is_paused`, `deleted_at` ed `extra_locales`. Per un blog **pubblico**
sospeso da un admin o in pausa, il dettaglio risponde `200` con i flag (e
solo quello: post, pagine, bibliografia restano `404`), così la pagina
pubblica può spiegare lo stato (mockup 3d). Un blog in attesa di
cancellazione è `404` per chiunque tranne il proprietario.

**`POST /api/v1/blogs/{slug}/transfer`** — `{username}`, solo il
proprietario. Trasferisce la proprietà a un **coautore attuale** (`400` se
l'utente non ha una membership `co_autore`, o ha già raggiunto il limite di
blog; `404` se non esiste). Chi cede resta come coautore; la membership del
nuovo proprietario viene rimossa. Registrato nel registro di audit
(`blog.ownership_transferred`).

**`GET /api/v1/blogs/{slug}/export`** — solo il proprietario. Scarica uno
ZIP (`application/zip`) con tutto il contenuto del blog: `blog.json`, un
Markdown per post in `posts/{locale}-{slug}.md` (front matter con titolo,
stato, data, autore, categoria, tag, copertina; note in coda come
`[^n]: …`), le pagine statiche in `pages/`, `comments.json` (tutti gli
stati), `categories.json`, `media.json` (URL e alt delle immagini citate —
i binari restano sullo storage) e `links.json`. Generato al volo, nessun
link a scadenza.

**`DELETE /api/v1/blogs/{slug}`** — `{confirm_slug}`, solo il proprietario;
`400` se lo slug non corrisponde. Cancellazione con tolleranza (B3, mockup
5c/3d): imposta `deleted_at`, il blog sparisce da pagine pubbliche, feed,
directory e sitemap (resta in `GET /blogs/mine` con `deleted_at`
valorizzato), i post non sono più scrivibili. Dopo 30 giorni il worker di
manutenzione (`app/workers/audit_maintenance.py`, un giro al giorno) lo
elimina definitivamente con post, commenti, frammenti, pagine, categorie,
collaboratori, follower, configurazione e oggetti su storage
(`app/domain/blog_lifecycle.py`). Idempotente. Audit `blog.deleted`.

**`POST /api/v1/blogs/{slug}/restore`** — solo il proprietario. Annulla la
cancellazione entro il periodo di tolleranza (`deleted_at` torna `null`).
Audit `blog.restored`.

**`POST /api/v1/blogs/{slug}/follow`** / **`DELETE .../follow`** — richiede
sessione. Segui/smetti di seguire un blog; idempotenti (`204` anche se già
nello stato richiesto).

**`GET /api/v1/blogs/{slug}/followers`** — pubblico. Lista `{username}` di chi
segue il blog.

**`GET /api/v1/blogs/{slug}/config`** — pubblico. Configurazione di
presentazione del blog (palette/tipografia/layout — vedi
[ROADMAP.md](../ROADMAP.md#2-estetica) per i vincoli), applicata dal frontend
a tutte le pagine pubbliche del blog (`BlogPageShell`, todo/UX_REDESIGN.md
B10): palette come variabili CSS, `typography.heading_font`/`body_font`/
`monospace_font` come `--font-heading`/`--font-body`/`--font-monospace` (font
self-hostati al build, nessuna richiesta a Google a runtime — quest'ultimo
usato per i blocchi di codice, blocco "evidenziazione sintassi"),
`body_size`/`measure` per dimensione del corpo e larghezza della colonna di
lettura del post, `layout` per la disposizione del feed della home del blog.
JSON libero; se il proprietario non ha ancora salvato nulla, ritorna il
default della piattaforma (identico allo shell non personalizzato):

```json
{
  "palette": {"background": "#fbf9f6", "foreground": "#2b2a28", "primary": "#3e6259", "muted": "#a8a29a", "border": "#e7e2da"},
  "typography": {"heading_font": "Lora", "body_font": "Source Sans 3", "monospace_font": "JetBrains Mono"},
  "layout": "standard"
}
```

**`PUT /api/v1/blogs/{slug}/config`** — richiede sessione, solo il
proprietario (`403` altrimenti). Sostituisce l'intera configurazione (non è
un merge). Regole imposte (`400` altrimenti), resto della struttura (`layout`
e qualsiasi altra chiave) libero:

- `palette`: al massimo 5 colori; ogni colore esadecimale non può superare il
  90% di saturazione HLS (palette "calma", CLAUDE.md § Estetica).
- `palette_dark` (opzionale): stessi vincoli di `palette`; è la variante
  scura applicata alle pagine pubbliche del blog quando il lettore usa il
  tema scuro. Assente, in tema scuro vale la palette scura di piattaforma.
- `typography`: al massimo 3 font distinti tra `heading_font`/`body_font`/
  `monospace_font` (`body_size`/`measure`, pur essendo anch'esse stringhe, non
  contano verso questo limite); se presenti, `heading_font` deve essere uno
  dei font serif curati (`Lora`, `Merriweather`, `Playfair Display`,
  `Source Serif 4`, `Crimson Pro`), `body_font` uno dei font sans-serif curati
  (`Inter`, `Nunito Sans`, `Work Sans`, `Source Sans 3`, `Karla`) e
  `monospace_font` uno dei font monospace curati (`JetBrains Mono`,
  `Fira Code`, `IBM Plex Mono`, `Source Code Pro`, `Space Mono`; default di
  piattaforma `JetBrains Mono`) — vedi `backend/app/domain/blog_config.py`.
  `body_size` (`"17"`/`"18"`/`"19"`) e `measure` (`"narrow"`/`"normal"`) non
  sono validati lato backend (solo accettati); un valore diverso da quelli
  attesi è ignorato dal frontend, che ricade sul default.
- `footer` (opzionale): override per questo blog delle sole colonne 1/2 del
  footer di piattaforma (`GET /api/v1/footer`) — `{"column1": "...",
  "column2": "..."}`, Markdown libero, max 5000 caratteri ciascuna, nessun'altra
  chiave ammessa (`400` altrimenti: non è possibile sovrascrivere `column3` né
  `bottom_bar`, sempre e solo di piattaforma). Assente/vuoto: il blog eredita
  il default di piattaforma per entrambe.

Altre chiavi restano libere.

**`POST /api/v1/blogs/{slug}/cover-image`** — richiede sessione, solo il
proprietario (`403` altrimenti). `multipart/form-data`, campo `file`.
Immagine di copertina del blog (banner della home pubblica, facoltativa):
stessi formati/limite di dimensione e stessa moderazione automatica di
`POST .../media` sotto (incluso il controllo dello spazio massimo per blog,
`413` se superato — vedi `platform_config.max_blog_storage_mb`) — l'upload
aggiorna `cover_image_url` e
`cover_image_is_sensitive` (risultato della moderazione), azzera
`cover_image_categories`/`cover_image_alt_text`. Sostituire una cover
esistente non cancella l'oggetto precedente su storage (stessa scelta di
`Post.cover_image_url`). Ritorna il `Blog` aggiornato (`BlogOut`).
Crea anche una riga nella libreria media del blog (`GET .../media` sotto,
`used_as_blog_cover=true` finché resta la cover corrente) — prima non ci
finiva mai, a differenza della cover di un post (che passa dallo stesso
endpoint di `POST .../media`).

**`PATCH /api/v1/blogs/{slug}/cover-image`** — solo il proprietario, `400`
se il blog non ha ancora una cover. `{"categories": ["nudity", ...],
"alt_text"?}` (vocabolario categorie in
`backend/app/domain/content_media.py::SENSITIVITY_CATEGORIES`): aggiorna
l'avviso manuale sui contenuti senza ricaricare l'immagine, stesso
principio del `PATCH /posts/{id}` quando cambia solo `cover_image_categories`
— categorie non vuote forzano `cover_image_is_sensitive=true`. `alt_text`
assente lascia invariato, presente (anche `null`/`""`) lo azzera o
sostituisce. Ritorna il `Blog` aggiornato.

**`DELETE /api/v1/blogs/{slug}/cover-image`** — solo il proprietario. Azzera
cover/avviso/categorie/alt text (l'oggetto su storage non viene cancellato,
stessa scelta di cui sopra). Ritorna il `Blog` aggiornato.

**`POST /api/v1/blogs/{slug}/favicon`** — richiede sessione, solo il
proprietario. `multipart/form-data`, campo `file`. Favicon dedicata del blog
(facoltativa, mostrata nella scheda del browser sulle sue pagine pubbliche):
PNG/JPEG/WEBP, max 512 KiB (`400` altrimenti) — **nessuna moderazione
automatica**, a differenza di cover/media: è un'icona di identità, non
contenuto, stesso principio dell'avatar utente (`POST /users/me/avatar`).
Sostituire una favicon esistente **cancella** l'oggetto precedente su
storage (a differenza della cover, qui l'oggetto è piccolo e dedicato, come
l'avatar). Bucket pubblico degli avatar, prefisso `favicons/{blog_id}/`.
Ritorna il `Blog` aggiornato.

**`DELETE /api/v1/blogs/{slug}/favicon`** — solo il proprietario. Cancella
l'oggetto su storage e azzera `favicon_url`. Ritorna il `Blog` aggiornato.

**`POST /api/v1/blogs/{slug}/media`** — richiede sessione e accesso in
scrittura al blog (proprietario/autore/co-autore). `multipart/form-data`,
campo `file`. Formati ammessi: PNG, JPEG, WEBP, GIF; max 10 MiB (`400`
altrimenti). Immagine da incorporare nel Markdown di un post (es.
`![alt](url)`). Vedi "Media e backup" sotto per il path S3. Se un Super
Admin ha impostato uno spazio massimo per blog
(`platform_config.max_blog_storage_mb`, `PATCH /admin/config`), l'upload
che lo supererebbe risponde `413` invece di essere accettato — nessun
controllo (comportamento invariato) se il limite non è impostato.

```json
{"url": "https://.../notturni/userdata/{user_uuid}/{blog_uuid}/media/{uuid}.png"}
```

**`GET /api/v1/blogs/{slug}/mentionable-users?q=<prefisso>&limit=8`** —
richiede sessione e accesso in scrittura al blog. Suggerimenti per
l'autocomplete delle `@menzioni` nell'editor: proprietario, collaboratori e
follower del blog il cui username inizia con `q` o il cui nome pubblico lo
contiene (`q` vuoto → primi risultati per username). `limit` 1–25 (default
8). Ritorna `[{"username": "...", "display_name": str|null, "avatar_url":
str|null}]`. Se il blog ha `mentions_enabled=false`, ritorna sempre `[]`.

**`GET /api/v1/blogs/{slug}/bibliography`** — token opzionale, segue la
`visibility` del blog (`404` se non visibile). Bibliografia automatica
(todo/EDITOR.md): tutte le note a piè di pagina dei post **pubblicati**,
raggruppate per testo identico (confronto senza distinzione di
maiuscole/spaziatura) e ordinate per recency del primo post che le cita:

```json
[
  {
    "content": "Testo della nota, Markdown inline.",
    "citations": [
      {"post_title": "...", "post_slug": "...", "permalink": "/{blog}/{slug}", "locale": "it", "idx": 1}
    ]
  }
]
```

**`GET /api/v1/blogs/{slug}/media-bibliography`** — stessa autorizzazione e
stesso principio della bibliografia sopra, per i media (oggi solo immagini)
citati nel corpo dei post **pubblicati**, raggruppati per URL identico:

```json
[
  {
    "url": "https://.../media/....jpg",
    "alt_text": "Descrizione dell'immagine",
    "categories": ["nudity", "explicit"],
    "is_sensitive": true,
    "citations": [
      {"post_title": "...", "post_slug": "...", "permalink": "/{blog}/{slug}", "locale": "it", "used_at": "2026-01-01T00:00:00Z"}
    ]
  }
]
```

`categories` è il sottoinsieme di `suggestive`/`nudity`/`explicit`/`other`
scelto dall'autore per quell'immagine (vedi "Avviso sui contenuti" nella
sezione Post) — vuoto se non segnalata *oppure* segnalata (dalla sola
automoderazione, o dal modal senza una categoria specifica) senza una
categoria: per questo `is_sensitive` è un campo separato, non derivabile da
`categories.length > 0` — è lui a decidere se l'immagine va mostrata sfocata
nella griglia, stesso flag usato dal rendering del post. `used_at` è la data
di pubblicazione del post che la cita.

**`GET /api/v1/blogs/{slug}/links-bibliography`** — stesso principio, per i
link citati nel corpo dei post pubblicati:

```json
[
  {
    "url": "https://esempio.org/articolo",
    "link_text": "testo del link",
    "citations": [
      {"post_title": "...", "post_slug": "...", "permalink": "/{blog}/{slug}", "locale": "it", "used_at": "2026-01-01T00:00:00Z"}
    ]
  }
]
```

I dati di entrambi gli endpoint vengono dalla stessa fonte di verità del
contenuto Markdown del post (tabelle `post_media`/`post_links`, riscritte
per intero ad ogni salvataggio come `post_tags`/`post_notes` — non un
elenco indicato a parte dal client).

### Categorie

Tassonomia del blog (CLAUDE.md): a differenza dei tag (liberi, fino a 5 per
post — vedi sezione "Tag"), le categorie sono definite in anticipo dal
proprietario/autori e un post ne ha **al più una**.

**`GET /api/v1/blogs/{slug}/categories`** — pubblico. Elenco, ordinato per
nome.

**`POST /api/v1/blogs/{slug}/categories`** — richiede sessione e accesso in
scrittura al blog. `{"name": "Viaggi", "slug": "viaggi"}` → `201`. `slug`:
minuscolo, lettere/cifre/trattini singoli, max 60 caratteri. `400` se il
formato non è valido, `409` se lo slug è già in uso su quel blog.

**`PATCH /api/v1/blogs/{slug}/categories/{category_id}`** — stessa
autorizzazione. Aggiorna `name`/`slug` (entrambi opzionali); stesse regole
di validazione/unicità della creazione.

**`DELETE /api/v1/blogs/{slug}/categories/{category_id}`** — stessa
autorizzazione. I post con questa categoria non vengono cancellati, restano
solo senza categoria.

### Pagine statiche del blog

Feature opt-in per blog (`Blog.static_pages_enabled`, **disattiva di
default** — a differenza delle pagine di piattaforma, sempre attive, vedi
sezione "Pagine statiche (sito principale)"), pensata per pagine come "Chi
sono"/"Contattami" del singolo blog. Stesso schema di traduzione dei post
(`translation_group_id` + `locale` + `slug`), ma **niente tag, categorie o
stato di pubblicazione a workflow**: solo `is_published` booleano. Permalink
pubblico `/{blog_slug}/pagina/{slug}` (niente data, a differenza dei post).

**`GET /api/v1/blogs/{slug}/pages?locale=it`** — elenco. Pubblico: solo
pagine `is_published=true`; chi ha accesso in scrittura al blog vede anche
le bozze.

**`GET /api/v1/blogs/{slug}/pages/{page_slug}?locale=it`** — risoluzione del
permalink pubblico. Stessa distinzione pubblicate/bozze di sopra. `404` se
non trovata.

**`GET /api/v1/blogs/{slug}/pages/by-id/{page_id}`** — richiede accesso in
scrittura al blog. Recupera una pagina per id (bozza inclusa), per l'editor
di dashboard — a differenza della rotta per slug sopra, pensata per la
risoluzione del permalink pubblico.

Tutte le risposte (`PageOut`) includono `permalink` (calcolato,
`/{blog_slug}/pagina/{slug}` per le pagine di blog o `/p/{slug}` per
quelle di piattaforma) e `mentions_enabled` (mirror di `Blog.mentions_enabled`,
sempre `true` per le pagine di piattaforma) — utili al rendering pubblico
lato frontend senza una fetch separata del blog.

**`POST /api/v1/blogs/{slug}/pages`** — richiede accesso in scrittura al
blog (proprietario o membership `autore`/`co_autore`) **e**
`static_pages_enabled=true` sul blog (`403` altrimenti, messaggio esplicito).
Crea una pagina, radice di una nuova famiglia di traduzioni:

```json
{"slug": "contattami", "locale": "it", "title": "Contattami", "content": "...", "is_published": true}
```

`400` se lo slug non è nel formato valido (minuscolo, lettere/cifre/trattini
singoli, max 80 caratteri) o la lingua non è supportata; `409` se esiste già
una pagina con quello slug in quella lingua su questo blog.

**`POST /api/v1/blogs/{slug}/pages/{page_id}/translations`** — stessa
autorizzazione (inclusa `static_pages_enabled`). Aggiunge una traduzione alla
stessa famiglia (stesso corpo di `POST .../pages`, `locale` sempre esplicito).

**`GET /api/v1/blogs/{slug}/pages/{page_id}/translations`** — pubblico.
Lista `{id, locale, slug, is_published}` delle sole traduzioni pubblicate
della stessa famiglia (stesso pattern di `GET /api/v1/posts/{post_id}/translations`
per i post): per il selettore di lingua lato frontend, che mostra sempre a
parte la pagina corrente.

**`PATCH /api/v1/blogs/{slug}/pages/{page_id}`** — richiede accesso in
scrittura al blog (**non** richiede `static_pages_enabled`: modificare o
disattivare pagine già esistenti resta sempre possibile, anche a feature
disattivata). Aggiorna `slug`/`title`/`content`/`is_published`, tutti
opzionali.

**`DELETE /api/v1/blogs/{slug}/pages/{page_id}`** — stessa autorizzazione di
`PATCH` (nessun controllo su `static_pages_enabled`). `204`.

### Collaboratori e inviti

Il proprietario può invitare altri utenti registrati come **co-autore** o
**mediatore** (todo/BLOG.md #3). L'invito resta `pending` finché l'invitato
non lo accetta dalla propria dashboard; solo all'accettazione nasce la
`BlogMembership`. Una sola riga di invito per (blog, utente): un nuovo invito
dopo un rifiuto/revoca riusa la stessa riga.

Lato proprietario (tutti `403` se non sei il proprietario del blog):

- **`GET /api/v1/blogs/{slug}/members`** — collaboratori del blog:
  `[{user_id, username, avatar_url, role, author_display_name, created_at}]`.
- **`PATCH /api/v1/blogs/{slug}/members/{user_id}`** — `{"role": "co_autore"|"mediatore"}`
  (`400` per altri ruoli). `404` se non è un collaboratore.
- **`DELETE /api/v1/blogs/{slug}/members/{user_id}`** — rimuove la membership
  (`204`, idempotente).
- **`GET /api/v1/blogs/{slug}/invitations`** — tutti gli inviti del blog
  (qualsiasi stato).
- **`POST /api/v1/blogs/{slug}/invitations`** — `{"username": "...", "role": "co_autore"|"mediatore"}`
  → `201`. `400` ruolo non ammesso / si invita il proprietario stesso; `404`
  utente inesistente; `409` è già collaboratore o ha già un invito `pending`.
- **`DELETE /api/v1/blogs/{slug}/invitations/{invitation_id}`** — revoca un
  invito `pending` (`204`).

Lato collaboratore:

- **`PATCH /api/v1/blogs/{slug}/my-membership`** — `{"author_display_name": "..."}`
  (`""` azzera): l'alias con cui il collaboratore firma i post su questo blog
  (todo/BLOG.md #4). `404` se non sei un collaboratore.

Lato invitato (sull'utente corrente, path senza slug per non collidere con
`GET /blogs/{slug}`):

- **`GET /api/v1/blogs/received-invitations`** — inviti ricevuti ancora
  `pending`: `[{id, blog_slug, blog_title, role, status, invited_username,
  invited_by_username, created_at, responded_at}]`.
- **`POST /api/v1/blogs/received-invitations/{invitation_id}/accept`** — crea
  la membership col ruolo dell'invito, segna `accepted`. `404` se non è tuo,
  `409` se non è più `pending`.
- **`POST /api/v1/blogs/received-invitations/{invitation_id}/decline`** —
  segna `declined`.

## Post

Il contenuto (`content`) è **Markdown**: nessun rendering lato backend, la
conversione a HTML (con sanificazione) è responsabilità del frontend al
momento della lettura.

**Menzioni `@username`** (todo/USERS.md #1): nel testo, una `@` a inizio riga
o preceduta da spazio seguita da uno username valido (minuscole/cifre,
`-`/`_` interni) è una menzione. Il backend non la elabora — resta `@username`
nel Markdown; è il frontend a trasformarla in link al profilo `/u/{username}`
al rendering, **solo se** il blog ha `mentions_enabled=true` (default; vedi
sezione Blog). `PostOut` riporta `mentions_enabled` del blog per comodità del
client. L'endpoint `GET /blogs/{slug}/mentionable-users` alimenta
l'autocomplete dell'editor.

**Note a piè di pagina** (todo/EDITOR.md): sono un elenco strutturato del
post (`notes`: `[{"idx": 1, "content": "testo Markdown inline"}]`), **non nel
corpo**. `idx` è il numero (1–999) della nota; nel `content` del post il
riferimento è il marcatore link `[idx](#nota-idx)` (l'editor lo inserisce
come vero nodo link, così sopravvive al round-trip del suo serializzatore) —
è accettata anche la forma testuale `[^idx]` per chi scrive via API. `idx`
duplicati, note vuote o oltre 2000 caratteri → `400`. `PostOut.notes` le
riporta ordinate per `idx`. La resa (elenco numerato a piè di pagina +
tooltip sul marcatore) è del frontend; l'aggregato del blog è
`GET /blogs/{slug}/bibliography` (vedi sezione Blog).

Ogni nota accetta anche 9 campi facoltativi per bibliografie strutturate
compatibili BibTeX — `title`, `author`, `kind` (`"book" | "article" | "web" |
"note"`, `400` se altro valore), `source` (max 300, editore/rivista/sito),
`issued` (max 32, anno/data come stringa libera — non un tipo data: non
tutte le fonti hanno un anno o una data ISO completa), `isbn` (max 32), `doi`
(max 255), `url` (max 2000), `page` (max 32), tutti `string | null`,
assenti/vuoti equivalgono a `null` — nell'editor dietro un toggle "Aggiungi
dettagli bibliografici" nel modal "Nota" (stesso stile dell'avviso sui
contenuti sensibili delle immagini). `title`/`author` max 300 caratteri.
Nessuno di questi è mai obbligatorio, solo `content` lo è. Propagati anche
alla nota di libreria del blog che questa nota aggancia (B8, sotto), **solo
alla creazione**: se la libreria ha già una nota con lo stesso testo
normalizzato, i suoi campi non vengono mai sovrascritti da qui (si preserva
un'eventuale modifica fatta dalla libreria stessa); `kind`/`url` indicati
direttamente qui hanno la precedenza sulle euristiche `guess_kind()`/
`extract_url()` usate come fallback quando assenti. `GET
/blogs/{slug}/bibliography` riporta questi stessi 9 campi per ogni voce
aggregata.

Stati: `draft` → (opzionale) `pending_review` → `published`. `published_at`
serve anche per la pianificazione: un post con `status=published` e
`published_at` nel futuro non è ancora pubblicamente visibile — vedi
`is_publicly_visible` più sotto.

`is_hidden` (`PostOut`): moderazione da parte di un admin di piattaforma
(`admin/moderazione`, `PATCH /api/v1/admin/posts/{id}`) — se `true`, il
post è irraggiungibile pubblicamente indipendentemente da `status`, per
l'autore incluso (stesso trattamento 404 di un post inesistente per chi non
ha accesso in scrittura — vedi `app/domain/authorization.py::is_publicly_visible`).
Mai impostabile dall'autore.

Ogni post ha un **permalink leggibile**, senza UUID: `blog_slug` e
`permalink` sono calcolati ad ogni risposta (non colonne del modello) e
inclusi in ogni `PostOut`:

```text
permalink = /{blog_slug}/{slug}
```

La data è quella di pubblicazione (`published_at`) se il post è pubblicato,
altrimenti quella di creazione (`created_at`) — permette comunque un link di
anteprima per una bozza, risolvibile solo da chi ha accesso in scrittura al
blog. La data non serve a garantire l'unicità (già data da `blog_id` +
`slug` + `locale`, vedi sopra): è solo una convenzione di leggibilità, in
stile WordPress. Vedi `app/domain/permalinks.py`.

**`POST /api/v1/blogs/{blog_slug}/posts`** — richiede sessione ed essere
proprietario del blog oppure avere membership con ruolo `autore` o
`co_autore` (`403` altrimenti). Crea un post in stato `draft`, radice di una
nuova "famiglia" di traduzioni (`translation_group_id` = un nuovo UUID).
Accoda anche un backup su S3 (vedi sezione "Media e backup" più sotto).

```json
{"slug": "primo-post", "title": "...", "content": "# Markdown...", "locale": null, "cover_image_url": null, "tags": null, "category_id": null, "notes": null}
```

Il nome pubblico dell'autore **non** è indicato dal client (todo/USERS.md #2):
è calcolato come primo valore applicabile in questo ordine:

1. alias del collaboratore su *questo* blog (`PATCH /blogs/{slug}/my-membership`);
2. `default_author_display_name` del blog.
   Se uno di questi due esiste è **imposto**, senza possibilità di override;
3. altrimenti la preferenza del profilo `post_author_name_style` (vedi sezione
   Utenti): `full_name` (nome e cognome), `display_name` (alias globale),
   `verified_domain` (dominio personalizzato verificato) o `username`
   (default).

`PostOut.author_avatar_url` è invece sempre l'avatar dell'autore vero e
proprio (`null` se non impostato) — indipendente dal nome mostrato sopra,
che può essere un alias: non esiste un "avatar del blog" sostitutivo per un
post.

Il valore è salvato in `PostOut.author_display_name` alla scrittura del
post, ma **ricalcolato di nuovo ad ogni lettura**: cambiare l'alias del blog
o della membership, o rinominare l'utente, si riflette subito su tutti i
post già scritti, non solo su quelli risalvati. Risalvando comunque il post
(`PATCH /posts/{id}`) l'autore stesso ne riallinea anche la colonna
salvata; un altro utente che modifica il post (es. un revisore) non la
tocca — irrilevante per la risposta, che è comunque ricalcolata.

`locale` è opzionale: se omesso usa il `default_locale` del blog. `cover_image_url` è opzionale: l'URL ritornato da un precedente upload
su `POST /blogs/{slug}/media` (vedi sotto) — il campo accetta qualsiasi
stringa, non verifica che punti davvero a un media caricato su questo blog.
`cover_image_is_sensitive` (default `false`) riprende l'esito della
moderazione automatica ricevuto in quella stessa risposta di upload — vedi
sezione "Moderazione automatica delle immagini" più sotto; non viene
ricalcolato qui. `cover_image_categories` (default `[]`) sono le categorie
di avviso sui contenuti scelte manualmente dall'autore (vedi "Avviso sui
contenuti" più sotto): non vuoto forza anche `cover_image_is_sensitive` a
`true`, indipendentemente dal valore passato per quel campo.
`cover_image_alt_text` (default `""`) è il testo alternativo della cover
(accessibilità), indipendente dall'eventuale alt text della stessa immagine
in libreria media. `tags` è opzionale (vedi sezione "Tag" sotto).
`category_id` è opzionale: l'UUID di una categoria esistente del blog (vedi
sezione "Categorie" sopra) — `404` se non appartiene a questo blog. `409` se
lo slug è già in uso su quel blog per quella lingua. `notes` è opzionale
(vedi "Note a piè di pagina" sopra).

### Tag

Massimo **5 tag per post**, che vengano dal campo dedicato (`tags` nel
payload di create/update) o da `#hashtag` scritti nel testo del post — i due
canali si sommano nello stesso insieme, deduplicati. Un tag è normalizzato
(minuscolo, senza `#`, solo lettere/cifre/trattini singoli, max 30
caratteri): `"#Poesia"` e `" poesia "` diventano entrambi `"poesia"`. Un tag
malformato nel campo dedicato è un errore esplicito (`400`); un hashtag
malformato nel testo libero viene semplicemente ignorato, non fa fallire il
salvataggio. Superare il limite di 5 (dedicato + testo insieme) è sempre un
errore esplicito (`400`) — mai un troncamento silenzioso. Vedi
`app/domain/tags.py`.

Ogni `PostOut` espone due campi:

- `manual_tags` — solo quelli inseriti nel campo dedicato (per
  ripresentarli in un form di modifica).
- `tags` — l'insieme effettivo (`manual_tags` + hashtag nel testo), quello
  da mostrare in lettura e usato per il filtro `?tag=` del feed e per la
  sezione di tendenza (vedi sezione "Feed" più sotto).

Ogni `PostOut` espone anche `category`: `null`, oppure
`{"id", "name", "slug"}` della categoria del blog assegnata al post (vedi
sezione "Categorie" sopra) — a differenza dei tag, un post ha al più una
categoria.

In `PATCH /posts/{post_id}` (vedi sotto): `tags` assente lascia invariati i
tag del campo dedicato; una lista (anche vuota, `[]`) li sostituisce. Gli
hashtag nel testo vengono invece ricalcolati **ad ogni modifica del
contenuto**, a prescindere da questo campo.

**`POST /api/v1/posts/{post_id}/translations`** — stessa autorizzazione della
creazione. Aggiunge una traduzione alla stessa famiglia del post indicato
(`translation_group_id` condiviso), come post `draft` indipendente con il
proprio slug:

```json
{"slug": "my-post", "locale": "en", "title": "...", "content": "...", "category_id": null, "notes": null}
```

`409` se esiste già una traduzione per quella lingua nella famiglia, o se lo
slug è già in uso su quel blog per quella lingua. `category_id` è opzionale:
se omesso la traduzione eredita la categoria del post originale; se passato
esplicitamente (anche `null`, per non assegnarne una) sovrascrive
l'eredità. `notes` è opzionale e **non** viene ereditato dall'originale: una
traduzione ha le proprie note.

**`GET /api/v1/posts/{post_id}/translations`** — pubblico. Lista
`{id, locale, slug, status}` di tutte le traduzioni **pubblicamente visibili**
della stessa famiglia — pensato per costruire un selettore di lingua lato
frontend.

**`GET /api/v1/blogs/{blog_slug}/posts`** — pubblico (token opzionale): senza
sessione, o senza accesso in scrittura al blog, solo i post pubblicamente
visibili (pubblicati e non pianificati nel futuro). Con sessione e accesso in
scrittura (proprietario/autore/co-autore), tutti i post — bozze, in
revisione, pianificati inclusi — necessario per la dashboard autore. Query
param opzionale `?locale=it` per filtrare una sola lingua; omesso, ritorna
tutte le lingue. Segue anche la `visibility` del blog: `404` se il chiamante
non può vedere un blog `members`/`private` (vedi sezione Blog). Stessa regola
per `GET /posts/{post_id}` e per il permalink qui sotto.

**`GET /api/v1/posts/{post_id}`** — se pubblicamente visibile, pubblico.
Altrimenti richiede di avere accesso in scrittura al blog (stessa regola
della creazione); `404` (non `403`, per non rivelarne l'esistenza) se non
autorizzato. Uso interno (dashboard/editor): identifica il post per UUID,
non per il permalink pubblico.

**`GET /api/v1/blogs/{blog_slug}/posts/{post_slug}`** — pubblico
(token opzionale), stesse regole di visibilità di `GET /posts/{post_id}`
sopra. Risolve il permalink leggibile (vedi sopra) verso il post: è
l'endpoint pensato per la pagina pubblica del post (es.
`https://notturni.eu/{blog_slug}/{post_slug}`), che così non deve mai
esporre l'UUID nell'URL. `404` se blog/slug non corrispondono a nessun post
(o a un post non visibile per il chiamante).

**`PATCH /api/v1/posts/{post_id}`** — stessa autorizzazione della creazione.
Aggiorna
`title`/`content`/`cover_image_url`/`cover_image_is_sensitive`/`cover_image_categories`/`cover_image_alt_text`/`tags`/`category_id`/`notes`
(tutti opzionali). `cover_image_alt_text` è indipendente da `cover_image_url`
(stesso principio di `cover_image_categories` sotto): campo assente lascia
l'alt text invariato, presente (anche `null`/`""`) lo azzera o sostituisce.
Se `content` cambia, accoda di nuovo il backup su S3 e
ricalcola anche i media/link citati (vedi "Avviso sui contenuti" e
"Media e link citati" più sotto). Per `notes`: campo assente lascia le note
invariate, una lista (anche vuota `[]`) le sostituisce. Per
`cover_image_url`: valore assente (`null`/campo omesso) lascia la cover
invariata, stringa vuota `""` la rimuove (azzerando anche
`cover_image_is_sensitive`/`cover_image_categories`, indipendentemente da
cosa viene passato per quei campi), qualsiasi altro valore la sostituisce
insieme a `cover_image_categories` nella stessa richiesta (assente in quel
caso → `[]`, coerente con `cover_image_is_sensitive`: una cover nuova parte
sempre senza avviso a meno di dirlo esplicitamente). Per
`cover_image_categories` **senza** un nuovo `cover_image_url` nella stessa
richiesta — a differenza di `cover_image_is_sensitive`, che in quel caso
resta invariato — assente lascia le categorie di una cover già esistente
invariate, una lista (anche vuota) le sostituisce, forzando
`cover_image_is_sensitive` a `true` se non vuota: permette al modal di
avviso sui contenuti di aggiornare una cover già caricata senza doverla
ricaricare. Per `tags`, vedi sezione "Tag" sopra. Per `category_id`: campo
assente lascia la categoria invariata, `null` esplicito la rimuove, un UUID
valido la sostituisce (`404` se non appartiene a questo blog) — a
differenza di `cover_image_url` non esiste un valore "vuoto" per un UUID, da
cui la distinzione esplicita assente/`null`/valore. Accetta anche
`search_indexing_enabled`/`ai_crawling_enabled` (bool, tri-stato come
`comments_mode`: campo assente non tocca, `null` esplicito torna a ereditare
da `Blog.search_indexing_enabled`/`ai_crawling_enabled`, un valore imposta un
override per questo solo post) — non può "riaprire" un crawler già escluso a
livello di blog, solo restringerlo ulteriormente (vedi
`app/domain/seo.py::effective_search_indexing`/`effective_ai_crawling`).
`PostOut` riporta sia il valore grezzo (`null` = eredita) sia
`effective_search_indexing_enabled`/`effective_ai_crawling_enabled` (sempre
valorizzati, già tengono conto del blocco a cascata).

### Avviso sui contenuti

Oltre alla segnalazione automatica (vedi "Moderazione automatica delle
immagini" più sotto), l'autore può impostare manualmente un avviso sulle
immagini con un modal stile Bluesky: **Suggestivo** (`suggestive`),
**Nudità** (`nudity`), **Esplicito** (`explicit`), **Contenuto sensibile**
(`other`) — `app/domain/content_media.py::SENSITIVITY_CATEGORIES`. Per le
immagini nel corpo del post viaggia nello stesso `title` Markdown già usato
per "sensitive" (vedi sotto): `![alt](url "sensitive")` per la sola
segnalazione automatica (categoria non nota), `![alt](url "sensitive:nudity,explicit")`
per una scelta esplicita — nessun campo dedicato, l'editor riscrive
direttamente il `title` dell'immagine. Selezionare zero categorie nel modal
rimuove del tutto l'avviso, anche se messo dall'automoderazione: la scelta
finale è sempre dell'autore. Per la cover, vedi `cover_image_categories`
sopra.

### Media e link citati

CLAUDE.md: come i tag, media (oggi solo immagini) e link nel corpo del post
sono estratti dal Markdown e materializzati in tabelle di sola lettura
(`post_media`/`post_links`) ad ogni salvataggio — non un elenco indicato a
parte dal client. Servono alla bibliografia aggregata del blog: vedi
`GET /api/v1/blogs/{slug}/media-bibliography` e `.../links-bibliography`
nella sezione Blog.

**`POST /api/v1/posts/{post_id}/submit-for-review`** — proprietario/autore/
co-autore. Sposta un post da `draft` a `pending_review` (`400` se non era in
`draft`).

**`POST /api/v1/posts/{post_id}/return-to-draft`** — proprietario o
**revisore** (ruolo di blog — vedi
[ROADMAP.md](../ROADMAP.md#1-prodotto-e-regole-di-dominio)). Rimanda un post
da `pending_review` a `draft` (rifiuto della
revisione); `400` se non era in `pending_review`, `403` se il chiamante non è
proprietario né revisore.

**`POST /api/v1/posts/{post_id}/publish`** — pubblica subito, oppure pianifica
se si passa una data futura:

```json
{"published_at": "2026-09-01T10:00:00Z"}
```

(corpo interamente opzionale: senza payload, pubblica subito). Consentito a
proprietario/autore/co-autore da **qualsiasi** stato; un **revisore** può
approvare solo da `pending_review` (altrimenti `403`) — è il modo in cui
approva una revisione invece di rifiutarla con `return-to-draft`.
Idempotente se già pubblicato e non si passa un nuovo `published_at`
(`published_at` esistente non viene toccato); passare una nuova data lo
sovrascrive sempre, anche per ripianificare un post già pubblicato.

## Pubblicazioni (todo/PUBLICATIONS.md, todo/UX_REDESIGN.md B9, mockup 2d/3g)

Una pubblicazione raccoglie post di un blog come capitoli sotto
`/{blog}/pub/{name}`: ordine cronologico dal più vecchio, oppure esplicito
(`Post.chapter_order`). Un post appartiene al più a una pubblicazione:
`POST /blogs/{slug}/posts`, `POST /posts/{id}/translations` e
`PATCH /posts/{id}` accettano `publication_id` (tri-state come
`category_id`; `400` se non è del blog; cambiare pubblicazione azzera
l'ordine). `PostOut` include `publication: {id, name, title} | null` e
`chapter_order`.

**`GET /api/v1/blogs/{slug}/publications`** — segue la visibilità del blog.
`[{id, name, title, description, chapters_total, chapters_published,
created_at}]`; i lettori vedono solo quelle con almeno un capitolo
pubblicato, chi ha accesso in scrittura tutte.

**`GET /api/v1/blogs/{slug}/publications/{name|id}`** — indice: come sopra
più `chapters: [{n, post_id, slug, title, locale, status, published_at,
permalink, reading_minutes, is_public}]`. Per i lettori solo capitoli
pubblicati (`404` se nessuno); chi scrive vede anche bozze/pianificati con
`is_public=false`.

**`POST /api/v1/blogs/{slug}/publications`** — `{name, title, description?}`
(accesso in scrittura; `name` = segmento URL, minuscole/numeri/trattini,
`409` se già usato). **`PATCH .../{name|id}`** — stessi campi.
**`DELETE .../{name|id}`** — `204`, i post restano senza pubblicazione.

**`PUT /api/v1/blogs/{slug}/publications/{name|id}/order`** — `{post_ids}`
nell'ordine voluto (mockup 3g drag-to-order); i post non elencati seguono
in coda in ordine cronologico; `400` se un id non è della pubblicazione.

## Libreria note del blog (todo/UX_REDESIGN.md B8, mockup 3b)

Le note a piè di pagina dei post (`post_notes`) vengono agganciate, al
salvataggio del post, a una **nota del blog** (`blog_notes`) con lo stesso
testo normalizzato (minuscolo, senza punteggiatura/apostrofi): la nota è
creata se manca, con `kind`/`url` indicati direttamente sulla nota di post
se presenti, altrimenti stimati (`guess_kind()`, `book` | `article` | `web` |
`note`; `extract_url()`, DOI → `https://doi.org/…`). `GET
/blogs/{slug}/bibliography` espone `kind`/`url` più gli stessi campi
bibliografici di `post_notes` (vedi sopra) per ogni voce.

**`GET /api/v1/blogs/{slug}/notes?q=`** — proprietario e collaboratori.
`[{id, content, kind, url, title, author, source, issued, isbn, doi, page,
created_at, updated_at, used_in: [{post_id, post_slug, post_title, idx}],
possible_duplicates: [id, …]}]` dalla più recente; `possible_duplicates` =
note dello stesso blog con gli stessi primi 30 caratteri normalizzati.
`title`/`author`/`source`/`issued`/`isbn`/`doi`/`page` sono scrivibili da
questa API (vedi `POST`/`PATCH` sotto) ma non ancora dalla UI della libreria
(`NotesTab.tsx`), che oggi edita solo `kind`/`url`/`content`.

**`POST /api/v1/blogs/{slug}/notes`** — `{content, kind?, url?, title?,
author?, source?, issued?, isbn?, doi?, page?}` (accesso in scrittura; `400`
se il tipo non è tra quelli previsti o l'URL non è http/https).

**`PATCH /api/v1/blogs/{slug}/notes/{id}`** — stessi campi di `POST`, tutti
opzionali; assente = non tocca il campo, stringa vuota = lo svuota. Cambiare
il testo lo aggiorna anche nei post che citano la nota.

**`DELETE /api/v1/blogs/{slug}/notes/{id}`** — `204`; `409` se citata in
un post.

**`POST /api/v1/blogs/{slug}/notes/{id}/merge`** — `{into_id}`: le citazioni
della nota passano alla destinazione (testo compreso), la sorgente viene
eliminata. Risponde con la nota di destinazione.

**`GET /api/v1/blogs/{slug}/notes/export.bib`** — BibTeX
(`application/x-bibtex`): una voce `@book`/`@article`/`@misc` per nota, con
i campi strutturati quando presenti (`title`, `author`, `publisher`-o-
`journal`-a-seconda-del-tipo per `source`, `year` per `issued` — stimato dal
testo se assente, per compatibilità con le note create prima di questo
campo —, `isbn`, `doi`, `url`) più sempre `note` col testo completo.

**`POST /api/v1/blogs/{slug}/notes/import`** — `{bibtex}`: parser minimale
che rilegge gli stessi campi strutturati sopra (`author`/`title`/`journal`-
o-`publisher`/`year`/`isbn`/`doi`, oppure `note` per il testo libero;
`url`/`doi`); le voci già presenti (testo normalizzato) vengono saltate.
`201` con le note create; `400` se non riconosce nessuna voce.

## Media e backup

Backend di storage selezionabile via `NOCT_STORAGE_BACKEND`: `s3` (default,
MinIO/AWS S3/qualunque endpoint S3-compatible) oppure `localstorage`
(filesystem locale, servito dal backend stesso su `/storage` — pensato per
installazioni `solo` senza storage S3). Tutto sotto un unico bucket/namespace
(`NOCT_S3_BUCKET_CONTENT`), con un prefisso
`{NOCT_SITE_SLUG}/userdata/{user_uuid}/{blog_uuid}/...` — pensato per poter
condividere lo stesso bucket fisico (o la stessa directory, sul backend
`localstorage`) tra più installazioni/scopi senza collisioni.

- **`.../media/{uuid}.{ext}`** — immagini caricate via `POST
  /blogs/{slug}/media` (sezione Blog sopra). Stesso endpoint sia per le
  immagini incorporate nel Markdown del contenuto (`![alt](url)`) sia per
  l'immagine di copertina di un post (`Post.cover_image_url`, impostata poi
  separatamente con `PATCH /posts/{post_id}`): l'upload non distingue i due
  usi, è il chiamante a decidere dove usare l'URL ritornato. **Pubblico in
  lettura**: sono pensate per essere incorporate nei post e servite ai
  visitatori. Bucket policy scoped solo a questo prefisso (`.../media/*`),
  non all'intero bucket.
- **`.../posts/{post_uuid}.md`** — copia di backup/fallback del Markdown di
  ogni post, scritta da `app/workers/post_backup_consumer.py` (consumer reale
  e funzionante, non un placeholder) ogni volta che un post viene creato o il
  suo contenuto modificato (accodato su RabbitMQ, coda `post_backup` — un
  problema di RabbitMQ/S3 non fa mai fallire il salvataggio del post: il
  database resta la fonte di verità, questa è solo una copia di sicurezza).
  **Privato** — non pensato per essere servito direttamente ai visitatori.

Il worker va avviato separatamente (`python -m
app.workers.post_backup_consumer`, già presente in `compose.yaml` come
servizio `worker-post-backup`); senza, i messaggi restano semplicemente in
coda finché il worker non viene avviato.

### Moderazione automatica delle immagini

Ogni upload via `POST /blogs/{slug}/media` passa (in modo sincrono, prima
della risposta) per un classificatore NSFW self-hosted — servizio separato
containerizzato in `moderation/` (nessuna immagine lascia mai
l'infrastruttura: coerente con l'impostazione EU-centrica/GDPR, vedi
CLAUDE.md), chiamato internamente via `NOCT_MODERATION_SERVICE_URL`
(impostato automaticamente da `compose.yaml`). La risposta include
`is_sensitive: bool`:

```json
{"url": "https://.../media/{uuid}.png", "is_sensitive": false}
```

Il chiamante decide cosa farne — per un'immagine nel contenuto, l'editor la
inserisce come `![alt](url "sensitive")` (il `title` è la convenzione con
cui l'informazione viaggia nel Markdown stesso, senza bisogno di una
tabella dedicata: vedi `frontend/src/lib/markdown.ts`, che la rende sfocata
e cliccabile per rivelarla); per la cover di un post, il flag va passato
esplicitamente come `cover_image_is_sensitive` in creazione/modifica (vedi
sezione Post) — non viene ricalcolato lato server in quel momento.

**Fail open**: se il servizio di moderazione non è raggiungibile, non
risponde in tempo, o `NOCT_MODERATION_SERVICE_URL` non è impostato,
`is_sensitive` è sempre `false` — un problema di questo servizio ausiliario
non deve mai far fallire un upload altrimenti riuscito (stesso principio
già in atto per il backup dei post su S3). Non è pensato come barriera di
sicurezza legale, solo come aiuto automatico all'autore.

## Libreria media del blog (todo/UX_REDESIGN.md B7, mockup 3c)

`POST /api/v1/blogs/{slug}/media` (upload, vedi sopra) ora registra ogni
immagine in `media_files` e risponde anche con `media_id`.

**`GET /api/v1/blogs/{slug}/media`** — proprietario e collaboratori
(`403` altrimenti). `{items: [{id, url, content_type, size_bytes, alt_text,
caption, categories, is_sensitive, uploader_username, created_at, used_in:
[{post_id, post_slug, post_title, permalink}], used_as_blog_cover}],
total_bytes}`, dal più recente. `used_in` copre sia le immagini citate nel
contenuto (`post_media`) sia quelle usate come cover di un post
(`Post.cover_image_url`) — prima tracciava solo le prime, quindi
un'immagine usata solo come cover risultava "non usata da nessuno" e
cancellabile mentre era ancora la cover live del post (bug corretto).
`used_as_blog_cover` è `true` se l'immagine è l'attuale cover del blog
(`Blog.cover_image_url`, sezione cover-image sopra) — anch'essa ora sempre
registrata qui all'upload.

**`POST /api/v1/blogs/{slug}/media/sync`** — accesso in scrittura. Importa
nella libreria le immagini citate nei post che non hanno ancora una riga
(blog precedenti alla libreria): `size_bytes=0`, senza caricatore.
Idempotente; risponde come `GET`.

**`PATCH /api/v1/blogs/{slug}/media/{media_id}`** — `{alt_text?, caption?,
categories?}` (categorie tra quelle di `SENSITIVITY_CATEGORIES`, `400`
altrimenti). I post già scritti portano alt e avviso nel proprio Markdown e
non vengono riscritti: i valori della libreria sono il default per gli usi
futuri.

**`DELETE /api/v1/blogs/{slug}/media/{media_id}`** — `204`; `409` se
l'immagine è ancora citata in un post (contenuto o cover) o è l'attuale
cover del blog. Rimuove la riga e, se caricata via libreria, l'oggetto su
storage.

## Anteprima di un link

**`GET /api/v1/link-preview?url=<url>`** — pubblico, nessuna autenticazione
(CLAUDE.md: serve anche al rendering della pagina pubblica del post, non
solo all'editor). Recupera titolo/descrizione/immagine Open Graph di una
pagina esterna:

```json
{"url": "https://esempio.org/articolo", "title": "...", "description": "...", "image": "https://.../cover.jpg"}
```

`title`/`description`/`image` sono `null` se non trovati nell'HTML della
pagina, o se il fetch fallisce (sito irraggiungibile, timeout, redirect —
i redirect non vengono seguiti automaticamente) — in quel caso la risposta
resta comunque `200` con solo `url` valorizzato, mai un errore, così chi
chiama può sempre mostrare un link semplice. `400` solo per un URL non
idoneo in partenza: schema diverso da `http`/`https`, o il cui hostname
risolve a un indirizzo privato/loopback/link-local/riservato (mitigazione
SSRF — `app/domain/link_preview.py::validate_previewable_url`; non è una
barriera assoluta, stesso principio di "aiuto best-effort" già in atto per
la moderazione automatica delle immagini sopra). `429` oltre 30
richieste/minuto dallo stesso IP (rate limiting via Redis,
`app/domain/rate_limit.py` — mitiga l'uso di questo endpoint come
proxy/scanner verso terzi vista l'assenza di autenticazione; fail open se
Redis non è raggiungibile).

**Cache a due livelli** (`app/domain/link_preview.py::get_cached_or_fetch_link_preview`),
condivisa e deduplicata per URL — due post/utenti/blog che citano lo stesso
link fanno un solo fetch reale, non uno ciascuno: Redis come cache calda
(TTL 6h), la tabella `link_preview_cache` (una riga per URL, unique su
`url_hash` = sha256 dell'URL) come fonte persistente che sopravvive a un
riavvio/svuotamento di Redis. Un'anteprima con dati Open Graph reali resta
valida una settimana prima di essere riverificata dal vivo; un fallimento
(host irraggiungibile, non HTML, nessun meta tag, ...) solo 6 ore, per non
restare bloccati più del necessario ma senza martellare un host che non
risponde mai. Il fetch dal vivo resta come prima (timeout 8s, corpo troncato
a ~3 MB o alla chiusura di `</head>`, redirect non seguiti).

Usato dall'editor (`frontend/src/components/editor/LinkPreviewCard.tsx`)
quando si incolla un URL da solo: il link resta testo semplice/cancellabile,
la card è un blocco indipendente subito sotto, salvato nel Markdown come un
link con `title="card"` (`[url](url "card")`, stessa convenzione di
"sensitive" sulle immagini) — senza salvare uno snapshot di
titolo/descrizione/immagine, richiesti di nuovo ad ogni apertura
dell'editor o rendering della pagina pubblica.

## Commenti

Chi può commentare è governato da `comments_mode` (`everyone` | `members` |
`closed`), di default `members`: a livello di blog (`PATCH /api/v1/blogs/{slug}`)
con un override opzionale per singolo post (`PATCH /api/v1/posts/{post_id}`,
`comments_mode: null` torna a ereditare dal blog). Un thread è supportato con
un solo livello: `Comment.parent_id` punta a un commento di *primo livello*
dello stesso post (`400` se punta a una risposta o a un commento di un altro
post) — mantiene le conversazioni leggibili senza limitare la profondità a
livello di schema.

**`POST /api/v1/posts/{post_id}/comments`** — autenticazione opzionale.
`comments_mode` effettivo = quello del post se impostato, altrimenti quello
del blog. `closed`: `403` per chiunque, anche un utente registrato.

- **con sessione valida** (qualunque `comments_mode` tranne `closed`):
  commento attribuito all'utente, stato `approved` automaticamente (nessuna
  moderazione per utenti registrati).
  ```json
  {"content": "...", "parent_id": "..."}
  ```
  `parent_id` opzionale (risposta a un commento di primo livello dello
  stesso post). `author_display_name` nella risposta segue la preferenza di
  profilo `post_author_name_style` (username/nome e cognome/alias
  globale/dominio verificato — non l'alias di blog, che si applica solo ai
  post: un commento resta sempre a nome della persona, non del blog) ed è
  **ricalcolato ad ogni lettura**, non solo alla creazione: un cambio di
  username o di preferenza si riflette subito anche sui commenti passati.
  `author_avatar_url` (sempre l'avatar vero dell'autore, non un alias) segue
  la stessa logica: `null` per i commenti anonimi.
- **senza sessione, `comments_mode="members"`:** `401`.
- **senza sessione, `comments_mode="everyone"`:** richiede
  `author_display_name`/`author_email` (altrimenti `400`) e un
  `captcha_token` valido del widget Cloudflare Turnstile, verificato
  server-side (`app/core/captcha.py`) — `400` se mancante o non valido
  (fail closed: un servizio Turnstile irraggiungibile blocca il commento).
  Il commento è comunque creato in stato `pending`.
  ```json
  {
    "content": "...",
    "author_display_name": "...",
    "author_email": "...",
    "captcha_token": "..."
  }
  ```

**`GET /api/v1/posts/{post_id}/comments`** — pubblico. Solo commenti
`approved`.

**`GET /api/v1/posts/{post_id}/comments/pending`** — richiede sessione ed
essere proprietario del blog, avere membership con ruolo `mediatore`, oppure
avere `platform_role` Amministratore/Super Admin/Moderatore (`403`
altrimenti — `app/domain/authorization.py::can_moderate_comments`). Coda di
moderazione del singolo post.

**`GET /api/v1/blogs/{blog_slug}/comments`** — stessa autorizzazione di
`pending`. Commenti di *tutti* i post del blog con lo stato indicato dal
parametro opzionale `status` (`pending` di default, oppure `approved` /
`rejected`), dal più recente. Ogni elemento ha in più `post_title` e
`post_slug`. Serve alla moderazione per-blog nel dashboard senza una
richiesta per ogni post — per la versione trasversale su tutti i blog vedi
`GET /api/v1/admin/comments` più sotto.

**`POST /api/v1/comments/{comment_id}/approve`** / **`.../reject`** — stessa
autorizzazione di `pending` (quindi utilizzabili anche da Amministratore/
Super Admin/Moderatore su un commento di un blog di cui non hanno nessuna
membership). Cambiano lo stato del commento.

### Coda estesa (todo/UX_REDESIGN.md B4, mockup 5b)

`GET /api/v1/blogs/{blog_slug}/comments` accetta anche `reported=true`
(commenti segnalati alla piattaforma, qualunque stato). Ogni commento
espone `reported_to_platform` e `report_note`. Lo stato `rejected` è il
"nascosto" della coda (nessuno stato nuovo).

**`POST /api/v1/comments/{comment_id}/report`** — `{note}` obbligatoria
(`400` se vuota), stessa autorizzazione di `approve`. Segnala il commento ai
moderatori di piattaforma: compare in `GET /api/v1/admin/comments?reported=true`
con `report_note`/`reported_at`. Audit `comment.reported`.

**`POST /api/v1/comments/{comment_id}/block-author`** — `{note?}`, stessa
autorizzazione. Aggiunge l'autore alla lista dei bloccati del blog e porta il
commento a `rejected`. Utente registrato → `user_id`; commento anonimo →
sha256 dell'email (mai l'email in chiaro né l'IP). `400` se l'autore è il
proprietario del blog o non è identificabile. Audit `comment.author_blocked`.
Un autore bloccato riceve `403` su `POST /posts/{id}/comments` di quel blog.

**`GET /api/v1/blogs/{blog_slug}/blocked`** / **`DELETE .../blocked/{block_id}`**
— stessa autorizzazione. Lista `{id, label, is_anonymous, note, created_at}`
(`label` è `@username` o il nome libero dell'anonimo) e sblocco (`204`).

**Chiusura automatica**: `PATCH /api/v1/blogs/{slug}` accetta
`comments_auto_close_days` (intero ≥ 0, `0`/`null` = mai): trascorsi N
giorni da `published_at`, `effective_comments_mode` del post diventa
`closed` (`403` ai nuovi commenti, i già scritti restano visibili).

## Segnalazioni (todo/UX_REDESIGN.md B5)

**`POST /api/v1/blogs/{slug}/report`** / **`POST /api/v1/posts/{post_id}/report`**
— richiede sessione. `{reason: spam | abuse | illegal | other, note?}`
(nota max 500 caratteri). Segnala un blog o un post pubblicamente
visibile ai moderatori di piattaforma: `201` con `{id, target_type,
target_id, reason, note, status, created_at}`. Una sola segnalazione per
lettore e bersaglio (una seconda richiesta ritorna la prima, `201`); `400`
se si segnala un proprio contenuto; `404` per contenuti non pubblici;
`429` oltre 10 segnalazioni l'ora per utente. Le segnalazioni aperte
compaiono nel pannello admin (`GET /api/v1/admin/blogs/{id}/reports`) e
vengono chiuse dalle azioni admin.

## Frammenti

Porzione di testo evidenziata dal lettore (con il mouse) su un post
pubblicato e salvata in una raccolta personale unificata — non legata
all'autore del post, che non deve fare nulla per abilitarla. Richiede sempre
sessione: un frammento appartiene a chi lo ha salvato. Pubblico o privato a
livello di **utente** (`is_public`, scelto al salvataggio e modificabile
ex-post) — non di blog: pubblico = visibile ad altri utenti iscritti alla
piattaforma, mai a visitatori anonimi; privato (default) = visibile solo a
chi lo ha salvato. Il testo è salvato così com'è (non un offset nel post):
il frontend lo ri-cerca nel testo reso ad ogni lettura per ri-evidenziarlo
(`lib/highlight-fragments.ts`), tollerando piccole differenze di spazi
bianchi ma non un post riscritto nel frattempo — in quel caso il frammento
resta salvato ma non più ri-evidenziato in pagina. Ri-selezionare (anche
solo in parte) un frammento già evidenziato propone di rimuoverlo (`DELETE`)
invece di salvarne uno nuovo — interamente lato frontend
(`Range.intersectsNode` sui `<mark>` già presenti), nessun endpoint dedicato
oltre a `DELETE`/`PATCH` già descritti sotto.

**`POST /api/v1/posts/{post_id}/fragments`** — richiede sessione. `404` se il
post non è pubblicato o non è visibile all'utente (stessa regola d'accesso
del permalink pubblico). `400` se il testo è vuoto o supera il **15%** della
lunghezza del Markdown grezzo del post (`app/domain/fragments.py`) — un
proxy della lunghezza del testo reso, che il backend non calcola mai (nessun
rendering Markdown lato server, vedi Post). Salvare due volte lo stesso
identico testo sullo stesso post è idempotente: ritorna lo stesso frammento
(la sua visibilità corrente, non sovrascritta da `is_public` della seconda
richiesta), non un errore.

```json
{"text": "il pezzo di testo evidenziato", "is_public": false}
```

`is_public` opzionale, default `false`.

```json
{"id": "...", "post_id": "...", "text": "...", "is_public": false, "created_at": "..."}
```

**`GET /api/v1/posts/{post_id}/fragments`** — richiede sessione. Solo i
frammenti salvati dall'utente corrente su quel post (mai quelli di altri
utenti, indipendentemente da `is_public` — questo endpoint resta quello per
la ri-evidenziazione dei *propri* frammenti) — usato dal frontend per
ri-evidenziarli ad ogni lettura, indipendentemente dal fatto che si sia
arrivati al post dalla pagina di raccolta o da un link diretto.

**`PATCH /api/v1/fragments/{fragment_id}`** — richiede sessione ed essere il
proprietario del frammento (`404` altrimenti, come `DELETE` sotto). Cambia
la visibilità dopo il salvataggio.

```json
{"is_public": true}
```

**`GET /api/v1/users/me/fragments`** — richiede sessione. Raccolta unificata
di tutti i frammenti salvati dall'utente, più recenti prima: testo,
visibilità, data di cattura, titolo del post e permalink pubblico, nome
dell'autore. Quest'ultimo è il valore già risolto al salvataggio/ultima
modifica del post (`Post.author_display_name`), non ricalcolato da un alias
cambiato dopo come invece fa `PostOut` — semplificazione accettata per
questa vista derivata.

```json
[{"id": "...", "text": "...", "is_public": false, "created_at": "...", "post_title": "...", "author_display_name": "...", "permalink": "/blog/post"}]
```

**`DELETE /api/v1/fragments/{fragment_id}`** — richiede sessione ed essere il
proprietario del frammento (`404` altrimenti, non `403`: non rivela
l'esistenza del frammento a chi non è suo). `204` se rimosso.

### Viste aggregate su tutti i blog dell'utente

Cinque endpoint, tutti `GET /api/v1/users/me/...`, richiedono sessione,
nessuna paginazione (come le liste per-singolo-blog che generalizzano).
Ambito comune: **tutti i blog di cui l'utente è proprietario o
collaboratore** (`app/domain/blog_scope.py::my_blog_ids`, union tra
`Blog.owner_id` e `BlogMembership.user_id`, blog cancellati esclusi) — non
un solo blog per volta come le rispettive tab in `dashboard/blogs/{slug}`,
che restano l'unico modo per modificare/caricare/eliminare questi
contenuti (queste viste sono di sola lettura). Ogni elemento porta
`blog_slug`/`blog_title` per sapere da quale blog proviene.

**`GET /api/v1/users/me/posts`** — tutti i post (qualunque stato: bozza,
revisione, pubblicato, pianificato) di tutti i blog dell'utente, dal più
recente. Stesso schema di risposta di `GET /blogs/{slug}/posts` (`PostOut`,
già include `blog_slug`/`permalink`), generalizzato a più blog.

**`GET /api/v1/users/me/media`** — generalizza `GET /blogs/{slug}/media`
(sezione "Libreria media del blog" sotto), stesso schema per riga (alt,
didascalia, categorie, sensibilità, "usato in") più `blog_slug`/`blog_title`:

```json
[{
  "id": "...", "url": "...", "content_type": "image/jpeg", "size_bytes": 12345,
  "alt_text": "...", "caption": null, "categories": [], "is_sensitive": false,
  "uploader_username": "mario", "created_at": "...",
  "used_in": [{"post_id": "...", "post_slug": "...", "post_title": "...", "permalink": "/blog/post"}],
  "blog_slug": "...", "blog_title": "..."
}]
```

**`GET /api/v1/users/me/publications`** — generalizza `GET
/blogs/{slug}/publications` (sezione "Pubblicazioni" sotto), stessi
conteggi capitoli totali/pubblicati per pubblicazione, più
`blog_slug`/`blog_title`.

**`GET /api/v1/users/me/links`** — generalizza `GET
/blogs/{slug}/links-bibliography` (sezione "Anteprima di un link"/
bibliografia sotto) a più blog, raggruppato per URL identico su tutti i
post. **Vista autore**: a differenza dell'endpoint pubblico che generalizza,
non applica il filtro di visibilità pubblica — un autore vede qui anche i
link nelle proprie bozze. Ogni citazione porta `blog_slug`/`blog_title`.

```json
[{"url": "...", "link_text": "...", "citations": [
  {"post_title": "...", "post_slug": "...", "permalink": "...", "locale": "it", "used_at": "...", "blog_slug": "...", "blog_title": "..."}
]}]
```

**`GET /api/v1/users/me/bibliography`** — generalizza la bibliografia delle
note a piè di pagina (sezione "Libreria note del blog" sotto) a più blog,
raggruppate per testo nota identico, stessa vista-autore (nessun filtro di
visibilità pubblica) e stessa cautela sugli URL non http(s) su note create
prima della validazione di schema. Ogni citazione porta
`blog_slug`/`blog_title`.

## Pagine statiche (sito principale)

Pagine come Chi siamo, Contatti, Privacy — non legate a un blog utente
(`blog_id: null` nella risposta), gestite dal team della piattaforma, sempre
attive (a differenza delle pagine di blog, opt-in — vedi "Pagine statiche del
blog" più sotto). Stesso schema di traduzione dei post (sezione Multilingua
sopra). Permalink pubblico `/p/{slug}` (prefisso dedicato per non
collidere con gli slug dei blog raggiungibili senza sottodominio su
`/{blog_slug}/...`), riportato anche nel campo `permalink` della risposta.

**`POST /api/v1/pages`** — richiede sessione con `platform_role` in
`amministratore`/`super_admin` (`403` altrimenti). Crea una pagina, radice di
una nuova famiglia di traduzioni.

```json
{"slug": "chi-siamo", "locale": "it", "title": "Chi siamo", "content": "...", "is_published": true}
```

`409` se esiste già una pagina con quello slug in quella lingua.

**`POST /api/v1/pages/{page_id}/translations`** — stessa autorizzazione.
Aggiunge una traduzione alla stessa famiglia (stesso corpo di `POST /pages`,
senza `locale` implicito: va sempre indicato esplicitamente).

**`GET /api/v1/pages/{page_id}/translations`** — pubblico. Lista
`{id, locale, slug, is_published}` delle sole traduzioni pubblicate della
stessa famiglia, stesso pattern delle pagine di blog sopra e dei post.

**`GET /api/v1/p/{slug}?locale=it`** — pubblico (token opzionale): senza
sessione admin, solo pagine `is_published=true` (`404` altrimenti, bozza o
inesistente). Con sessione admin, anche le bozze — per poterle rivedere
prima di pubblicarle.

**`GET /api/v1/pages?locale=it`** — pubblico (token opzionale): stessa
distinzione pubblicate/tutte in base al ruolo del chiamante. Query param
opzionale `q`: filtra per titolo o slug (`ilike`, sottostringa), usato dalla
ricerca della sezione Pagine del dashboard (`frontend/src/app/admin/pagine`).

**`PATCH /api/v1/pages/{page_id}`** — richiede ruolo admin. Aggiorna una
singola traduzione (`slug`, `title`, `content`, `is_published`, tutti
opzionali).

## Interessi utente

**`GET /api/v1/interests`** — pubblico, nessuna autenticazione. Elenco
corrente degli interessi selezionabili (blocco "interessi utente", tag
fissi multilingua — chiave canonica non linguistica, mai testo libero):

```json
[{"key": "music", "translations": {"it": "Musica", "en": "Music"}}, ...]
```

Sola sorgente di verità: `platform_config.interests`, seminata da
`NOCT_DEFAULT_INTERESTS` (JSON, stesso schema) o da un elenco builtin
curato alla prima installazione, poi modificabile in qualsiasi momento da
un Super Admin (`PATCH /api/v1/admin/config`, campo `interests` — vedi
sezione Amministrazione). Il frontend risolve la traduzione nella lingua
corrente da sé (fallback `en`, poi la prima disponibile, poi `key`).

## Profilo utente e follow

**`GET /api/v1/users`** — pubblico, nessuna autenticazione. Directory
pubblica degli utenti (blocco "directory di utenti", stesso schema di
`GET /blogs`): solo account attivi (non anonimizzati/cancellati) che non
hanno scelto l'opt-out (`User.directory_listed`, vedi `PATCH /users/me`
sotto). Il Super Admin è **sempre** escluso, a prescindere dal proprio
`directory_listed` — non un'opzione dell'utente, per sicurezza (evitare che
l'account con i privilegi più ampi sia individuabile dalla directory
pubblica). `q` cerca in username/alias pubblico/bio (`ILIKE`), `locale` filtra
per lingua madre (`native_language`), `interest` filtra per chiave canonica
di interesse (per trovare persone con cui condividerlo e seguirle), `sort`
è `new` (registrazione, default) o `followers`, `limit` (default 30,
massimo 100), `offset`. Voce:

```json
{
  "username": "...", "display_name": "...", "bio": "...",
  "avatar_url": "...", "verification_tier": "none", "custom_domain": null,
  "interests": ["music", "cinema"], "follower_count": 3
}
```

**`GET /api/v1/users/{username}`** — pubblico. Profilo pubblico:

```json
{
  "username": "...", "bio": "...",
  "first_name": "...", "last_name": "...", "display_name": "...",
  "post_author_name_style": "username",
  "country": "IT", "native_language": "it", "fallback_languages": ["en", "fr"],
  "interests": ["music", "cinema"],
  "avatar_url": "...", "social_links": [...], "created_at": "...",
  "verification_tier": "none", "custom_domain": null,
  "atproto_did": "did:web:notturni.eu:users:<uuid>",
  "activitypub_actor_id": "https://notturni.eu/ap/actors/<uuid>"
}
```

`verification_tier` (`none`|`bronze`|`silver`|`gold`|`blue`): sigillo di
verifica del profilo, stile Bluesky/Instagram/Twitter — `bronze` da dominio
custom verificato via DNS (vedi sotto), `gold`/`silver`/`blue` da elenchi/
domini gestiti a mano da un Super Admin (`PATCH /admin/config`, vedi
sezione Amministrazione), ricalcolati da `app/domain/verification.py`.
`custom_domain` è valorizzato solo
se un dominio custom è stato verificato con successo (mai per uno stato
`pending`/`failed`) — lo username di piattaforma resta comunque sempre
citabile/risolvibile, il dominio è un'aggiunta, non una sostituzione a
livello di routing/permalink. `atproto_did`/`activitypub_actor_id` sono
identificativi **placeholder** per un'eventuale federazione futura (AT
Protocol/Bluesky, poi ActivityPub/Mastodon, ROADMAP.md §5): calcolati al volo
da `NOCT_INSTANCE_FQDN` + l'id UUID dell'utente (mai dallo username, per
restare stabili anche se questo cambia), non persistiti, non federati
realmente — nessun endpoint `/ap/...`/WebFinger servito, solo la stringa
mostrata nel profilo (`app/domain/fediverse.py`).

`{username}` in questo endpoint (e in tutti gli altri `GET
/api/v1/users/{username}/...` sotto, incluso follow/unfollow) accetta anche
un dominio custom verificato al posto dello username: se non trova
corrispondenza esatta su `username`, ritenta su `custom_domains.domain`
(solo stato `verified`) prima del 404 (`_find_user_by_username_or_domain` in
`app/api/v1/users.py`). Lo username resta comunque sempre risolvibile: il
dominio è un identificativo aggiuntivo, non esclusivo.

**`GET /api/v1/users/me`** — richiede sessione. Come sopra ma con i campi
privati del proprietario, mai esposti sul profilo pubblico di nessuno:

```json
{
  "...": "tutti i campi di GET /users/{username}",
  "email": "...",
  "username_changed_at": "2026-09-10T12:00:00Z",
  "next_username_change_allowed_at": "2026-09-15T12:00:00Z",
  "pending_email_change": {"new_email": "...", "stage": "awaiting_old_confirmation"},
  "domain_pending_verification": "...",
  "domain_verification_instructions": {"txt_record_name": "...", "txt_record_value": "..."},
  "directory_listed": true, "interests": ["music", "cinema"]
}
```

`username_changed_at`/`next_username_change_allowed_at` sono `null` se lo
username non è mai stato cambiato. `pending_email_change.stage` è
`awaiting_old_confirmation` (in attesa del codice sulla vecchia casella) o
`awaiting_new_confirmation` (in attesa del codice sulla nuova) — vedi il
flusso di cambio email sotto. `domain_pending_verification`/
`domain_verification_instructions` sono valorizzati solo se esiste un
dominio custom non ancora verificato (`pending`/`failed`), per poter
riprendere il flusso senza dover richiamare `POST .../domain`.
`directory_listed` (privato, mai esposto su `GET /{username}`): opt-out
dalla directory pubblica (`GET /users` sotto), attivo di default — il
profilo resta comunque sempre raggiungibile dal link diretto `@username`,
questo flag esclude solo dall'elenco/ricerca.

`display_name` è un alias pubblico globale (todo/BLOG.md #4): quando
valorizzato, è l'intestazione del profilo pubblico al posto di username /
nome e cognome.
`post_author_name_style` (todo/USERS.md #2) è la preferenza dell'utente su
cosa mostrare come nome autore sui propri post — `username` (default),
`full_name` (nome e cognome), `display_name` (alias globale) o
`verified_domain` (dominio personalizzato verificato, vedi sotto) —
applicata solo quando il blog non impone un nome pubblico (vedi sezione
Post). Come `display_name`, se il valore scelto non è disponibile (dominio
non verificato) ricade sullo username.
`first_name`/`last_name`/`country`/`native_language` sono liberi/opzionali.
`country` è solo controllato nel formato (ISO 3166-1 alpha-2, es. `IT`, non
verificato contro un elenco ufficiale dei paesi — vedi
`app/domain/profile.py`). `native_language`/`fallback_languages` sono codici
ISO 639-1 di 2 lettere, stesso formato del `locale` di post/pagine (vedi
sezione Multilingua). `fallback_languages` sono pensate anche come le lingue
verso cui l'utente potrà eventualmente tradurre i propri contenuti; massimo
5.

**`PATCH /api/v1/users/me`** (accetta anche `ui_locale`: `it`|`en`, `""` = torna al default di piattaforma — lingua dell'interfaccia, restituita da `GET /auth/me`) — richiede sessione, ritorna lo stesso schema di
`GET /api/v1/users/me`. Aggiorna `username`, `bio`,
`first_name`, `last_name`, `display_name`, `post_author_name_style`,
`country`, `native_language`, `fallback_languages` (tutti opzionali). Per
`first_name`/`last_name`/`display_name`/`country`/`native_language`: stringa
vuota `""` azzera il campo, assente lo lascia invariato, qualsiasi altro
valore lo sostituisce (`400` se il formato di `country`/`native_language` non
è valido). `post_author_name_style`: uno tra `username` | `full_name` |
`display_name` | `verified_domain` (`422` altrimenti), assente lo lascia
invariato — accettato anche senza un dominio verificato attivo (ricade sullo
username finché non lo è, stesso comportamento di `display_name` non
impostato). Per
`fallback_languages`: assente lascia invariata la lista, una lista (anche
vuota) la sostituisce (`400` se oltre 5 o un codice non valido).
`directory_listed`: booleano, assente lascia invariato — opt-out dalla
directory pubblica (`GET /users` sotto). `interests`: array di chiavi
canoniche (vedi `GET /api/v1/interests`), assente lascia invariato, una
lista (anche vuota) la sostituisce — massimo 5, `400` se oltre il limite o
se contiene una chiave non tra quelle correnti di piattaforma. `username`:
assente lo lascia invariato, altrimenti stesso formato/blacklist della
registrazione (`app/domain/usernames.py`, `400` se non valido, `409` se già
in uso) **più un cooldown di 5 giorni** (`USERNAME_CHANGE_COOLDOWN_DAYS`,
`app/domain/usernames.py`) tra due cambi consecutivi — `409` con un
messaggio che riporta la data del prossimo cambio consentito se violato; il
primo cambio in assoluto non è mai bloccato (`username_changed_at` parte
`null`). L'id resta comunque la vera chiave con cui il resto del sistema
referenzia l'utente, quindi un cambio consentito è visibile subito ovunque
(post, commenti, autocomplete `@menzioni`) — eccetto le `@menzioni` già
scritte nel testo di post/pagine esistenti, salvate come testo semplice e non
riscritte. Evento di audit `user.username_changed`
(`payload.old_username`/`new_username`).

### Cambio email verificato

Nessun `email` in `ProfileUpdateRequest`/`PATCH /users/me`: il cambio email
passa da un flusso dedicato a **due passi**, a prova che chi lo richiede
controlla sia la vecchia sia la nuova casella — stesso meccanismo OTP
dell'MFA email (`app/domain/mfa.py`), accodato su RabbitMQ e inviato dal
worker `worker-email-otp` esistente, tabella dedicata
(`email_change_requests`) invece di riusare `mfa_email_codes` per non
confondere i due tipi di codice per lo stesso utente. Solo sul prodotto
online (richiede `NOCT_SMTP_HOST` configurato per l'invio reale, come per
l'MFA email — senza, il codice resta solo loggato in sviluppo).

**`POST /api/v1/users/me/email/request`** — richiede sessione.
`{"new_email": "..."}` → `202`. Invia un codice a 6 cifre (TTL 10 minuti)
alla casella **attuale** dell'utente. `400` se `new_email` coincide con
quella attuale o è già in uso da un altro account. Una richiesta pending
precedente non completata viene sostituita.

**`POST /api/v1/users/me/email/verify-current`** — richiede sessione.
`{"code": "..."}` → `202`. Verifica il codice inviato alla vecchia casella,
poi ne invia uno nuovo alla **nuova** casella. `400` se codice errato/scaduto
o nessuna richiesta pending.

**`POST /api/v1/users/me/email/verify-new`** — richiede sessione.
`{"code": "..."}` → `200`, stesso schema di `GET /users/me` con l'email già
aggiornata. Verifica il codice inviato alla nuova casella e applica il
cambio (ricontrollando l'unicità per evitare race condition). `400` se
codice errato/scaduto o passo precedente non completato. Evento di audit
`user.email_changed` (`payload.old_email`/`new_email`).

**`DELETE /api/v1/users/me/email/request`** — richiede sessione. `204`,
idempotente (anche senza una richiesta pending). Annulla la richiesta
pending a qualunque passo si trovi — cancella la riga in
`email_change_requests`, non solo lo stato lato client, altrimenti l'OTP già
inviato resterebbe comunque valido fino a scadenza.

### Dominio custom verificato via DNS (stile Bluesky)

Un dominio per utente (`custom_domains`, `user_id` unico), verificato
dimostrando il possesso pubblicando un record TXT sul proprio DNS — nessuna
dipendenza da HTTP/SSRF, solo lookup DNS (`dnspython`,
`app/domain/custom_domains.py`). Una verifica riuscita assegna il sigillo
`bronze` (mai degrada un tier superiore già assegnato da altra logica
futura) e copia il dominio su `users.verified_domain` (colonna denormalizzata,
azzerata alla rimozione del dominio — vedi `app/models/user.py`). Lo username
di piattaforma resta **sempre** l'identificativo di riserva, citabile e
risolvibile: il dominio è un'aggiunta, non cablata nel routing/permalink (il
sottodominio-per-blog resta `⚪` in ROADMAP.md §3), ma **risolvibile** al
posto dello username in `GET /api/v1/users/{username}` e negli endpoint
pubblici correlati (vedi sopra), e selezionabile come `post_author_name_style`
per firmare post/commenti (`verified_domain`, con fallback allo username se
il dominio non è verificato).

**`POST /api/v1/users/me/domain`** — richiede sessione. `{"domain":
"iltuodominio.it"}` → `200`, crea/sostituisce il dominio in stato `pending`:

```json
{
  "domain": "iltuodominio.it", "status": "pending",
  "txt_record_name": "_notturni-challenge.iltuodominio.it",
  "txt_record_value": "notturni-verify=<token>"
}
```

`400` se il formato non è un hostname valido o è un (sotto)dominio della
piattaforma stessa (`NOCT_INSTANCE_FQDN`); `409` se già rivendicato e
verificato da un altro account, o se due utenti rivendicano in parallelo lo
stesso dominio ancora libero (vince chi fa commit per primo, l'altro riceve
`409` invece di un errore generico).

**`POST /api/v1/users/me/domain/verify`** — richiede sessione. Interroga il
DNS per il record TXT atteso (timeout 5s, fail sulla singola verifica non
sulla feature); se combacia, `200` con `status: "verified"` e assegna il
sigillo bronzo; altrimenti `400` con `status: "failed"`, riprovabile.
Rate-limitato (5 tentativi/10 minuti per utente, `app/domain/rate_limit.py`,
stesso fail-open del resto). Evento di audit `user.domain_verified`.

**`DELETE /api/v1/users/me/domain`** — richiede sessione, `204`, idempotente.
Se il dominio era verificato e il tier era `bronze` (assegnato solo da
questa verifica), riporta `verification_tier` a `none`.

**`GET /api/v1/users/{username}/blogs`**, **`.../posts`**, **`.../comments`**
— pubblici, `404` se l'utente non esiste. Tab del profilo pubblico (mockup
3e): blog pubblici non sospesi di proprietà dell'utente (stesso schema di
`GET /blogs/{slug}`, `owner_id` a `null`), post pubblicati su blog pubblici
(stesso schema del feed; `limit` default 20 max 50, `offset`) e commenti
approvati su post pubblici (`{id, content, created_at, post_title,
permalink}`, stessa paginazione). **Regola di privacy (CLAUDE.md #8)**: sono
elencati solo i contenuti firmati pubblicamente con lo username — un blog
con `default_author_display_name` diverso dallo username, i post firmati con
quell'alias e i commenti lasciati con un alias restano fuori, altrimenti
questi endpoint collegherebbero l'alias all'identità reale.

**`GET /api/v1/users/me/follow-stats`** — richiede sessione. Somma i
follower dell'utente (`UserFollow`) con quelli di tutti i suoi blog
(`BlogFollow`), inclusi i blog che si presentano con un alias diverso dal
suo username:

```json
{
  "user_followers": 12,
  "blogs": [{"blog_slug": "il-mio-blog", "blog_title": "...", "alias": "Anonimo Curioso", "followers": 40}],
  "total_followers": 52
}
```

Riservato al proprietario: è l'unico endpoint in cui identità reale e alias
di blog compaiono insieme. Pubblicamente, `GET /api/v1/users/{username}/followers`
e `GET /api/v1/blogs/{slug}/followers` continuano a mostrare solo il
conteggio/elenco della singola entità, senza mai collegarli tra loro.

**`POST /api/v1/users/me/avatar`** — richiede sessione, `multipart/form-data`
con campo `file`. Formati ammessi: PNG, JPEG, WEBP; max 2 MiB (`400`
altrimenti). Carica sul backend di storage configurato (S3/MinIO con
endpoint custom sempre iniettato, o filesystem locale — bucket/namespace
`avatars` reso pubblico in lettura), sostituisce ed elimina l'eventuale
avatar precedente. Ritorna `{"avatar_url": "..."}`.

**`DELETE /api/v1/users/me/avatar`** — richiede sessione. Rimuove l'avatar
corrente (idempotente).

**`POST /api/v1/users/me/social-links`** — richiede sessione.
`{"label": "mastodon", "url": "https://..."}` → `201`. Massimo 5 link per
profilo (`400` oltre il limite). `label` non è più pensato come etichetta
libera ma come chiave di piattaforma (es. `mastodon`, `bluesky`, `github`,
`website`) — l'elenco delle piattaforme note, con relativa icona
monocromatica, è solo lato frontend (`frontend/src/lib/social-platforms.tsx`,
facilmente estendibile): il backend continua a non validare il valore
contro un elenco chiuso, resta una stringa libera.

**`DELETE /api/v1/users/me/social-links/{link_id}`** — richiede sessione,
solo un link proprio (`404` altrimenti, per non rivelarne l'esistenza).

**`POST /api/v1/users/{username}/follow`** / **`DELETE .../follow`** —
richiede sessione. Segui/smetti di seguire un utente; idempotenti. `400` se
si prova a seguire se stessi.

**`GET /api/v1/users/{username}/followers`** / **`.../following`** —
pubblici. Liste `{username}`.

### GDPR (ROADMAP.md §1)

**`GET /api/v1/users/me/export-data`** — richiede sessione. Diritto di
accesso/portabilità (Art. 20): istantanea JSON di tutti i dati collegati
all'account — profilo, blog di proprietà, post e commenti scritti (ovunque,
non solo sui propri blog — restano comunque parole scritte dall'utente),
frammenti salvati, follow (in entrambe le direzioni, solo gli id), token API
(nome/prefisso/date, mai il segreto o l'hash), eventi di audit di cui è
l'attore (fino a 1000, i più recenti) e l'eventuale claim di dominio custom
(anche se ancora `pending`/`failed`, non solo quello già verificato — mai il
`verification_token`, segreto operativo e non dato personale). Struttura
libera, non un `response_model` tipizzato: vedi
`app/domain/gdpr.py::export_user_data` per i campi esatti.

**`DELETE /api/v1/users/me`** — richiede sessione.
`{"confirm_username": "il-proprio-username"}` → `204`, `400` se non
corrisponde esattamente allo username corrente (nessuna conferma via
password: il flusso è pensato per essere lanciato da un form di conferma già
autenticato in dashboard, non da un client automatizzato).

**Importante**: non cancella la riga `users` — la **anonimizza**. Un utente
può essere proprietario di blog pubblici con collaboratori o aver commentato
su blog altrui: cancellare la riga richiederebbe bloccare l'operazione finché
non esistono più blog/post/commenti collegati (nessuna funzionalità di
trasferimento/cancellazione blog esiste oggi) oppure cancellare a cascata
anche quel contenuto, danneggiando terzi che non hanno chiesto nulla —
approccio comunque ammesso dal GDPR quando l'erasure in senso stretto
confligge con diritti di terzi (Art. 17.3).

Cancellati per intero (dati puramente personali): sessioni (logout ovunque),
token API, codici MFA email pendenti, identità SSO, link social, frammenti
salvati, follow (in entrambe le direzioni) e le membership come collaboratore
su blog altrui. `username`/`email` sostituiti con valori anonimi univoci,
`is_active=false` (blocca subito login e riutilizzo di token/sessioni
esistenti, `app/api/deps.py::get_current_user`), `display_name="Utente
eliminato"` con `post_author_name_style="display_name"`: blog di proprietà,
post e commenti restano, ma da subito con questo autore ovunque (stessa
risoluzione dinamica degli alias già in uso per la privacy dei blog,
`app/domain/display_names.py`). Evento `user.account_deleted` in audit log,
registrato **prima** dell'anonimizzazione (l'`actor_label` conserva quindi
username/email originali). Vedi `app/domain/gdpr.py` per i dettagli.

## API token (motore core / accesso diretto utente)

Il token è un valore opaco (non un JWT decodificabile), generato con prefisso
`noct_`; solo il suo hash sha256 è persistito in database (tabella
`api_tokens`, modello `app/models/api_token.py`). Ogni token ha un
`owner_type`:

- **`core`** — token del motore/servizio, non legato a un utente. Pensato per
  chiamate machine-to-machine: worker interni, script di manutenzione, task
  pianificati.
- **`user`** — token legato a uno `User` specifico, per permettere
  all'utente di interfacciarsi con l'API senza passare dall'editor del
  frontend o dall'admin del proprio blog. Gestibile dalla dashboard
  (`frontend/src/app/dashboard/token`).

Gli endpoint `/api/v1/tokens` accettano **due schemi di autenticazione**
diversi sullo stesso header `Authorization: Bearer`:

- un **ApiToken opaco** (`noct_...`) — inerita `owner_type` (e utente, se
  applicabile) del token stesso; è il meccanismo di bootstrap/rotazione per i
  token `core`.
- un **access token JWT di sessione** (login utente) — necessario per
  emettere il *primo* token `user` dalla dashboard, dove per definizione non
  esiste ancora nessun ApiToken con cui autenticare la richiesta. Con questa
  autenticazione `owner_type` è sempre `user`, legato all'utente della
  sessione.

`app/api/deps.py::get_token_actor` distingue i due casi in base al prefisso
del valore ricevuto (`noct_` vs JWT) e normalizza l'attore per gli endpoint
sotto e per l'audit log.

### Come ottenere il primo token core

```bash
cd backend && source .venv/bin/activate
python -m scripts.create_api_token --name "core-engine"
```

Il valore in chiaro viene stampato una sola volta. Un utente non ha bisogno
dello script: crea il proprio primo token `user` direttamente dalla dashboard
(sessione JWT, vedi sopra).

### `POST /api/v1/tokens`

Crea un nuovo token con lo stesso `owner_type` (e utente, se applicabile)
dell'attore che ha autenticato la richiesta (ApiToken o sessione JWT).

```json
{"name": "descrizione-libera"}
```

→ `201`, il campo `token` è il valore in chiaro, mostrato solo qui.

### `GET /api/v1/tokens`

Elenca i token dello stesso `owner_type` del chiamante (per `owner_type=user`,
solo i token dell'utente stesso). Non espone mai il valore in chiaro né
l'hash, solo `token_prefix`.

### `DELETE /api/v1/tokens/{token_id}`

Revoca un token (imposta `revoked_at`, non lo elimina). `403` se non è un
token proprio, `404` se l'id non esiste.

## Amministrazione di piattaforma

Tutti gli endpoint richiedono sessione con `platform_role` in
`amministratore`/`super_admin` (`403` altrimenti), **eccetto**
`GET /api/v1/admin/comments` più sotto, che accetta anche `moderatore`.
Consumati dalle sezioni
`frontend/src/app/admin/{utenti,blog,moderazione,moderazione-commenti,registro}`
— sotto il prefisso `/admin/*`, separato da `/dashboard/*` (sezioni
personali), non un'app a parte — vedi ROADMAP.md.

**`GET /api/v1/admin/overview`** — accetta anche `moderatore`. Panoramica
di piattaforma (todo/UX_REDESIGN.md B1, mockup 5d): `users_total`,
`users_new_7d`, `blogs_total`, `blogs_suspended`, `posts_published`, le code
`queue_pending_comments`/`queue_posts_in_review`/`queue_hidden_posts`,
`queue_open_reports`, `audit_today` (voci del registro dalla mezzanotte UTC), `deployment_mode` e
`services`: lista `{name, status, detail?}` con `status` in `ok`/`down`/
`unconfigured` per `postgres` (`SELECT 1`), `redis` (`PING`), `rabbitmq` e
`storage` (connessione TCP con timeout 2 s; `storage` è sempre `ok` con
`localstorage`) e `moderation` (`GET /health` del servizio, `unconfigured`
se `NOCT_MODERATION_SERVICE_URL` è assente). Solo aggregati, nessun dato
personale.

**Nota obbligatoria (B5, mockup 5e)**: `PATCH /admin/users/{id}` (cambio
ruolo o attivazione), `PATCH /admin/blogs/{id}` (sospensione) e
`PATCH /admin/posts/{id}` (nascondere/mostrare) richiedono `note` (almeno 3
caratteri, `400` altrimenti) quando cambiano davvero lo stato; la nota
finisce in `payload.note` della voce di audit. Nessuna nota richiesta se
la richiesta non cambia nulla.

### Impostazioni di piattaforma (todo/UX_REDESIGN.md B6, mockup 5f)

**`GET /api/v1/admin/config`** / **`PATCH /api/v1/admin/config`** — solo
`super_admin` (`403` altrimenti). Riga unica `platform_config`, creata al
primo accesso con i default (`NOCT_DEFAULT_LOCALE` per la lingua, 5 blog,
registrazione aperta). Campi: `default_locale` (`it`|`en`),
`registration_mode` (`open`|`invite`|`closed` — `invite`/`closed` fanno
rispondere `403` a `POST /auth/register`, con messaggi diversi),
`sso_providers` (sottoinsieme dei provider configurati via env; vuoto =
tutti quelli configurati; un provider escluso risponde `403` su
`/auth/sso/{provider}/login`), `mfa_required_for_admins` (se attivo, un
Amministratore/Super Admin senza MFA riceve `403` su tutta l'area admin
finché non la attiva), `reserved_blog_names` (in aggiunta alla blacklist di
codice `reserved_builtin`, sola lettura), `moderation_threshold` (0–1,
passata al servizio di moderazione a ogni upload), `max_blogs_per_user`
(1–100), `anonymous_comments_allowed` (se `false`, `comments_mode=everyone`
non è più impostabile), `audit_retention_days` (7–3650, default seminato da
`NOCT_AUDIT_RETENTION_DAYS`: giorni di conservazione degli eventi in
`audit_log` prima della cancellazione periodica —
`app/workers/audit_maintenance.py::prune`, letto dalla riga `platform_config`
a ogni giro, non più dall'env dopo la creazione della riga). La risposta
include anche `infrastructure`: riepilogo di sola lettura dell'ambiente
`NOCT_*` (mai segreti, non include più `audit_retention_days` — ora un campo
modificabile a sé, non un valore d'ambiente), `footer_column1_markdown`/
`footer_column2_markdown`/`footer_column3_markdown`/`footer_bottom_bar_markdown`
(Markdown libero, max 5000 caratteri ciascuno, `""` azzera — footer mostrato
su ogni pagina pubblica di piattaforma e di ogni blog, vedi `GET /api/v1/footer`
sotto; le colonne 1/2 sono solo il default, sovrascrivibile per singolo blog
in `PUT /blogs/{slug}/config` — mai la 3 né `bottom_bar`, sempre e solo di
piattaforma), `interests` (blocco "interessi utente": elenco completo —
questo campo **sostituisce**, non aggiunge, a differenza di
`reserved_blog_names` — di `{"key": "...", "translations": {"it": "...",
"en": "..."}}`; chiave canonica in formato slug `[a-z0-9_-]{1,40}`, univoca,
almeno una traduzione non vuota per voce, massimo 200 voci; seminato alla
creazione della riga da `NOCT_DEFAULT_INTERESTS` (JSON, stesso schema) se
valorizzata, altrimenti da un elenco builtin curato
(`app/domain/interests.py::DEFAULT_INTERESTS`) — vedi `GET /api/v1/interests`
sotto per l'elenco pubblico e `PATCH /users/me` per la scelta dell'utente;
rimuovere una chiave qui ripulisce anche `User.interests` di ogni utente che
l'aveva selezionata, non solo l'elenco di piattaforma), `max_blog_storage_mb`
(spazio massimo per blog — media + backup Markdown, stesso conteggio di
`GET /blogs/{slug}/overview::storage_bytes` — in MB; `null`/`0` = nessun
limite, default; superarlo risponde `413` su
`POST /blogs/{slug}/media` e `POST /blogs/{slug}/cover-image`),
`verification_gold_identifiers`/`verification_silver_identifiers`
(array di email/username, confronto case-insensitive: assegnano
rispettivamente il sigillo di verifica `gold` — entità verificate a mano
dalla piattaforma, testate/agenzie/organizzazioni/personalità note — e
`silver` — sostenitori economici del progetto; `verification_gold_identifiers`
accetta anche un dominio email nudo, es. `"testata.it"`, non solo email/
username interi) e `verification_blue_domains` (array di domini email,
formato hostname validato: assegnano il sigillo `blue`, in aggiunta al
dominio della piattaforma stessa — `NOCT_INSTANCE_FQDN` — già incluso
automaticamente). `User.verification_tier` (vedi `GET /users/{username}`)
è ricalcolato da `app/domain/verification.py` a ogni evento che può
cambiarlo (registrazione, cambio email/username, verifica/rimozione del
dominio custom, modifica di uno di questi tre elenchi — che ricalcola
**tutti** gli utenti attivi in un colpo solo); priorità in caso di più
criteri soddisfatti: `gold` > `silver` > `blue` > `bronze` (dominio custom
verificato via DNS) > `none`, mai persa "per errore" (rimosso da un elenco
più alto ricade sul tier immediatamente inferiore ancora valido, non su
`none` a prescindere).
Ogni modifica va nel registro (`platform.config_updated`, con
`changes: {campo: {from, to}}`) e, se cambia un campo `footer_*`, invalida la
cache del frontend sul tag condiviso `platform-footer` (tutte le pagine
pubbliche, non solo quelle di un blog).
`GET /api/v1/config` (pubblico) espone `default_locale`, `registration_mode`
e `sso_providers` effettivi.

**`GET /api/v1/footer`** — pubblico, nessuna auth. Footer di piattaforma,
Markdown grezzo non ancora renderizzato (il frontend lo fa al momento della
lettura, stesso principio dei post):

```json
{"column1": "...", "column2": "...", "column3": "...", "bottom_bar": "..."}
```

Ogni chiave è `null` se non configurata (nessun default se non per
`bottom_bar`, seminato alla creazione della riga `platform_config` con un
link al repository — comunque modificabile/azzerabile in qualsiasi momento).

### Richieste GDPR (B6, mockup 5f)

**`GET /api/v1/admin/gdpr?status=`** — admin. Registro delle richieste
`{id, username, type: export|deletion, status: open|approved|completed|
rejected, deadline_at, note, created_by_username, approved_by_username,
approved_at, completed_at, created_at}`, aperte prima e per scadenza (30
giorni dalla ricezione, Art. 12). Le azioni self-service (`GET
/users/me/export-data`, `DELETE /users/me`) lasciano una riga già
`completed` con nota `self-service`.

**`POST /api/v1/admin/gdpr`** — `{username, type, note}` (nota obbligatoria:
chi/come ha chiesto). Inserisce una richiesta arrivata fuori banda.

**`POST /api/v1/admin/gdpr/{id}/approve`** — seconda approvazione: per una
`deletion` chi approva deve essere un admin **diverso** da chi l'ha inserita
(`403` altrimenti). Un `export` non ne ha bisogno.

**`POST /api/v1/admin/gdpr/{id}/execute`** — `export` (aperto o approvato):
risponde con il JSON dei dati dell'utente (stesso formato di
`/users/me/export-data`) e chiude la richiesta; `deletion`: solo se
`approved`, anonimizza l'account come `DELETE /users/me` (`400` per un
Super Admin). **`.../reject`** — `{note}` obbligatoria. Tutto nel registro
(`gdpr.request_created|approved|rejected`, `gdpr.export_executed`,
`gdpr.deletion_executed`).

**`GET /api/v1/admin/users`** — lista tutti gli utenti della piattaforma
(id, username, email, `platform_role`, `is_active`, `mfa_enabled`,
`blogs_count` — blog di proprietà — e `last_seen_at`, ultimo uso di una
sessione di refresh, `null` se mai usata). Query param opzionale `q`: filtra
per username o email (`ilike`, sottostringa).

**`PATCH /api/v1/admin/users/{user_id}`** — `{platform_role?, is_active?}`.

- Assegnare o rimuovere i ruoli `amministratore`/`super_admin` (sia come
  valore di partenza che di arrivo) richiede essere `super_admin`: un
  `amministratore` può gestire solo `utente`/`moderatore` (`403` altrimenti).
- Non è possibile disattivare il proprio stesso account (`400`) — evita
  l'auto-blocco dell'unico Super Admin rimasto.
- Un utente disattivato (`is_active=false`) non può più fare login.

**`POST /api/v1/admin/users/{user_id}/reset-password`** — richiede
`Amministratore`/`Super Admin`, sempre `202`, nessun corpo. Reset forzoso
della password: innesca verso l'utente lo stesso ciclo email di
`POST /auth/password/forgot` (stessa funzione di dominio
`app/domain/password_reset.py::request_password_reset` — codice a 6 cifre,
TTL 10 minuti, invio via coda RabbitMQ), ma avviato dall'admin invece che
dall'utente stesso: l'admin non imposta né vede alcuna password, solo
innesca l'invio. `400` se l'utente target non è attivo. Nessun rate limit
per IP/email (quello di `/auth/password/forgot` è pensato per un anonimo
che enumera indirizzi): solo un limite più permissivo per attore admin
(20/5 min), contro un account admin compromesso che spamma reset su molti
utenti. Evento di audit `user.password_reset_triggered` (nessuna nota
richiesta, a differenza di cambio ruolo/attivazione — non è un cambio di
stato persistente sull'account).

Non esiste un endpoint per creare il primo Super Admin (nessuna sessione da
cui autenticare la richiesta), ma non serve più promuoverlo a mano sul
database: se `NOCT_SUPER_ADMIN_USERNAME`/`NOCT_SUPER_ADMIN_EMAIL`/
`NOCT_SUPER_ADMIN_PASSWORD` sono tutte valorizzate (`.env`/`k8s/secret.yaml`),
l'account viene creato automaticamente all'avvio del backend se non esiste
già (`app/domain/auth.py::bootstrap_super_admin`, eseguita dalla `lifespan`
di `app/main.py`) — idempotente: un riavvio successivo non lo ricrea né lo
tocca. Restano comunque disponibili, come alternativa, l'auto-promozione del
primo utente in modalità `solo` e l'`UPDATE` manuale a DB in modalità
`platform`.

**`GET /api/v1/admin/blogs`** — lista tutti i blog della piattaforma (id,
slug, titolo, `owner_username`, `visibility`, `is_suspended`, `is_paused`,
`deleted_at`, `posts_count`, `reports_open`). Query param opzionali: `q`
(slug, titolo o proprietario), `visibility`, `state` (`active` |
`suspended` | `paused` | `deleted` | `reported` — quest'ultimo: solo blog
con segnalazioni aperte, ordinati per numero).

**`PATCH /api/v1/admin/blogs/{blog_id}`** — `{is_suspended: bool, note}`. Un blog
sospeso è irraggiungibile pubblicamente (il dettaglio resta leggibile con
`is_suspended=true` per la pagina di avviso, vedi sezione Blog) e non
scrivibile, proprietario incluso, finché non viene riattivato.
Registrato nel registro di audit con la nota.

**`GET /api/v1/admin/blogs/{blog_id}/reports`** — pannello segnalazioni
(mockup 5e): `{blog, owner_mfa_enabled, owner_email_domain, reports: [{id,
target_type, target_id, post_slug, post_title, reason, note,
reporter_username, created_at}]}` — solo le segnalazioni **aperte** sul
blog e sui suoi post.

**`POST /api/v1/admin/blogs/{blog_id}/action`** — `{action, note}` (nota
obbligatoria). `action`: `suspend`, `restore`, `hide_reported_posts` (solo i
post con segnalazioni aperte), `deactivate_owner` (disattiva l'account del
proprietario e sospende il blog; `403` per un amministratore se non si è
super admin), `dismiss` (archivia). Chiude tutte le segnalazioni aperte del
blog (`actioned`, o `dismissed` per `dismiss`) e registra l'azione con la
nota. Risponde con il blog aggiornato.

**`GET /api/v1/admin/posts`** — lista tutti i post della piattaforma (con `reports_open`), dal più
recente (id, title, slug, `blog_slug`, `blog_title`, `author_username`,
`status`, `is_hidden`, `published_at`, `created_at`) — bozze/in
revisione/pianificati inclusi, non solo i pubblicati. Query param opzionale
`q`: filtra per titolo o slug del post, slug del blog, o username
dell'autore (`ilike`, sottostringa).

**`PATCH /api/v1/admin/posts/{post_id}`** — `{is_hidden: bool}`. Vedi
`Post.is_hidden` nella sezione Post: nasconde/mostra un post indipendentemente
da `status`, anche per l'autore. Nessuna notifica all'autore e nessun campo
per la motivazione; il cambio di stato viene però registrato nel registro di
audit (`post.hidden`/`post.unhidden`, vedi sotto).

**`GET /api/v1/admin/comments`** (accetta anche `reported=true`: solo i commenti segnalati dai blog, con `report_note`/`reported_at`, dal più recente segnalato) — richiede `platform_role` in
`amministratore`/`super_admin`/**`moderatore`** (unico endpoint di questa
sezione aperto anche al ruolo Moderatore, ROADMAP.md §1). Commenti di *tutti*
i blog della piattaforma nello stato indicato dal parametro opzionale
`status` (`pending` di default, oppure `approved`/`rejected`), dal più
recente — stessa forma di `GET /api/v1/blogs/{slug}/comments` ma senza
restrizione a un singolo blog, con in più `blog_id`/`blog_slug`/`blog_title`.
Query param opzionale `q`: filtra per contenuto del commento, nome
dell'autore, titolo del post o slug del blog (`ilike`, sottostringa).
Approvazione/rifiuto restano i `POST /api/v1/comments/{comment_id}/approve`/
`reject` già descritti sopra (stessa autorizzazione).

### Registro di audit

Ogni azione sensibile lascia una riga in `audit_log` (append-only), scritta
nella stessa transazione dell'azione: autenticazione (`auth.login` con
`payload.method` `password`/`mfa_*`/`sso_*`, `auth.login_failed`),
amministrazione (`user.role_change` con `payload {from,to}`,
`user.activated`/`user.deactivated`, `blog.suspended`/`blog.unsuspended`,
`post.hidden`/`post.unhidden`, `comment.approved`/`comment.rejected`), API
token (`api_token.created`/`api_token.revoked`), account (`user.
account_deleted`, GDPR), inviti e membership del blog
(`blog.invitation_created`/`_accepted`/`_declined`/`_revoked`,
`blog.member_role_changed` con `payload {from,to}`, `blog.member_removed`),
pagine statiche (`page.created`/`page.updated`/`page.deleted`, sia di
piattaforma sia di blog — `blog_id` presente solo per queste ultime). Ogni
riga porta:

- `actor_type`/`actor_id`/`actor_label` — chi: tipo di attore, il suo id (se
  applicabile) e uno snapshot leggibile `username <email>` al momento del
  fatto (resta valido anche se l'account viene poi rinominato o cancellato).
- `ip`/`user_agent` — indirizzo IP sorgente della richiesta (**ultimo** hop di
  `X-Forwarded-For` dietro Traefik — l'unico scritto dal proxy fidato, non dal
  client, che potrebbe altrimenti spoofare un primo hop a piacere — altrimenti
  l'IP di connessione diretta, `app/core/http.py::client_ip`) e user agent.
- `target_type`/`target_id`/`blog_id` — su cosa: tipo e id dell'oggetto
  coinvolto, più il blog di contesto quando applicabile (denormalizzato per
  filtrare senza join).
- `payload` (JSON) — dettagli specifici dell'azione; per le azioni con un
  blog di contesto include anche `blog_alias`, l'alias pubblico sotto cui
  compare quel blog (`Blog.default_author_display_name`, `null` se il blog
  non ne ha uno) — in aggiunta all'identità reale in `actor_label`, mai al
  posto.
- `occurred_at`.

**Canale** (web/api/system): non una colonna a sé, ma un campo calcolato da
`actor_type` sia nella risposta di `GET /api/v1/admin/audit-log` sia nel suo
filtro (vedi sotto) — `user`/`anonymous` sono sempre passati da una sessione
autenticata via browser/app o non ancora autenticata (login), mai da un
ApiToken; `core_token`/`user_token` sono sempre un accesso diretto via token
opaco; `system` un processo interno senza richiesta HTTP (bootstrap, job
schedulati). Mappa in `app/api/v1/admin.py::_CHANNEL_BY_ACTOR_TYPE`.

Gli eventi oltre `NOCT_AUDIT_RETENTION_DAYS` (default 105) vengono scaricati
su storage in NDJSON gzippato per settimana ISO e poi rimossi dal database
(`app/workers/audit_maintenance.py`); la cancellazione non tocca mai eventi
non ancora archiviati.

**`GET /api/v1/admin/audit-log`** — righe dal più recente. Query param
opzionali: `action`, `actor_id`, `target_id`, `blog_id` (uguaglianza esatta),
`channel` (`web`/`api`/`system`, vedi sopra), `since`/`until` (ISO 8601,
intervallo `[since, until)` su `occurred_at`), `limit` (1–500, default 100),
`offset` (default 0). Solo gli eventi ancora nel database: quelli già
archiviati stanno su storage, reimportabili con
`python -m app.workers.audit_maintenance --restore AAAAwSS`.

## Feed (homepage multi-blog)

**`GET /api/v1/feed/posts`** — pubblico, nessuna autenticazione. Post
pubblicati (e con `published_at` raggiunto) dei soli blog **`public`** (i
blog `members`/`private` sono esclusi — vedi `visibility` nella sezione
Blog), dal più recente — pensato per la homepage della piattaforma
(CLAUDE.md #2: "raccolta degli articoli nella lingua dell'utente, stile
dev.to").

Query param opzionali: `locale` (filtra una lingua, altrimenti tutte
insieme), `tag` (filtra per tag normalizzato, es. `poesia` non `#Poesia` —
vedi sezione "Tag" sopra), `category` (filtra per slug di categoria — vedi
sezione "Categorie" sopra; essendo la categoria per-blog, blog diversi con
una categoria omonima compaiono insieme, come già avviene per i tag),
`limit` (default 20, massimo 50), `offset` (paginazione, default 0),
`following=true` (richiede sessione, `401` altrimenti — mockup 1c
"Seguiti"): solo i post dei blog seguiti o scritti dagli utenti seguiti,
con gli stessi vincoli di visibilità del resto del feed. Router
separato da `/blogs/{slug}/posts` apposta: qui i post attraversano blog
diversi, non sono scoped a uno slug/id specifico.

**`GET /api/v1/feed/trending`** — pubblico, nessuna autenticazione. Tag più
usati tra i post pubblicati (dei soli blog `public`) negli ultimi `days`
giorni (default 7, massimo 90), dal più frequente:
`[{"tag": "poesia", "post_count": 12}, ...]`. Query
param opzionali: `days`, `limit` (default 10, massimo 30). Non esistono
ancora contatori di like/condivisioni in piattaforma (vedi ROADMAP.md): è
l'unica base disponibile oggi per una sezione "di tendenza".

**`GET /api/v1/feed/locales`** — pubblico, nessuna autenticazione. Conteggio
post per lingua, stessi filtri di visibilità di `GET /feed/posts`, dal più
usato, **massimo 5 risultati**, solo lingue con almeno un post:
`[{"locale": "it", "count": 42}, {"locale": "en", "count": 7}]`. Pensato
per i pill del filtro lingua sulla homepage — sostituisce un elenco statico
di lingue hardcoded che poteva mostrare lingue senza alcun post.

## Ricerca

Due endpoint distinti, non uno solo con uno scope opzionale: la ricerca sul
portale attraversa tutti i blog pubblici, quella su un singolo blog (anche
raggiunto dal proprio sottodominio) resta ristretta ai suoi soli post.
Entrambi cercano in titolo e contenuto con `ILIKE` (nessuno stemming/full
text search Postgres oggi, stesso approccio pragmatico già usato da
`GET /blogs?q=`), quindi case-insensitive e senza bisogno di parole intere.

**`GET /api/v1/search/posts`** — pubblico, nessuna autenticazione. `q`
(obbligatorio, stringa vuota o solo spazi → `[]` senza errore) cerca in
titolo/contenuto tra i post pubblicati dei soli blog `public`, stessi
vincoli di visibilità di `GET /feed/posts`, dal più recente. Query param
opzionali: `limit` (default 20, massimo 50), `offset` (paginazione, default
0). Per cercare i blog stessi per nome vedi `GET /blogs?q=` (sezione Blog).

**`GET /blogs/{slug}/search`** — pubblico. Come sopra ma ristretto ai post
del blog `{slug}` (`404` se il blog non è visibile al richiedente, stesso
comportamento di `GET /blogs/{slug}/posts`); sempre solo post effettivamente
pubblicati, anche per chi ha accesso in scrittura al blog (a differenza di
`GET /blogs/{slug}/posts`, che a loro mostra anche bozze/revisione: la
ricerca è una casella pubblica, non uno strumento di editing). Stessi
`limit`/`offset` di sopra.

## Newsletter (ROADMAP.md §3)

Una lista per blog (`blog_slug`) più una lista di piattaforma (digest,
nessun `blog_slug`), doppio opt-in via email, disiscrizione/cancellazione
self-service senza login.

**`POST /api/v1/newsletter/subscribe`** — pubblico. Body `{email, blog_slug?,
locale?}`. Rate-limited (5/ora per IP, 3/ora per email). Risponde sempre
`202 {"status": "ok"}`, incluso quando l'email è già iscritta e confermata:
nessuna enumerazione di indirizzi via risposta diversa. Se nuovo o non
ancora confermato, genera un token di conferma opaco (hash sha256 in
tabella, come i token API) e accoda l'invio dell'email su RabbitMQ.

**`GET /api/v1/newsletter/confirm?token=...`** — pubblico. `200
{"status": "confirmed"|"already_confirmed"|"invalid"}`, idempotente (un
secondo click sullo stesso link valido risponde `already_confirmed`, non
errore). Token scaduto dopo 48 ore → `invalid`.

**`POST /api/v1/newsletter/unsubscribe`** — pubblico. Body `{token,
reason?}`. Il `token` è un link firmato HMAC (non un hash in tabella: serve
poterlo ricostruire ad ogni invio, non solo verificarlo una volta), incluso
in fondo a ogni email inviata. `400` se il token non è valido.

**`POST /api/v1/newsletter/unsubscribe/delete`** — pubblico. Body
`{token}`. Cancellazione permanente della riga (diritto alla cancellazione
GDPR Art. 17), self-service senza bisogno di login: chi riceve l'email è
già identificato dal token firmato.

**Gestione per blog** (proprietario o membership `autore`/`co_autore`,
stessa logica di `can_write_posts`):

- `GET /blogs/{slug}/newsletter/stats` → `{pending, confirmed,
  unsubscribed}`.
- `GET /blogs/{slug}/newsletter/campaigns` → elenco campagne (automatiche e
  manuali), più recenti prima.
- `POST /blogs/{slug}/newsletter/campaigns` — body `{subject, body_markdown,
  scheduled_at?}`. Senza `scheduled_at` (o nel passato): invio immediato
  (`status=sending`, accodato su RabbitMQ). Con `scheduled_at` futuro:
  resta `status=scheduled` — **nessuno scheduler la invia ancora
  automaticamente**, è solo lo stato persistito. `scheduled_at` deve
  includere il fuso orario (es. suffisso `Z` o `+00:00`): un valore naive
  è rifiutato con 422, non confrontabile con l'istante corrente. L'invio
  (`app/workers/newsletter_consumer.py`) è idempotente su ridelivery del
  messaggio (`NewsletterCampaign.sent_to_subscriber_ids`): un iscritto già
  notificato non riceve una seconda email se il worker viene interrotto a
  metà invio e il messaggio torna in coda.
- `PATCH /blogs/{slug}/newsletter/campaigns/{id}` / `DELETE .../campaigns/{id}`
  — modifica (`{subject?, body_markdown?, scheduled_at?}`) o annulla una
  campagna, **solo mentre `status=scheduled`** (409 altrimenti: una
  campagna già `sending` può essere già stata presa in carico dal worker,
  nessuna finestra sicura per intercettarla; le automatiche
  `post_notification` nascono già `sending`, quindi non sono mai in questo
  stato). `DELETE` non cancella la riga, la porta a `status=canceled` (il
  worker la salta se il messaggio è già in coda). `scheduled_at` è
  tri-state come le impostazioni sopra: omesso lascia invariato, `null` o
  un istante nel passato converte subito la campagna in invio immediato
  (stessa logica della creazione).
- `GET /blogs/{slug}/newsletter/settings` / `PATCH .../newsletter/settings`
  — `{newsletter_auto_notify_enabled, newsletter_sender_name,
  newsletter_banner_url, newsletter_banner_alt_text}`. Il primo campo
  disattiva/riattiva la notifica automatica ad ogni post pubblicato per
  questo blog (attivo di default, `Blog.newsletter_auto_notify_enabled`),
  un solo invio automatico per post anche in caso di ripubblicazione
  (vincolo unique su `post_id`). Gli altri tre personalizzano l'email delle
  campagne: nome visualizzato nell'header `From` (indirizzo resta sempre
  `NOCT_SMTP_FROM_EMAIL`, mai un dominio arbitrario) e un banner mostrato in
  cima al corpo HTML (`newsletter_banner_url` deve iniziare con `http://`
  o `https://`, come i link nelle note/bibliografia). **PATCH è tri-state**:
  un campo omesso nel body lascia il valore attuale invariato, `null` lo
  azzera esplicitamente — vale anche per `newsletter_auto_notify_enabled`,
  reso opzionale per questo. Default (tutti i campi assenti/vuoti): nome
  mittente = titolo del blog, nessun banner.
- Le email delle campagne (`app/workers/newsletter_consumer.py`) sono
  `multipart/alternative`: un fallback testuale (client senza HTML, screen
  reader) più una versione HTML con banner e `body_markdown` renderizzato
  a HTML (`app/domain/markdown_render.py`, `markdown` + sanificazione
  `nh3` — niente `<script>`/attributi `on*`/schema diverso da
  `http`/`https`/`mailto` in `href`/`src`, stessa cautela delle note/
  bibliografia). È l'unico punto del backend che renderizza Markdown lato
  server: i post lo fanno solo lato frontend. Le altre email di piattaforma
  (OTP, reset password) restano solo testo.

**Digest di piattaforma** (Super Admin/Amministratore,
`require_platform_admin`), stesse forme di sopra con `blog_id=None`:
`GET /admin/newsletter/stats`, `GET /admin/newsletter/campaigns`,
`POST /admin/newsletter/campaigns`, `PATCH`/`DELETE
/admin/newsletter/campaigns/{id}` (stesse regole di editabilità/annullo di
sopra), e `GET`/`PATCH /admin/newsletter/settings` — stesso schema
tri-state di sopra ma senza `newsletter_auto_notify_enabled` (non
applicabile a un digest non legato alla pubblicazione di un singolo blog),
sorgente `platform_config` invece di `Blog`.

## CORS

Il backend accetta chiamate dal browser dalle origini in `NOCT_CORS_ORIGINS`
(separate da virgola; default `http://localhost:3000`, origini esatte) più,
opzionalmente, `NOCT_CORS_ORIGIN_REGEX` (regex Python, `allow_origin_regex` di
Starlette) — necessaria per i sottodomini per-blog (`slug.notturni.eu`, vedi
`k8s/ingressroute.yaml`): un'origine esatta per ciascuno non è enumerabile in
anticipo. Esempio produzione: `https://([a-z0-9-]+\.)?notturni\.eu`.

## Health

**`GET /api/v1/health`** — pubblico, nessuna autenticazione. Esegue anche
`SELECT 1` sul database. Pensato per probe di readiness/liveness (Kubernetes,
compose healthcheck).

**`GET /api/v1/config`** — pubblico, nessuna autenticazione. Espone
`{"deployment_mode": "solo"|"platform", "turnstile_site_key": "..."|null}`
(`NOCT_DEPLOYMENT_MODE`/`NOCT_TURNSTILE_SITE_KEY`). `deployment_mode` è
usato da `frontend/src/app/admin/layout.tsx` per nascondere la voce Utenti
in modalità `solo`; `turnstile_site_key` da `CommentsSection` per sapere se
mostrare il widget captcha sui commenti aperti a tutti (`null` se l'istanza
non ne ha uno configurato) — entrambi senza dover già avere una sessione
autenticata. Site key, mai la secret key: pensata per essere pubblica.

**`GET /api/v1/seo/crawl-directives`** — pubblico, nessuna autenticazione.
Espone `{"search_disallow": ["/blog", "/blog/post", ...], "ai_disallow":
[...], "ai_user_agents": ["GPTBot", "ClaudeBot", ...]}` — percorsi relativi
da mettere in `Disallow` in robots.txt, calcolati da
`app/domain/seo.py::build_crawl_directives` a partire dagli opt-in per
crawler di blog/post (vedi sotto). `ai_disallow` include sempre anche tutto
`search_disallow`: un gruppo user-agent specifico in robots.txt sostituisce
interamente `User-agent: *` per quel bot, non lo integra. Solo blog `public`
non sospesi e post pubblicamente visibili (`is_publicly_visible`) — un blog
`members`/`private` non è comunque raggiungibile da un crawler anonimo.
Consumato da `frontend/src/app/robots.ts` (route speciale Next.js, genera
`/robots.txt`).

**`GET /api/v1/seo/sitemap-entries`** — pubblico, nessuna autenticazione.
Espone `{"blogs": [{"slug", "updated_at"}, ...], "posts": [{"permalink",
"updated_at"}, ...]}` (`app/domain/seo.py::build_sitemap_entries`) — solo
contenuto effettivamente indicizzabile (stesso criterio di
`search_disallow` sopra: un blog/post con `search_indexing_enabled`
effettivo a `false` non compare). Consumato da
`frontend/src/app/sitemap.ts` per generare `/sitemap.xml`.

## Errori comuni

| Caso                                        | Status |
| -------------------------------------------- | ------ |
| Header `Authorization` assente/malformato     | 401    |
| Sessione/token inesistente, revocato o scaduto| 401    |
| Credenziali di login errate                   | 401    |
| Regola di dominio violata (slug, limite blog)  | 400    |
| Slug già in uso                                | 409    |
| Permesso insufficiente (ruolo/proprietà)       | 403    |
| Risorsa non trovata                            | 404    |
