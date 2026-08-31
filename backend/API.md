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
{"slug": "il-mio-blog", "title": "Il mio blog", "default_locale": "it"}
```

→ `201`. Applica le regole di dominio (vedi
[ROADMAP.md](../ROADMAP.md#1-prodotto-e-regole-di-dominio)): slug di almeno 4
caratteri alfanumerici/trattino, non in blacklist (vedi
`app/domain/blog_rules.py`), massimo 5 blog per utente. `default_locale` è
opzionale (default `it`).
`400` se una regola non è rispettata, `409` se lo slug è già in uso.

**`GET /api/v1/blogs/mine`** — richiede sessione. Lista i blog di proprietà
dell'utente (non ancora quelli su cui ha solo una membership — vedi nota
sotto).

**`GET /api/v1/blogs/{slug}`** — pubblico. Dettaglio del blog.

**`PATCH /api/v1/blogs/{slug}`** — richiede sessione, solo il proprietario
(`403` altrimenti). Campi aggiornabili: `title`, `allow_anonymous_comments`
(quest'ultimo governa se i commenti sono aperti anche a chi non è registrato,
vedi sezione Commenti), `default_author_display_name` — nome pubblico
predefinito per i testi scritti su questo blog (CLAUDE.md #1: il nome
dell'autore può differire dal nome utente reale), usato come default di
`Post.author_display_name` quando non specificato esplicitamente in
creazione (vedi sezione Post): stringa vuota `""` lo azzera (si torna al
fallback sullo username), assente lo lascia invariato.

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

## Post

Il contenuto (`content`) è **Markdown**: nessun rendering lato backend, la
conversione a HTML (con sanificazione) è responsabilità del frontend al
momento della lettura.

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
{"slug": "primo-post", "title": "...", "content": "# Markdown...", "author_display_name": null, "locale": null, "cover_image_url": null, "tags": null}
```

`author_display_name` è opzionale: se omesso usa lo username, ma può essere
diverso — il nome pubblico dell'autore può non coincidere con l'utente reale
(vedi [ROADMAP.md](../ROADMAP.md#1-prodotto-e-regole-di-dominio)). `locale`
è opzionale: se omesso usa il `default_locale` del
blog. `cover_image_url` è opzionale: l'URL ritornato da un precedente upload
su `POST /blogs/{slug}/media` (vedi sotto) — il campo accetta qualsiasi
stringa, non verifica che punti davvero a un media caricato su questo blog.
`cover_image_is_sensitive` (default `false`) riprende l'esito della
moderazione automatica ricevuto in quella stessa risposta di upload — vedi
sezione "Moderazione automatica delle immagini" più sotto; non viene
ricalcolato qui. `tags` è opzionale (vedi sezione "Tag" sotto). `409` se lo
slug è già in uso su quel blog per quella lingua.

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

In `PATCH /posts/{post_id}` (vedi sotto): `tags` assente lascia invariati i
tag del campo dedicato; una lista (anche vuota, `[]`) li sostituisce. Gli
hashtag nel testo vengono invece ricalcolati **ad ogni modifica del
contenuto**, a prescindere da questo campo.

**`POST /api/v1/posts/{post_id}/translations`** — stessa autorizzazione della
creazione. Aggiunge una traduzione alla stessa famiglia del post indicato
(`translation_group_id` condiviso), come post `draft` indipendente con il
proprio slug:

```json
{"slug": "my-post", "locale": "en", "title": "...", "content": "...", "author_display_name": null}
```

`409` se esiste già una traduzione per quella lingua nella famiglia, o se lo
slug è già in uso su quel blog per quella lingua.

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
tutte le lingue.

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
Aggiorna `title`/`content`/`cover_image_url`/`cover_image_is_sensitive`/`tags`
(tutti opzionali). Se `content` cambia, accoda di nuovo il backup su S3. Per
`cover_image_url`: valore assente (`null`/campo omesso) lascia la cover
invariata, stringa vuota `""` la rimuove (azzerando anche
`cover_image_is_sensitive`, indipendentemente da cosa viene passato per
quel campo), qualsiasi altro valore la sostituisce. Per `tags`, vedi
sezione "Tag" sopra.

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

## Commenti

Di default solo utenti registrati; il proprietario del blog può aprire ai non
registrati ma con moderazione obbligatoria in quel caso.

**`POST /api/v1/posts/{post_id}/comments`** — autenticazione opzionale:

- **con sessione valida:** commento attribuito all'utente, stato `approved`
  automaticamente (nessuna moderazione per utenti registrati).
  ```json
  {"content": "..."}
  ```
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

## Pagine statiche (sito principale)

Pagine come Chi siamo, Contatti, Privacy — non legate a un blog utente,
gestite dal team della piattaforma. Stesso schema di traduzione dei post
(sezione Multilingua sopra).

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
  "first_name": "...", "last_name": "...",
  "country": "IT", "native_language": "it", "fallback_languages": ["en", "fr"],
  "avatar_url": "...", "social_links": [...], "created_at": "..."
}
```

`first_name`/`last_name`/`country`/`native_language` sono liberi/opzionali.
`country` è solo controllato nel formato (ISO 3166-1 alpha-2, es. `IT`, non
verificato contro un elenco ufficiale dei paesi — vedi
`app/domain/profile.py`). `native_language`/`fallback_languages` sono codici
ISO 639-1 di 2 lettere, stesso formato del `locale` di post/pagine (vedi
sezione Multilingua). `fallback_languages` sono pensate anche come le lingue
verso cui l'utente potrà eventualmente tradurre i propri contenuti; massimo
5.

**`PATCH /api/v1/users/me`** — richiede sessione. Aggiorna `bio`,
`first_name`, `last_name`, `country`, `native_language`,
`fallback_languages` (tutti opzionali). Per `country`/`native_language`:
stringa vuota `""` azzera il campo, assente lo lascia invariato, qualsiasi
altro valore lo sostituisce (`400` se il formato non è valido). Per
`fallback_languages`: assente lascia invariata la lista, una lista (anche
vuota) la sostituisce (`400` se oltre 5 o un codice non valido).

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
pubblicati (e con `published_at` raggiunto) di **tutti i blog**, dal più
recente — pensato per la homepage della piattaforma (CLAUDE.md #2:
"raccolta degli articoli nella lingua dell'utente, stile dev.to").

Query param opzionali: `locale` (filtra una lingua, altrimenti tutte
insieme), `tag` (filtra per tag normalizzato, es. `poesia` non `#Poesia` —
vedi sezione "Tag" sopra), `limit` (default 20, massimo 50), `offset`
(paginazione, default 0). Router separato da `/blogs/{slug}/posts` apposta:
qui i post attraversano blog diversi, non sono scoped a uno slug/id
specifico.

**`GET /api/v1/feed/trending`** — pubblico, nessuna autenticazione. Tag più
usati tra i post pubblicati negli ultimi `days` giorni (default 7, massimo
90), dal più frequente: `[{"tag": "poesia", "post_count": 12}, ...]`. Query
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
