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

Entrambi usano l'header `Authorization: Bearer <valore>`, ma non sono
intercambiabili — ogni gruppo di endpoint richiede quello giusto:

| Meccanismo           | Formato token                                         | Endpoint che lo richiedono                                        | Ottenuto da                                 |
| --------------------- | ------------------------------------------------------ | -------------------------------------------------------------------- | ---------------------------------------------- |
| **Sessione utente**  | JWT firmato, breve durata (15 min)                    | `/auth/me`, `/auth/mfa/*`, `/blogs`, `/posts`, `/comments`          | login (`/auth/login`, SSO)                  |
| **API token**        | Opaco, prefisso `noct_`, nessuna scadenza di default    | `/tokens`                                                             | script di bootstrap o un token già valido    |

Il primo (sessione) rappresenta *chi sei* (un utente loggato dal browser o da
un client che ha fatto login); il secondo (API token) rappresenta *un accesso
diretto* — motore core oggi, in futuro anche utenti che vogliono
interfacciarsi con l'API senza passare da editor o admin del blog. Dettagli
sugli API token in fondo a questo file (sezione invariata rispetto alla
versione precedente).

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
l'identificatore citabile come `@username` nei contenuti.

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

- MFA non attiva: sessione diretta.
  ```json
  {"access_token": "...", "refresh_token": "...", "token_type": "bearer"}
  ```
- MFA attiva: richiede un secondo passaggio.
  ```json
  {"mfa_required": true, "method": "totp", "challenge": "..."}
  ```
  Se `method` è `"email"`, a questo punto è già stato accodato l'invio del
  codice (vedi limitazione email più sotto).

`401` su credenziali errate o utente disattivato.

**`POST /api/v1/auth/mfa/verify`** — completa il login dopo una risposta
`mfa_required`.

```json
{"challenge": "...", "code": "123456"}
```

→ `200` con `access_token`/`refresh_token` come sopra. `401` se il codice è
sbagliato/scaduto o il challenge non è più valido (dura 5 minuti).

**`POST /api/v1/auth/refresh`**

```json
{"refresh_token": "..."}
```

→ `200`, nuova coppia access/refresh token. Il refresh token usato viene
revocato (rotation): riusarlo dà `401`.

**`POST /api/v1/auth/logout`**

```json
{"refresh_token": "..."}
```

→ `204`. Revoca la sessione; idempotente (nessun errore se già revocata).

**`GET /api/v1/auth/me`** — richiede sessione. Ritorna
`{id, username, email, mfa_enabled}`.

### MFA — gestione (richiede una sessione attiva, cioè un login già fatto)

**`POST /api/v1/auth/mfa/totp/setup`** → genera un secret TOTP (non ancora
attivo) e lo ritorna insieme a un `provisioning_uri` (`otpauth://...`) da
mostrare come QR code:

```json
{"secret": "BASE32...", "provisioning_uri": "otpauth://totp/Notturni:mario%40example.com?..."}
```

**`POST /api/v1/auth/mfa/totp/confirm`** — `{"code": "123456"}` → `204` e
attiva l'MFA (`mfa_enabled=true`, `mfa_method="totp"`). `400` se il codice non
corrisponde al secret generato dal setup.

**`POST /api/v1/auth/mfa/email/setup`** → `202`, genera e accoda (RabbitMQ)
l'invio di un codice all'email dell'utente (vedi limitazione più sotto).

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
`/auth/login` (`access_token`/`refresh_token`).

**Limitazione nota:** senza credenziali OAuth reali (client id/secret per
ciascun provider) il flow non è testabile end-to-end in questo ambiente di
sviluppo. La logica di account linking è invece testata direttamente (vedi
`app/domain/sso.py`); gli endpoint `/login` e `/callback` andranno verificati
con credenziali reali prima di un uso in produzione — in particolare per
LinkedIn, i cui dettagli esatti dell'endpoint userinfo potrebbero richiedere
aggiustamenti.

**Limitazione nota (Email OTP):** il codice viene generato, salvato (hash) e
pubblicato su RabbitMQ (coda `email_otp`) correttamente, ma **non esiste
ancora un invio email reale** — nessun provider SMTP/transazionale è
configurato nel progetto. Il consumer in
`app/workers/email_otp_consumer.py` è un placeholder che logga il codice
invece di spedirlo. Da collegare a un provider reale prima di usare l'MFA via
email in produzione.

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
`visibility` (`public` | `members` | `private`), `allow_anonymous_comments`
(governa se i commenti sono aperti anche a chi non è registrato, vedi sezione
Commenti), `mentions_enabled` (bool — trasforma le `@username` nei post in
link, vedi sezione Post), `default_author_display_name` — nome pubblico
degli autori sui post di questo blog. Se valorizzato è **imposto** (nessun
override per singolo autore o post — todo/USERS.md #2), a meno che il
collaboratore non abbia un proprio alias di membership, che ha la precedenza.
Stringa vuota `""` lo azzera, assente lo lascia invariato.

**`POST /api/v1/blogs/{slug}/follow`** / **`DELETE .../follow`** — richiede
sessione. Segui/smetti di seguire un blog; idempotenti (`204` anche se già
nello stato richiesto).

**`GET /api/v1/blogs/{slug}/followers`** — pubblico. Lista `{username}` di chi
segue il blog.

**`GET /api/v1/blogs/{slug}/config`** — pubblico. Configurazione di
presentazione del blog (palette/tipografia/layout — vedi
[ROADMAP.md](../ROADMAP.md#2-estetica) per i vincoli). JSON libero; se il
proprietario non ha ancora salvato nulla, ritorna il default della
piattaforma:

```json
{
  "palette": {"background": "#fbf9f6", "foreground": "#2b2a28", "primary": "#3e6259", "muted": "#a8a29a", "border": "#e7e2da"},
  "typography": {"heading_font": "Lora", "body_font": "Inter"},
  "layout": "standard"
}
```

**`PUT /api/v1/blogs/{slug}/config`** — richiede sessione, solo il
proprietario (`403` altrimenti). Sostituisce l'intera configurazione (non è
un merge). Uniche regole imposte: `palette` al massimo 5 colori, al massimo 3
font distinti in `typography` (`400` altrimenti) — il resto della struttura
(`layout` e qualsiasi altra chiave) è libero.

**`POST /api/v1/blogs/{slug}/media`** — richiede sessione e accesso in
scrittura al blog (proprietario/autore/co-autore). `multipart/form-data`,
campo `file`. Formati ammessi: PNG, JPEG, WEBP, GIF; max 10 MiB (`400`
altrimenti). Immagine da incorporare nel Markdown di un post (es.
`![alt](url)`). Vedi "Media e backup" sotto per il path S3.

```json
{"url": "https://.../notturni/userdata/{user_uuid}/{blog_uuid}/media/{uuid}.png"}
```

**`GET /api/v1/blogs/{slug}/mentionable-users?q=<prefisso>&limit=8`** —
richiede sessione e accesso in scrittura al blog. Suggerimenti per
l'autocomplete delle `@menzioni` nell'editor: proprietario, collaboratori e
follower del blog il cui username inizia con `q` o il cui nome pubblico lo
contiene (`q` vuoto → primi risultati per username). `limit` 1–25 (default
8). Ritorna `[{"username": "...", "display_name": str|null}]`. Se il blog ha
`mentions_enabled=false`, ritorna sempre `[]`.

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
      {"post_title": "...", "post_slug": "...", "permalink": "/{blog}/{YYYYMMDD}/{slug}", "locale": "it", "idx": 1}
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
    "citations": [
      {"post_title": "...", "post_slug": "...", "permalink": "/{blog}/{YYYYMMDD}/{slug}", "locale": "it", "used_at": "2026-01-01T00:00:00Z"}
    ]
  }
]
```

`categories` è il sottoinsieme di `suggestive`/`nudity`/`explicit`/`other`
scelto dall'autore per quell'immagine (vedi "Avviso sui contenuti" nella
sezione Post) — vuoto se non segnalata o segnalata senza una categoria
specifica. `used_at` è la data di pubblicazione del post che la cita.

**`GET /api/v1/blogs/{slug}/links-bibliography`** — stesso principio, per i
link citati nel corpo dei post pubblicati:

```json
[
  {
    "url": "https://esempio.org/articolo",
    "link_text": "testo del link",
    "citations": [
      {"post_title": "...", "post_slug": "...", "permalink": "/{blog}/{YYYYMMDD}/{slug}", "locale": "it", "used_at": "2026-01-01T00:00:00Z"}
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
`/{blog_slug}/pagina/{slug}` per le pagine di blog o `/pages/{slug}` per
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
  `[{user_id, username, role, author_display_name, created_at}]`.
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

Stati: `draft` → (opzionale) `pending_review` → `published`. `published_at`
serve anche per la pianificazione: un post con `status=published` e
`published_at` nel futuro non è ancora pubblicamente visibile — vedi
`is_publicly_visible` più sotto.

Ogni post ha un **permalink leggibile**, senza UUID: `blog_slug` e
`permalink` sono calcolati ad ogni risposta (non colonne del modello) e
inclusi in ogni `PostOut`:

```text
permalink = /{blog_slug}/{YYYYMMDD}/{slug}
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
   Utenti): `full_name` (nome e cognome), `display_name` (alias globale) o
   `username` (default).

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
`true`, indipendentemente dal valore passato per quel campo. `tags` è
opzionale (vedi sezione "Tag" sotto).
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

**`GET /api/v1/blogs/{blog_slug}/posts/{YYYYMMDD}/{post_slug}`** — pubblico
(token opzionale), stesse regole di visibilità di `GET /posts/{post_id}`
sopra. Risolve il permalink leggibile (vedi sopra) verso il post: è
l'endpoint pensato per la pagina pubblica del post (es.
`https://notturni.eu/{blog_slug}/{YYYYMMDD}/{post_slug}`), che così non deve
mai esporre l'UUID nell'URL. `400` se la data non è nel formato `YYYYMMDD`,
`404` se blog/slug/data non corrispondono a nessun post (o a un post non
visibile per il chiamante).

**`PATCH /api/v1/posts/{post_id}`** — stessa autorizzazione della creazione.
Aggiorna
`title`/`content`/`cover_image_url`/`cover_image_is_sensitive`/`cover_image_categories`/`tags`/`category_id`/`notes`
(tutti opzionali). Se `content` cambia, accoda di nuovo il backup su S3 e
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
cui la distinzione esplicita assente/`null`/valore.

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

## Media e backup su S3

Tutto sotto un unico bucket (`NOCT_MINIO_BUCKET_CONTENT`), con un prefisso
`{NOCT_SITE_SLUG}/userdata/{user_uuid}/{blog_uuid}/...` — pensato per poter
condividere lo stesso bucket fisico tra più installazioni/scopi senza
collisioni.

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
la moderazione automatica delle immagini sopra). **Nessuna cache**: ogni
chiamata rifà il fetch (timeout 5s, corpo troncato a 512 KB) — un buon primo
caso d'uso per Redis quando verrà usato per la prima volta nel progetto
(oggi deployato ma non ancora sfruttato, vedi ROADMAP.md).

Usato dall'editor (`frontend/src/components/editor/LinkPreviewCard.tsx`)
quando si incolla un URL da solo: il link resta testo semplice/cancellabile,
la card è un blocco indipendente subito sotto, salvato nel Markdown come un
link con `title="card"` (`[url](url "card")`, stessa convenzione di
"sensitive" sulle immagini) — senza salvare uno snapshot di
titolo/descrizione/immagine, richiesti di nuovo ad ogni apertura
dell'editor o rendering della pagina pubblica.

## Commenti

Di default solo utenti registrati; il proprietario del blog può aprire ai non
registrati ma con moderazione obbligatoria in quel caso.

**`POST /api/v1/posts/{post_id}/comments`** — autenticazione opzionale:

- **con sessione valida:** commento attribuito all'utente, stato `approved`
  automaticamente (nessuna moderazione per utenti registrati).
  ```json
  {"content": "..."}
  ```
  `author_display_name` nella risposta segue la preferenza di profilo
  `post_author_name_style` (username/nome e cognome/alias globale — non
  l'alias di blog, che si applica solo ai post: un commento resta sempre a
  nome della persona, non del blog) ed è **ricalcolato ad ogni lettura**, non
  solo alla creazione: un cambio di username o di preferenza si riflette
  subito anche sui commenti passati.
- **senza sessione:** richiede che il blog abbia `allow_anonymous_comments=true`
  (altrimenti `401`); richiede `author_display_name` e `author_email` nel
  payload (altrimenti `400`); il commento è creato in stato `pending`.
  ```json
  {"content": "...", "author_display_name": "...", "author_email": "..."}
  ```

**`GET /api/v1/posts/{post_id}/comments`** — pubblico. Solo commenti
`approved`.

**`GET /api/v1/posts/{post_id}/comments/pending`** — richiede sessione ed
essere proprietario del blog o avere membership con ruolo `mediatore` (`403`
altrimenti). Coda di moderazione.

**`POST /api/v1/comments/{comment_id}/approve`** / **`.../reject`** — stessa
autorizzazione di `pending`. Cambiano lo stato del commento.

## Frammenti

Porzione di testo evidenziata dal lettore (con il mouse) su un post
pubblicato e salvata in una raccolta personale unificata — non legata
all'autore del post, che non deve fare nulla per abilitarla. Richiede sempre
sessione: un frammento appartiene a chi lo ha salvato, mai pubblico. Il testo
è salvato così com'è (non un offset nel post): il frontend lo ri-cerca nel
testo reso ad ogni lettura per ri-evidenziarlo (`lib/highlight-fragments.ts`),
tollerando piccole differenze di spazi bianchi ma non un post riscritto nel
frattempo — in quel caso il frammento resta salvato ma non più
ri-evidenziato in pagina. Ri-selezionare (anche solo in parte) un frammento
già evidenziato propone di rimuoverlo (`DELETE`) invece di salvarne uno
nuovo — interamente lato frontend (`Range.intersectsNode` sui `<mark>` già
presenti), nessun endpoint dedicato oltre alla `DELETE` già descritta sotto.

**`POST /api/v1/posts/{post_id}/fragments`** — richiede sessione. `404` se il
post non è pubblicato o non è visibile all'utente (stessa regola d'accesso
del permalink pubblico). `400` se il testo è vuoto o supera il **15%** della
lunghezza del Markdown grezzo del post (`app/domain/fragments.py`) — un
proxy della lunghezza del testo reso, che il backend non calcola mai (nessun
rendering Markdown lato server, vedi Post). Salvare due volte lo stesso
identico testo sullo stesso post è idempotente: ritorna lo stesso frammento,
non un errore.

```json
{"text": "il pezzo di testo evidenziato"}
```

```json
{"id": "...", "post_id": "...", "text": "...", "created_at": "..."}
```

**`GET /api/v1/posts/{post_id}/fragments`** — richiede sessione. Solo i
frammenti salvati dall'utente corrente su quel post (mai quelli di altri
utenti) — usato dal frontend per ri-evidenziarli ad ogni lettura,
indipendentemente dal fatto che si sia arrivati al post dalla pagina di
raccolta o da un link diretto.

**`GET /api/v1/users/me/fragments`** — richiede sessione. Raccolta unificata
di tutti i frammenti salvati dall'utente, più recenti prima: testo, data di
cattura, titolo del post e permalink pubblico, nome dell'autore. Quest'ultimo
è il valore già risolto al salvataggio/ultima modifica del post
(`Post.author_display_name`), non ricalcolato da un alias cambiato dopo come
invece fa `PostOut` — semplificazione accettata per questa vista derivata.

```json
[{"id": "...", "text": "...", "created_at": "...", "post_title": "...", "author_display_name": "...", "permalink": "/blog/20260101/post"}]
```

**`DELETE /api/v1/fragments/{fragment_id}`** — richiede sessione ed essere il
proprietario del frammento (`404` altrimenti, non `403`: non rivela
l'esistenza del frammento a chi non è suo). `204` se rimosso.

## Pagine statiche (sito principale)

Pagine come Chi siamo, Contatti, Privacy — non legate a un blog utente
(`blog_id: null` nella risposta), gestite dal team della piattaforma, sempre
attive (a differenza delle pagine di blog, opt-in — vedi "Pagine statiche del
blog" più sotto). Stesso schema di traduzione dei post (sezione Multilingua
sopra). Permalink pubblico `/pages/{slug}` (prefisso dedicato per non
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

**`GET /api/v1/pages/{slug}?locale=it`** — pubblico (token opzionale): senza
sessione admin, solo pagine `is_published=true` (`404` altrimenti, bozza o
inesistente). Con sessione admin, anche le bozze — per poterle rivedere
prima di pubblicarle.

**`GET /api/v1/pages?locale=it`** — pubblico (token opzionale): stessa
distinzione pubblicate/tutte in base al ruolo del chiamante.

**`PATCH /api/v1/pages/{page_id}`** — richiede ruolo admin. Aggiorna una
singola traduzione (`slug`, `title`, `content`, `is_published`, tutti
opzionali).

## Profilo utente e follow

**`GET /api/v1/users/{username}`** — pubblico. Profilo pubblico:

```json
{
  "username": "...", "bio": "...",
  "first_name": "...", "last_name": "...", "display_name": "...",
  "post_author_name_style": "username",
  "country": "IT", "native_language": "it", "fallback_languages": ["en", "fr"],
  "avatar_url": "...", "social_links": [...], "created_at": "..."
}
```

`display_name` è un alias pubblico globale (todo/BLOG.md #4): quando
valorizzato, è l'intestazione del profilo pubblico al posto di username /
nome e cognome.
`post_author_name_style` (todo/USERS.md #2) è la preferenza dell'utente su
cosa mostrare come nome autore sui propri post — `username` (default),
`full_name` (nome e cognome) o `display_name` (alias globale) — applicata
solo quando il blog non impone un nome pubblico (vedi sezione Post).
`first_name`/`last_name`/`country`/`native_language` sono liberi/opzionali.
`country` è solo controllato nel formato (ISO 3166-1 alpha-2, es. `IT`, non
verificato contro un elenco ufficiale dei paesi — vedi
`app/domain/profile.py`). `native_language`/`fallback_languages` sono codici
ISO 639-1 di 2 lettere, stesso formato del `locale` di post/pagine (vedi
sezione Multilingua). `fallback_languages` sono pensate anche come le lingue
verso cui l'utente potrà eventualmente tradurre i propri contenuti; massimo
5.

**`PATCH /api/v1/users/me`** — richiede sessione. Aggiorna `username`, `bio`,
`first_name`, `last_name`, `display_name`, `post_author_name_style`,
`country`, `native_language`, `fallback_languages` (tutti opzionali). Per
`first_name`/`last_name`/`display_name`/`country`/`native_language`: stringa
vuota `""` azzera il campo, assente lo lascia invariato, qualsiasi altro
valore lo sostituisce (`400` se il formato di `country`/`native_language` non
è valido). `post_author_name_style`: uno tra `username` | `full_name` |
`display_name` (`422` altrimenti), assente lo lascia invariato. Per
`fallback_languages`: assente lascia invariata la lista, una lista (anche
vuota) la sostituisce (`400` se oltre 5 o un codice non valido). `username`:
assente lo lascia invariato, altrimenti stesso formato/blacklist della
registrazione (`app/domain/usernames.py`, `400` se non valido, `409` se già
in uso); l'id resta la vera chiave con cui il resto del sistema referenzia
l'utente, quindi il cambio è visibile subito ovunque (post, commenti,
autocomplete `@menzioni`) — eccetto le `@menzioni` già scritte nel testo di
post/pagine esistenti, salvate come testo semplice e non riscritte.

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
altrimenti). Carica su MinIO/S3 (endpoint custom sempre iniettato, bucket
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

## API token (motore core / accesso diretto)

Sezione invariata rispetto alla versione precedente di questo documento.

Il token è un valore opaco (non un JWT decodificabile), generato con prefisso
`noct_`; solo il suo hash sha256 è persistito in database (tabella
`api_tokens`, modello `app/models/api_token.py`). Ogni token ha un
`owner_type`:

- **`core`** — token del motore/servizio, non legato a un utente. Pensato per
  chiamate machine-to-machine: worker interni, script di manutenzione, task
  pianificati.
- **`user`** — (predisposto per il futuro) token legato a uno `User`
  specifico, per permettere all'utente di interfacciarsi con l'API senza
  passare dall'editor del frontend o dall'admin del proprio blog.

Un token può generare altri token solo con lo stesso `owner_type` (e, se
`user`, per lo stesso utente).

### Come ottenere il primo token

```bash
cd backend && source .venv/bin/activate
python -m scripts.create_api_token --name "core-engine"
```

Il valore in chiaro viene stampato una sola volta.

### `POST /api/v1/tokens`

Crea un nuovo token con lo stesso `owner_type` (e utente, se applicabile) del
token usato per autenticare la richiesta.

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
`amministratore`/`super_admin` (`403` altrimenti).

**`GET /api/v1/admin/users`** — lista tutti gli utenti della piattaforma
(id, username, email, `platform_role`, `is_active`, `mfa_enabled`).

**`PATCH /api/v1/admin/users/{user_id}`** — `{platform_role?, is_active?}`.

- Assegnare o rimuovere i ruoli `amministratore`/`super_admin` (sia come
  valore di partenza che di arrivo) richiede essere `super_admin`: un
  `amministratore` può gestire solo `utente`/`moderatore` (`403` altrimenti).
- Non è possibile disattivare il proprio stesso account (`400`) — evita
  l'auto-blocco dell'unico Super Admin rimasto.
- Un utente disattivato (`is_active=false`) non può più fare login.

Non esiste (ancora) un endpoint per creare il primo Super Admin: va promosso
manualmente sul database dopo la registrazione — task aperto, vedi
[ROADMAP.md](../ROADMAP.md#1-prodotto-e-regole-di-dominio) (bootstrap via
env/secret al momento della creazione delle risorse, non ancora
automatizzato in questi script).

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
`limit` (default 20, massimo 50), `offset` (paginazione, default 0). Router
separato da `/blogs/{slug}/posts` apposta: qui i post attraversano blog
diversi, non sono scoped a uno slug/id specifico.

**`GET /api/v1/feed/trending`** — pubblico, nessuna autenticazione. Tag più
usati tra i post pubblicati (dei soli blog `public`) negli ultimi `days`
giorni (default 7, massimo 90), dal più frequente:
`[{"tag": "poesia", "post_count": 12}, ...]`. Query
param opzionali: `days`, `limit` (default 10, massimo 30). Non esistono
ancora contatori di like/condivisioni in piattaforma (vedi ROADMAP.md): è
l'unica base disponibile oggi per una sezione "di tendenza".

## CORS

Il backend accetta chiamate dal browser solo dalle origini in
`NOCT_CORS_ORIGINS` (separate da virgola; default `http://localhost:3000`).
In produzione copre solo un'origine esatta — non i sottodomini per-blog
(`nomeutente.notturni.eu`): da rivedere quando le pagine pubbliche dei blog
chiameranno l'API direttamente dal browser.

## Health

**`GET /api/v1/health`** — pubblico, nessuna autenticazione. Esegue anche
`SELECT 1` sul database. Pensato per probe di readiness/liveness (Kubernetes,
compose healthcheck).

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
