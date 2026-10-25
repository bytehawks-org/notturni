# Riferimento API — endpoint per modulo

Elenco di tutti gli endpoint esposti dal backend, ricavato dai manifest `api.toml` di ogni modulo (212 rotte in 24 moduli).
Complementa [API.md](API.md), che descrive payload, regole di dominio ed esempi dei flussi principali; qui l'obiettivo è
sapere **quali rotte esistono, con quale verbo, a chi sono aperte e perché esistono**. Lo schema machine-readable completo
(tipi di richiesta/risposta) è servito da `GET /api/v1/openapi.json` del backend in esecuzione.

## Convenzioni

- **Prefisso**: tutti i percorsi sono relativi a `/api/v1` (es. `POST /auth/login` → `POST /api/v1/auth/login`), salvo `/storage` nella sezione «Piattaforma».
- **Alias**: `/spaces/…` equivale a `/blogs/…` (un «blog» del contratto è uno *Spazio*: `blog`, `wiki` o `docs`).
- **Formato**: JSON; gli errori hanno corpo `{"detail": "…"}` con lo status HTTP appropriato.
- **Autenticazione**: header `Authorization: Bearer <access token>` (login) o `Bearer noct_…` (token API, solo dove indicato).
  Il refresh token è un cookie `HttpOnly`.
- **Verbi**: `GET` legge (idempotente, senza effetti); `POST` crea o avvia un'azione/un job; `PUT` sostituisce una risorsa o un insieme
  per intero; `PATCH` modifica parzialmente (campi assenti = invariati, `null`/`""` azzerano dove documentato); `DELETE` rimuove.
- **Status**: se non indicato, `200` per `GET`/`PUT`/`PATCH`, `201` per creazioni, `204` senza corpo; `202` = accodato (l'esito arriva in modo asincrono).

### Colonna «Accesso»

| Valore | Significato |
|---|---|
| pubblica | Nessuna autenticazione. Le risorse di uno Spazio rispettano comunque la sua visibilità (`public` · `members` · `private`). |
| utente | Qualunque utente autenticato. |
| utente o token API | Utente autenticato oppure token API con lo scope adatto. |
| scrittura sullo Spazio | Utente con permesso di scrittura sullo Spazio indicato nel percorso (proprietario, co-autore, ecc.); la rotta dichiara `scope = blog`. |
| ruolo: … | Ruolo di piattaforma richiesto (super admin, amministratore, moderatore). |

La colonna **Note** riporta lo status diverso dal predefinito, il **rate limit** (`massimo / finestra`, per `ip` o per `user`)
e gli eventuali flag: *raw* = handler che gestisce direttamente la risposta (cookie, download, stream); *upload* = multipart.

## Piattaforma (rotte del server, non di un modulo)

| Verbo | Percorso | Accesso | Descrizione |
|---|---|---|---|
| GET | `/health` | pubblica | Liveness: il processo risponde. Usata dalle probe. |
| GET | `/health/ready` | pubblica | Readiness: verifica database e servizi di infrastruttura prima di ricevere traffico. |
| GET | `/openapi.json` | pubblica | Documento OpenAPI generato dai manifest dei moduli attivi. |
| GET | `/storage/{bucket}/…` (fuori da `/api/v1`) | pubblica | Solo con `NOCT_STORAGE_BACKEND=localstorage`: serve i bucket pubblici dal filesystem. |


## admin  (core)

Amministrazione di piattaforma: panoramica, utenti, blog, post, registro di audit e impostazioni. I moduli opzionali vi aggiungono contatori e impostazioni con `AdminSection`.

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| GET | `/admin/overview` | ruolo: super admin, amministratore, moderatore | KPI, code aperte, audit di oggi e salute dei servizi |  |
| GET | `/admin/users` | ruolo: super admin, amministratore | Utenti con numero di blog e ultimo accesso (`q` su username/email) |  |
| PATCH | `/admin/users/{user_id}` | ruolo: super admin, amministratore | Ruolo e attivazione (nota obbligatoria; i ruoli di amministrazione solo dal Super Admin) |  |
| POST | `/admin/users/{user_id}/reset-password` | ruolo: super admin, amministratore | Avvia per l'utente lo stesso ciclo email della password dimenticata | 202; rate limit 20/5 min per user |
| GET | `/admin/blogs` | ruolo: super admin, amministratore | Tutti i blog con contatori; filtri `q`, `visibility`, `state` (active\|suspended\|paused\|deleted\|reported) |  |
| PATCH | `/admin/blogs/{blog_id}` | ruolo: super admin, amministratore | Sospende o riattiva un blog (nota obbligatoria se cambia) |  |
| GET | `/admin/posts` | ruolo: super admin, amministratore | Tutti i post con segnalazioni aperte (`q`) |  |
| PATCH | `/admin/posts/{post_id}` | ruolo: super admin, amministratore | Nasconde o mostra un post (nota obbligatoria se cambia) |  |
| GET | `/admin/audit-log` | ruolo: super admin, amministratore | Registro di audit, dal più recente; filtri action, actor_id, target_id, blog_id, channel, since, until, limit (max 500), offset |  |
| GET | `/admin/config` | ruolo: super admin | Impostazioni di piattaforma, comprese quelle dei moduli attivi |  |
| PATCH | `/admin/config` | ruolo: super admin | Aggiorna le impostazioni; ogni modifica va nel registro di audit con i valori precedenti |  |
| GET | `/admin/modules` | ruolo: super admin | Moduli compilati nel binario con stato voluto e in esecuzione |  |
| POST | `/admin/modules/{name}/enable` | ruolo: super admin | Attiva un modulo opzionale (a caldo: migra lo schema, monta le rotte, avvia i consumer) |  |
| POST | `/admin/modules/{name}/disable` | ruolo: super admin | Disattiva un modulo opzionale (rotte smontate, dati e tabelle intatti; rifiutato se un altro modulo attivo lo richiede) |  |

## auth  (core)

Registrazione, login con sessione (access JWT + refresh con rotation), password dimenticata, cambio password, API token.

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| POST | `/auth/register` | pubblica | Registra un utente | 201; rate limit 30/10 min per ip |
| GET | `/auth/username-available` | pubblica | Verifica la disponibilità di uno username (?username=) | rate limit 30/1 min per ip |
| POST | `/auth/login` | pubblica | Login con email e password; il refresh token viaggia in un cookie httpOnly | raw |
| POST | `/auth/refresh` | pubblica | Rinnova la sessione dal cookie di refresh (rotation); richiede X-CSRF-Token | raw |
| POST | `/auth/logout` | pubblica | Revoca la sessione corrente; richiede X-CSRF-Token | 204; raw |
| GET | `/auth/me` | utente | L'utente della sessione corrente |  |
| POST | `/auth/password/forgot` | pubblica | Invia un codice di reset (risponde sempre 202, l'email non è enumerabile) | 202 |
| POST | `/auth/password/reset` | pubblica | Imposta una nuova password con il codice ricevuto | 204 |
| POST | `/users/me/password` | utente | Cambia la password conoscendo quella attuale; revoca tutte le sessioni | rate limit 10/5 min per user |
| POST | `/tokens` | utente o token API | Emette un API token (mostrato una sola volta) | 201 |
| GET | `/tokens` | utente o token API | I token dello stesso proprietario |  |
| DELETE | `/tokens/{token_id}` | utente o token API | Revoca un token del proprio proprietario | 204 |

## blogs  (core)

Blog, collaboratori e inviti, ciclo di vita (pausa, cancellazione con tolleranza, trasferimento), configurazione di presentazione.

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| POST | `/blogs` | utente | Crea uno Spazio (`kind` = blog\|wiki\|docs; slug di almeno 4 caratteri, non riservato; limite per utente su tutti gli Spazi) | 201 |
| GET | `/blogs/mine` | utente | Blog di proprietà dell'utente |  |
| GET | `/blogs/member-of` | utente | Blog altrui su cui l'utente ha una membership |  |
| GET | `/blogs/received-invitations` | utente | Inviti a collaborare in attesa di risposta |  |
| POST | `/blogs/received-invitations/{invitation_id}/accept` | utente | Accetta un invito (crea la membership) |  |
| POST | `/blogs/received-invitations/{invitation_id}/decline` | utente | Rifiuta un invito |  |
| GET | `/blogs/{slug}` | pubblica | Dettaglio del blog (404 se la visibilità non lo consente) |  |
| PATCH | `/blogs/{slug}` | utente | Modifica il blog (solo il proprietario) |  |
| DELETE | `/blogs/{slug}` | utente | Cancellazione con tolleranza (richiede di ridigitare lo slug) |  |
| POST | `/blogs/{slug}/restore` | utente | Annulla la cancellazione entro il periodo di tolleranza |  |
| POST | `/blogs/{slug}/transfer` | utente | Trasferisce la proprietà a un coautore |  |
| GET | `/blogs/{slug}/members` | utente | Collaboratori del blog (solo il proprietario) |  |
| PATCH | `/blogs/{slug}/members/{user_id}` | utente | Cambia il ruolo di un collaboratore (co_autore o mediatore) |  |
| DELETE | `/blogs/{slug}/members/{user_id}` | utente | Rimuove un collaboratore (idempotente) | 204 |
| PATCH | `/blogs/{slug}/my-membership` | utente | Il collaboratore sceglie l'alias con cui firma i post su questo blog |  |
| GET | `/blogs/{slug}/invitations` | utente | Inviti emessi dal blog (solo il proprietario) |  |
| POST | `/blogs/{slug}/invitations` | utente | Invita un utente come co_autore o mediatore | 201 |
| DELETE | `/blogs/{slug}/invitations/{invitation_id}` | utente | Revoca un invito in attesa | 204 |
| GET | `/blogs/{slug}/config` | pubblica | Configurazione di presentazione (segue la visibilità del blog) |  |
| PUT | `/blogs/{slug}/config` | utente | Sostituisce l'intera configurazione (solo il proprietario) |  |

## media  (core)

Immagini dei blog: upload per l'editor con libreria media, copertina e favicon del blog, limite di spazio per blog, moderazione automatica (fail open).

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| POST | `/blogs/{slug}/media` | scrittura sullo Spazio | Immagine da incorporare nel Markdown (PNG, JPEG, WEBP, GIF; max 10 MiB) | 201; upload |
| GET | `/blogs/{slug}/media` | utente | Libreria media del blog (proprietario e collaboratori) |  |
| POST | `/blogs/{slug}/media/sync` | scrittura sullo Spazio | Importa nella libreria le immagini citate nei post che non hanno ancora una riga |  |
| PATCH | `/blogs/{slug}/media/{media_id}` | scrittura sullo Spazio | Testo alternativo, didascalia e avviso sui contenuti |  |
| DELETE | `/blogs/{slug}/media/{media_id}` | scrittura sullo Spazio | Cancella un'immagine non più usata (409 se è in un post o è la copertina) | 204 |
| POST | `/blogs/{slug}/cover-image` | utente | Copertina del blog (solo il proprietario) | 201; upload |
| PATCH | `/blogs/{slug}/cover-image` | utente | Avviso sui contenuti e testo alternativo della copertina già caricata |  |
| DELETE | `/blogs/{slug}/cover-image` | utente | Rimuove la copertina (l'oggetto su storage resta) |  |
| POST | `/blogs/{slug}/favicon` | utente | Favicon del blog (PNG, JPEG, WEBP; max 512 KiB; nessuna moderazione) | 201; upload |
| DELETE | `/blogs/{slug}/favicon` | utente | Rimuove la favicon e cancella l'oggetto su storage |  |
| GET | `/users/me/media` | utente | Librerie media di tutti i miei blog (proprietario o collaboratore), con il blog di appartenenza |  |

## platform  (core)

Impostazioni di piattaforma (riga unica): limiti, nomi riservati, soglia di moderazione, footer. Si modificano da `/admin/config` (modulo core `admin`).

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| GET | `/config` | pubblica | Configurazione pubblica dell'istanza: lingua, registrazioni, captcha, provider di accesso |  |
| GET | `/footer` | pubblica | Footer di piattaforma in Markdown grezzo (le colonne 1-2 sono sovrascrivibili per blog) |  |

## posts  (core)

Post: bozze, revisione, pubblicazione e pianificazione, traduzioni, tag, media e link citati, permalink.

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| POST | `/blogs/{slug}/posts` | scrittura sullo Spazio | Crea un post (bozza) | 201 |
| GET | `/blogs/{slug}/posts` | pubblica | Post del blog: i pubblicati a tutti, anche bozze e pianificati a chi può scrivere (`?locale=`) |  |
| GET | `/blogs/{slug}/search` | pubblica | Ricerca nei post pubblicati del blog (`?q=&limit=&offset=`) |  |
| GET | `/blogs/{slug}/posts/{post_slug}` | pubblica | Risolve il permalink pubblico /{blog}/{post} |  |
| GET | `/blogs/{slug}/media-bibliography` | pubblica | Immagini citate nei post pubblicati, raggruppate per URL |  |
| GET | `/blogs/{slug}/links-bibliography` | pubblica | Link citati nei post pubblicati, raggruppati per URL |  |
| GET | `/posts/{post_id}` | pubblica | Dettaglio di un post per id (404 se non visibile) |  |
| PATCH | `/posts/{post_id}` | utente | Modifica un post |  |
| POST | `/posts/{post_id}/translations` | utente | Aggiunge la traduzione di un post | 201 |
| GET | `/posts/{post_id}/translations` | pubblica | Traduzioni pubblicate di un post (per il selettore di lingua) |  |
| POST | `/posts/{post_id}/submit-for-review` | utente | Manda una bozza in revisione |  |
| POST | `/posts/{post_id}/return-to-draft` | utente | Il revisore o il proprietario rimanda in bozza un post in revisione |  |
| POST | `/posts/{post_id}/publish` | utente | Pubblica (subito o pianificato) |  |
| POST | `/posts/{post_id}/unpublish` | utente | Toglie dalla pubblicazione un post pubblicato (o pianificato): torna in bozza |  |
| DELETE | `/posts/{post_id}` | utente | Elimina definitivamente un post (con commenti, note, frammenti e traduzioni solo di quel post) | 204 |
| GET | `/users/me/posts` | utente | Tutti i miei post (qualunque stato) di tutti i miei blog, dal più recente |  |
| GET | `/users/me/links` | utente | Link citati nei post (qualunque stato) di tutti i miei blog, raggruppati per URL |  |
| GET | `/seo/crawl-directives` | pubblica | Percorsi da escludere in robots.txt (opt-out per blog e per post, motori di ricerca e IA separati) |  |
| GET | `/seo/sitemap-entries` | pubblica | Blog e post indicizzabili per sitemap.xml |  |
| GET | `/blogs/{slug}/export` | utente | ZIP con tutto il contenuto del blog in formati aperti (solo il proprietario); i binari delle immagini restano sullo storage | raw |

## profiles  (core)

Profilo utente: anagrafica pubblica e privata, avatar, link social, cambio email in tre passi, directory degli utenti, blog e post pubblici di un utente.

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| GET | `/users` | pubblica | Directory pubblica degli utenti (`q`, `locale`, `interest`, `sort` = new\|followers, `limit` max 100, `offset`) |  |
| GET | `/users/me` | utente | Profilo privato del proprietario (con l'email) |  |
| PATCH | `/users/me` | utente | Aggiorna il profilo; i campi assenti restano invariati, "" azzera i campi testuali |  |
| GET | `/users/{username}` | pubblica | Profilo pubblico, per username o per dominio personale verificato |  |
| POST | `/users/me/avatar` | utente | Avatar (PNG, JPEG, WEBP; max 2 MiB) | rate limit 20/10 min per user; upload |
| DELETE | `/users/me/avatar` | utente | Rimuove l'avatar | 204 |
| POST | `/users/me/social-links` | utente | Aggiunge un link social (max 5) | 201 |
| DELETE | `/users/me/social-links/{link_id}` | utente | Rimuove un link social | 204 |
| POST | `/users/me/email/request` | utente | Passo 1/3 del cambio email: codice alla casella attuale | 202; rate limit 5/10 min per user |
| POST | `/users/me/email/verify-current` | utente | Passo 2/3: verifica il codice della casella attuale e invia quello alla nuova | 202; rate limit 10/10 min per user |
| DELETE | `/users/me/email/request` | utente | Annulla il cambio email in corso, a qualunque passo | 204 |
| POST | `/users/me/email/verify-new` | utente | Passo 3/3: verifica il codice della nuova casella e applica il cambio | rate limit 10/10 min per user |
| GET | `/users/{username}/blogs` | pubblica | Blog pubblici dell'utente; i blog con un alias diverso dallo username non compaiono |  |
| GET | `/users/{username}/posts` | pubblica | Post pubblicati dell'utente, solo quelli firmati con lo username (`limit` max 50, `offset`) |  |

## auth_mfa  (opzionale)

Autenticazione a due fattori: app TOTP o codice via email, sfida al login.

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| POST | `/auth/mfa/verify` | pubblica | Completa il login con il codice MFA; il refresh token viaggia in un cookie httpOnly | rate limit 20/5 min per ip; raw |
| POST | `/auth/mfa/totp/setup` | utente | Genera un nuovo segreto TOTP (attivo solo dopo la conferma) |  |
| POST | `/auth/mfa/totp/confirm` | utente | Conferma il segreto con un codice e attiva l'MFA via app | 204; rate limit 20/5 min per ip |
| POST | `/auth/mfa/email/setup` | utente | Invia un codice alla tua email per confermare l'MFA via email | 202; rate limit 5/5 min per user |
| POST | `/auth/mfa/email/confirm` | utente | Conferma il codice ricevuto e attiva l'MFA via email | 204; rate limit 20/5 min per ip |
| POST | `/auth/mfa/disable` | utente | Disattiva l'MFA: senza `code` 202 (con il metodo email spedisce il codice), con `code` valido 204 | rate limit 20/5 min per ip; raw |

## auth_sso  (opzionale)

Accesso con provider esterni (Google, Microsoft, GitHub, LinkedIn) e collegamento degli account per email.

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| GET | `/auth/sso/{provider}/login` | pubblica | Avvia l'accesso con un provider: redirect all'autorizzazione (state e PKCE in un cookie httpOnly) | rate limit 30/5 min per ip; raw |
| GET | `/auth/sso/{provider}/callback` | pubblica | Chiude l'accesso: sessione e redirect al frontend, oppure la sfida MFA per collegare un account esistente | rate limit 30/5 min per ip; raw |

## comments  (opzionale)

Commenti ai post: registrati senza moderazione, anonimi (captcha) moderati, risposte, segnalazione, autori bloccati; modalità per blog e per post.

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| POST | `/posts/{post_id}/comments` | pubblica | Commenta un post (registrati: subito pubblicato; anonimi: captcha e moderazione) | 201; rate limit 30/1 min per ip |
| GET | `/users/{username}/comments` | pubblica | Commenti approvati dell'utente su post pubblici, solo quelli firmati con lo username (`limit` max 50, `offset`) |  |
| GET | `/posts/{post_id}/comments` | pubblica | Commenti approvati di un post |  |
| GET | `/posts/{post_id}/comments/pending` | utente | Commenti in attesa di moderazione di un post (proprietario, mediatore, moderatori di piattaforma) |  |
| GET | `/blogs/{slug}/comments` | utente | Commenti di tutti i post del blog (`?status=` default pending, `?reported=true`) |  |
| POST | `/comments/{comment_id}/approve` | utente | Approva un commento |  |
| POST | `/comments/{comment_id}/reject` | utente | Rifiuta un commento |  |
| POST | `/comments/{comment_id}/report` | utente | Segnala il commento ai moderatori di piattaforma, con una nota |  |
| POST | `/comments/{comment_id}/block-author` | utente | Blocca l'autore del commento nel blog e rifiuta il commento |  |
| GET | `/blogs/{slug}/blocked` | utente | Autori bloccati del blog |  |
| DELETE | `/blogs/{slug}/blocked/{block_id}` | utente | Sblocca un autore | 204 |
| GET | `/admin/comments` | ruolo: super admin, amministratore, moderatore | Commenti di tutti i blog (`status`, default pending; `reported=true` per i segnalati; `q`) |  |

## custom_domains  (opzionale)

Dominio personale dell'utente verificato con un record TXT sul DNS: dà il sigillo di verifica «bronzo» e rende il profilo raggiungibile anche dal dominio.

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| POST | `/users/me/domain` | utente | Registra (o sostituisce) il dominio, in attesa di verifica, e restituisce il record TXT da pubblicare | rate limit 20/1 h per user |
| POST | `/users/me/domain/verify` | utente | Interroga il DNS: se il record TXT combacia verifica il dominio e assegna il sigillo bronzo | rate limit 5/10 min per user |
| DELETE | `/users/me/domain` | utente | Rimuove il dominio (anche verificato) | 204 |

## discovery  (opzionale)

Scoperta: feed del portale, tendenze, ricerca globale, directory dei blog, follow di blog e utenti, panoramica del blog, letture, menzioni.

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| GET | `/feed/posts` | pubblica | Feed del portale, dal più recente; per default solo articoli di Spazi `blog` (`kind` = blog\|wiki\|docs, `locale`, `tag`, `category`, `following=true`, `limit` max 50, `offset`) |  |
| GET | `/feed/locales` | pubblica | Le prime 5 lingue per numero di post nel feed |  |
| GET | `/feed/trending` | pubblica | Tag più usati negli ultimi `days` giorni (max 90) |  |
| GET | `/search/posts` | pubblica | Ricerca globale in titolo e contenuto (`q`, `limit` max 50, `offset`) |  |
| GET | `/blogs` | pubblica | Directory pubblica degli Spazi (`kind` = blog\|wiki\|docs, `q`, `locale`, `sort` = active\|new\|followers, `limit` max 100, `offset`) |  |
| POST | `/blogs/{slug}/follow` | utente | Segue un blog (idempotente) | 204 |
| DELETE | `/blogs/{slug}/follow` | utente | Smette di seguire un blog, anche se nel frattempo non è più visibile | 204 |
| GET | `/blogs/{slug}/followers` | pubblica | Username dei follower del blog |  |
| POST | `/users/{username}/follow` | utente | Segue un utente (idempotente) | 204 |
| DELETE | `/users/{username}/follow` | utente | Smette di seguire un utente | 204 |
| GET | `/users/{username}/followers` | pubblica | Chi segue l'utente |  |
| GET | `/users/{username}/following` | pubblica | Chi l'utente segue |  |
| GET | `/users/me/follow-stats` | utente | Quante persone mi seguono in tutto: utenti e follower dei miei blog |  |
| GET | `/blogs/{slug}/overview` | utente | Panoramica (conteggi, letture degli ultimi 30 giorni, spazio): proprietario e membri |  |
| GET | `/blogs/{slug}/mentionable-users` | scrittura sullo Spazio | Suggerimenti per le @menzioni: proprietario, collaboratori e follower (`q`, `limit` max 25) |  |
| POST | `/posts/{post_id}/read` | pubblica | Conta una lettura (aggregata per giorno, nessun dato del lettore) | 204 |

## fragments  (opzionale)

Frammenti: porzioni di testo evidenziate e salvate dai lettori su un post pubblicato, raccolta personale.

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| POST | `/posts/{post_id}/fragments` | utente | Salva un frammento | 201 |
| GET | `/posts/{post_id}/fragments` | utente | Frammenti già salvati dall'utente su questo post |  |
| GET | `/users/me/fragments` | utente | Raccolta dei frammenti dell'utente, i più recenti per primi |  |
| PATCH | `/fragments/{fragment_id}` | utente | Cambia la visibilità di un frammento |  |
| DELETE | `/fragments/{fragment_id}` | utente | Elimina un frammento | 204 |

## gdpr  (opzionale)

Diritti GDPR: esportazione dei propri dati (art. 20), cancellazione dell'account per anonimizzazione (art. 17) e coda delle richieste con scadenza legale.

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| GET | `/users/me/export-data` | utente | Copia dei propri dati in un unico JSON (GDPR art. 20) | rate limit 5/1 h per user |
| DELETE | `/users/me` | utente | Cancella l'account (GDPR art. 17): dati personali eliminati, contenuti condivisi anonimizzati | 204; rate limit 5/10 min per user |
| GET | `/admin/gdpr` | ruolo: super admin, amministratore | Registro delle richieste (`status`): aperte e approvate prima, per scadenza |  |
| POST | `/admin/gdpr` | ruolo: super admin, amministratore | Inserisce una richiesta arrivata fuori banda a nome di un utente | 201 |
| POST | `/admin/gdpr/{request_id}/approve` | ruolo: super admin, amministratore | Approva; una cancellazione la approva un amministratore diverso da chi l'ha inserita |  |
| POST | `/admin/gdpr/{request_id}/reject` | ruolo: super admin, amministratore | Rifiuto motivato |  |
| POST | `/admin/gdpr/{request_id}/execute` | ruolo: super admin, amministratore | Esegue: un export restituisce il JSON dei dati; una cancellazione approvata anonimizza l'account |  |

## git_sync  (opzionale)

Sincronizzazione dei testi di uno Spazio verso un branch di un repository Git remoto (GitHub, GitLab, Forgejo...): solo esportazione, via HTTPS con token.

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| GET | `/blogs/{slug}/git-sync` | utente | Configurazione e ultimo esito della sincronizzazione Git (solo proprietario; il token non viene mai restituito) |  |
| PUT | `/blogs/{slug}/git-sync` | utente | Imposta il repository, il branch e il token (HTTPS; branch diverso da quello predefinito del repository) |  |
| DELETE | `/blogs/{slug}/git-sync` | utente | Rimuove la sincronizzazione e cancella il token salvato (il repository remoto non viene toccato) | 204 |
| POST | `/blogs/{slug}/git-sync/run` | utente | Accoda una sincronizzazione subito; l'esito compare in `last_status` (polling su GET) | 202 |

## interests  (opzionale)

Elenco pubblico degli interessi di piattaforma, scelti dagli utenti nel profilo e usati per filtrare la directory utenti.

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| GET | `/interests` | pubblica | Interessi di piattaforma selezionabili nel profilo |  |

## link_preview  (opzionale)

Anteprima Open Graph dei link incollati nell'editor e delle card nei post (fetch lato server, protetto da SSRF).

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| GET | `/link-preview` | pubblica | Anteprima Open Graph di un URL (`?url=`) | rate limit 30/1 min per ip |

## newsletter  (opzionale)

Newsletter per blog e digest di piattaforma: iscrizione con doppio consenso, disiscrizione e cancellazione senza login, notifica automatica dei nuovi post e campagne manuali.

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| POST | `/newsletter/subscribe` | pubblica | Iscrizione con doppio consenso; risposta sempre uguale (nessuna enumerazione delle email) | 202; rate limit 5/1 h per ip |
| GET | `/newsletter/confirm` | pubblica | Conferma l'iscrizione (`?token=`); idempotente | rate limit 30/5 min per ip |
| POST | `/newsletter/unsubscribe` | pubblica | Disiscrizione con il link firmato ricevuto per email, senza login | rate limit 30/5 min per ip |
| POST | `/newsletter/unsubscribe/delete` | pubblica | Cancellazione dei propri dati dalla lista (GDPR art. 17), senza login | rate limit 30/5 min per ip |
| GET | `/blogs/{slug}/newsletter/stats` | scrittura sullo Spazio | Iscritti per stato |  |
| GET | `/blogs/{slug}/newsletter/campaigns` | scrittura sullo Spazio | Campagne del blog, dalla più recente |  |
| POST | `/blogs/{slug}/newsletter/campaigns` | scrittura sullo Spazio | Crea una campagna manuale (immediata o pianificata) | 201; rate limit 20/1 h per user |
| PATCH | `/blogs/{slug}/newsletter/campaigns/{campaign_id}` | scrittura sullo Spazio | Modifica una campagna ancora pianificata |  |
| DELETE | `/blogs/{slug}/newsletter/campaigns/{campaign_id}` | scrittura sullo Spazio | Annulla una campagna ancora pianificata |  |
| GET | `/blogs/{slug}/newsletter/settings` | scrittura sullo Spazio | Invio automatico, nome del mittente e banner delle email |  |
| PATCH | `/blogs/{slug}/newsletter/settings` | scrittura sullo Spazio | Aggiorna le impostazioni (campo assente = invariato, `null` = azzera) |  |
| GET | `/admin/newsletter/stats` | ruolo: super admin, amministratore | Iscritti al digest per stato |  |
| GET | `/admin/newsletter/settings` | ruolo: super admin, amministratore | Nome del mittente e banner del digest |  |
| PATCH | `/admin/newsletter/settings` | ruolo: super admin, amministratore | Aggiorna le impostazioni del digest |  |
| GET | `/admin/newsletter/campaigns` | ruolo: super admin, amministratore | Campagne del digest |  |
| POST | `/admin/newsletter/campaigns` | ruolo: super admin, amministratore | Crea una campagna del digest | 201 |
| PATCH | `/admin/newsletter/campaigns/{campaign_id}` | ruolo: super admin, amministratore | Modifica una campagna del digest ancora pianificata |  |
| DELETE | `/admin/newsletter/campaigns/{campaign_id}` | ruolo: super admin, amministratore | Annulla una campagna del digest ancora pianificata |  |

## notes  (opzionale)

Note a piè di pagina dei post, libreria note del blog (duplicati, merge, BibTeX) e bibliografie (campo notes di post).

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| GET | `/blogs/{slug}/notes` | utente | Libreria note del blog con «citata in» e duplicati (`?q=` filtra sul testo); proprietario e collaboratori |  |
| POST | `/blogs/{slug}/notes` | scrittura sullo Spazio | Crea una nota di libreria | 201 |
| GET | `/blogs/{slug}/notes/export.bib` | utente | Esporta la libreria in BibTeX | raw |
| POST | `/blogs/{slug}/notes/import` | scrittura sullo Spazio | Importa BibTeX (le voci già presenti si saltano) | 201 |
| PATCH | `/blogs/{slug}/notes/{note_id}` | scrittura sullo Spazio | Modifica una nota; il testo si aggiorna anche nei post che la citano |  |
| DELETE | `/blogs/{slug}/notes/{note_id}` | scrittura sullo Spazio | Elimina una nota non citata da alcun post (409 altrimenti) | 204 |
| POST | `/blogs/{slug}/notes/{note_id}/merge` | scrittura sullo Spazio | Unisce la nota in un'altra: le citazioni passano alla destinazione |  |
| GET | `/blogs/{slug}/bibliography` | pubblica | Bibliografia dei post pubblicati, raggruppata per testo identico |  |
| GET | `/users/me/bibliography` | utente | Note di tutti i post di tutti i miei blog (qualunque stato) |  |

## pages  (opzionale)

Pagine statiche: del sito (solo amministratori) e dei blog che le attivano (campo static_pages_enabled del blog).

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| GET | `/blogs/{slug}/pages` | pubblica | Pagine del blog in una lingua |  |
| GET | `/blogs/{slug}/pages/by-id/{page_id}` | scrittura sullo Spazio | Pagina per id (editor, bozze incluse) |  |
| GET | `/blogs/{slug}/pages/{key}` | pubblica | Pagina per slug e lingua |  |
| POST | `/blogs/{slug}/pages` | scrittura sullo Spazio | Crea una pagina | 201 |
| POST | `/blogs/{slug}/pages/{key}/translations` | scrittura sullo Spazio | Aggiunge una traduzione | 201 |
| GET | `/blogs/{slug}/pages/{key}/translations` | pubblica | Traduzioni pubblicate di una pagina |  |
| PATCH | `/blogs/{slug}/pages/{key}` | scrittura sullo Spazio | Modifica una pagina |  |
| DELETE | `/blogs/{slug}/pages/{key}` | scrittura sullo Spazio | Elimina una pagina | 204 |
| GET | `/pages` | pubblica | Pagine del sito in una lingua (`?locale=`, `?q=` cerca in titolo e slug) |  |
| GET | `/pages/{key}` | pubblica | Pagina del sito per slug e lingua |  |
| POST | `/pages` | ruolo: super admin, amministratore | Crea una pagina del sito | 201 |
| POST | `/pages/{key}/translations` | ruolo: super admin, amministratore | Aggiunge una traduzione | 201 |
| GET | `/pages/{key}/translations` | pubblica | Traduzioni pubblicate di una pagina del sito |  |
| PATCH | `/pages/{key}` | ruolo: super admin, amministratore | Modifica una pagina del sito |  |

## publications  (opzionale)

Pubblicazioni: raccolte di post come capitoli, indice pubblico e ordine esplicito (campi publication_id/publication/chapter_order di post).

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| GET | `/blogs/{slug}/publications` | pubblica | Pubblicazioni del blog con i conteggi dei capitoli |  |
| POST | `/blogs/{slug}/publications` | scrittura sullo Spazio | Crea una pubblicazione | 201 |
| GET | `/blogs/{slug}/publications/{ref}` | pubblica | Indice dei capitoli di una pubblicazione |  |
| PATCH | `/blogs/{slug}/publications/{ref}` | scrittura sullo Spazio | Modifica nome, titolo o descrizione |  |
| PUT | `/blogs/{slug}/publications/{ref}/order` | scrittura sullo Spazio | Imposta l'ordine dei capitoli |  |
| DELETE | `/blogs/{slug}/publications/{ref}` | scrittura sullo Spazio | Elimina la pubblicazione (i post restano, senza pubblicazione) | 204 |
| GET | `/users/me/publications` | utente | Pubblicazioni di tutti i miei blog (proprietario o collaboratore), con i conteggi dei capitoli |  |

## reports  (opzionale)

Segnalazioni di blog e post da parte dei lettori registrati, una per bersaglio, da chiudere dalle rotte di amministrazione.

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| POST | `/blogs/{slug}/report` | utente | Segnala un blog (una sola volta; non il proprio) | 201; rate limit 10/1 h per user |
| POST | `/posts/{post_id}/report` | utente | Segnala un post pubblicato (una sola volta; non un contenuto proprio) | 201; rate limit 10/1 h per user |
| GET | `/admin/blogs/{blog_id}/reports` | ruolo: super admin, amministratore | Segnalazioni aperte su un blog e sui suoi post, con motivo, nota e chi ha segnalato |  |
| POST | `/admin/blogs/{blog_id}/action` | ruolo: super admin, amministratore | Azione dal pannello segnalazioni (nota obbligatoria): chiude le segnalazioni aperte del blog |  |

## taxonomy  (opzionale)

Categorie per-blog dei post (campi category_id/category in create/update/risposta di post).

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| GET | `/blogs/{slug}/categories` | pubblica | Categorie del blog |  |
| POST | `/blogs/{slug}/categories` | scrittura sullo Spazio | Crea una categoria |  |
| PATCH | `/blogs/{slug}/categories/{category_id}` | scrittura sullo Spazio | Modifica nome o slug di una categoria |  |
| DELETE | `/blogs/{slug}/categories/{category_id}` | scrittura sullo Spazio | Elimina una categoria |  |

## wiki  (opzionale)

Spazi wiki e documentazione: albero delle pagine, link interni [[pagina]] con backlink, cronologia delle revisioni con diff.

| Verbo | Percorso | Accesso | Descrizione | Note |
|---|---|---|---|---|
| GET | `/blogs/{slug}/outline` | pubblica | Indice dello Spazio (`?locale=`): i lettori vedono i post pubblicati, chi scrive tutti |  |
| PUT | `/blogs/{slug}/outline` | scrittura sullo Spazio | Riordina e sposta le pagine (solo Spazi wiki/docs); 409 se l'elenco non coincide con i post dello Spazio |  |
| GET | `/posts/{post_id}/revisions` | utente | Cronologia delle revisioni, dalla più recente (chi può scrivere nello Spazio) |  |
| GET | `/posts/{post_id}/revisions/{revision}` | utente | Una revisione con il contenuto completo |  |
| GET | `/posts/{post_id}/revisions/{revision}/diff` | utente | Differenze per righe tra la revisione e `?against=` (default: la precedente) |  |
| GET | `/posts/{post_id}/backlinks` | pubblica | Pagine dello stesso Spazio che rimandano a questa con [[link]] (solo quelle visibili al chiamante) |  |

## Come si mantiene questo file

Le tabelle sono generate dai file `api.toml` (campo `summary` di ogni rotta). Aggiungendo o modificando una rotta, scrivere un
`summary` che spieghi **perché la rotta esiste**, non solo cosa fa, e rigenerare questo documento. Le rotte senza `summary`
compaiono con il solo identificativo.

Il modulo di prova `sandbox` (usato nei test) non è incluso.
