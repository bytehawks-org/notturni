# Modifiche all'API suggerite durante la riscrittura in Rust

Il contratto `/api/v1` della v1 ([legacy/backend/API.md](../legacy/backend/API.md)) è mantenuto. Qui si elencano le
differenze **introdotte** (da verificare con il frontend) e quelle **suggerite** (da approvare caso per caso).
Nessuna è una rottura silenziosa.

## Introdotte (compatibili con il frontend attuale)

| Dove | Cambio | Perché |
|---|---|---|
| Validazione | Gli errori di validazione dei corpi sono `400` con `{"detail": "…"}` dove FastAPI rispondeva `422` con un elenco. Restano `422` il JSON malformato e gli identificatori non UUID | un solo formato d'errore leggibile; il frontend mostra `detail` |
| `POST /blogs`, `PATCH /blogs/{slug}` | `title` non vuoto e ≤ 255 caratteri; `default_author_display_name` ≤ 255 | in v1 un titolo lungo dava `500` |
| `POST|PATCH /posts` | `slug` solo `[A-Za-z0-9._~-]` (max 255), titolo non vuoto ≤ 255, `cover_image_alt_text` ≤ 300, `content` ≤ 500 000 caratteri, categorie di avviso validate | in v1 qualsiasi stringa; un `/` o uno spazio nello slug rendeva il permalink irraggiungibile. Il frontend produce già slug `[a-z0-9-]` |
| Upload immagini | Il contenuto deve corrispondere al tipo dichiarato (PNG/JPEG/WEBP/GIF) | un HTML/SVG con `Content-Type: image/png` era accettato e servito dallo storage pubblico |
| Elenchi | `GET /blogs/mine`, `/blogs/{slug}/posts` ordinati per `created_at`; `/blogs/member-of` per titolo | v1 non ordinava (ordine di heap di Postgres) |
| `GET /posts/{id}/translations` | `404` se il blog non è visibile al chiamante | v1 elencava slug e id anche di blog privati |
| `POST /blogs` | Registra l'evento di audit `blog.created` | assente in v1 |
| Ricerca nel blog | `%` e `_` nel testo cercato sono letterali | in v1 erano caratteri jolly di `ILIKE` |
| Creazione blog | Il limite per utente è `platform_config.max_blogs_per_user` anche nel trasferimento (v1 usava la costante) | coerenza con l'impostazione del Super Admin |
| Pubblicazione | Il post pubblica `posts.post.published` nella stessa transazione (outbox) | in v1 il messaggio partiva dopo il commit e un broker spento lo perdeva |
| Campi di moduli spenti | `comments_mode`, `category`, `notes`, `publication`… compaiono solo se il modulo è attivo | modularità; i moduli di default li attivano tutti |
| `PATCH /posts/{id}` | `category_id` e `publication_id` non UUID → `400` (FastAPI: `422`); categoria o pubblicazione di un altro blog → `400` come in v1 | un solo formato d'errore per i campi dei moduli |
| Pagine | `slug` validato anche per le pagine di sito (v1 solo per quelle di blog); `locale` e `title` ≤ 255 con `400` invece di `500` | uno slug con spazi o `/` rendeva la pagina irraggiungibile |
| `GET /pages/{slug}`, `GET /blogs/{slug}/pages` | `?locale=` mancante → `422` | invariato rispetto a v1, ma esplicito |
| `GET /blogs/{slug}/pages/{id}/translations` | segue la visibilità del blog (`404` se non visibile) | in v1 non controllava la visibilità e rivelava slug di pagine di blog privati |
| `GET /blogs/{slug}/publications/{ref}` | capitoli ordinati con `chapter_order` poi cronologico, a parità per `id` | v1 ordinava in Python senza criterio finale |
| `GET /link-preview` | `?url=` mancante → `422`; il blocco SSRF copre anche CGNAT, IPv4-mapped e NAT64, e nessun proxy di sistema | difesa più completa; stessa risposta di v1 per gli URL validi |
| `/posts/{id}/comments` (lettura e scrittura), `/posts/{id}/fragments` | solo su post **pubblicati** di un blog visibile al chiamante; altrimenti `404` | in v1 si poteva commentare e leggere i commenti anche di bozze e di blog privati se si conosceva l'id |
| `GET /posts/{id}/comments` e viste pubbliche | `reported_to_platform` e `report_note` sono `false`/`null` per chi non modera | in v1 la nota di segnalazione era visibile a tutti nell'elenco pubblico |
| `POST /posts/{id}/comments` | limite di 30 richieste al minuto per IP; email validata (`400`) | in v1 nessun limite oltre al captcha |
| `PATCH /blogs/{slug}`, `PATCH /posts/{id}` | `comments_mode` non valido o `comments_auto_close_days` non intero → `400` | un solo formato d'errore |
| Note | un `kind`/`url` non valido nelle note del post → `400` con il messaggio del campo, come in v1; `notes: null` equivale ad assente | coerenza con gli altri tri-stati |
| `GET /blogs/{slug}/notes` | `403` a chi non è proprietario né collaboratore; la ricerca `q` ignora le maiuscole sul testo intero | invariato rispetto a v1, ma esplicito |
| `GET /feed/posts?following=true` | i post di un utente seguito compaiono solo se firmati con il suo username | in v1 comparivano anche quelli con alias, rivelando a chi segue la persona di chi fossero |
| `POST /blogs/{slug}/follow`, `GET /blogs/{slug}/followers` | `404` se il blog non è visibile al chiamante; `DELETE /blogs/{slug}/follow` funziona comunque | in v1 si poteva seguire (e vedere i follower di) un blog privato conoscendone lo slug |
| `GET /search/posts` | `q` mancante → `422`; `%` e `_` nel testo cercato sono letterali | in v1 erano jolly di `ILIKE` |
| `GET /blogs/{slug}/overview` | `storage_bytes` è la somma di `media_files.size_bytes` (la stessa della quota); i conteggi dei commenti sono 0 se il modulo `comments` è spento | in v1 si contavano gli oggetti su storage, backup compresi |
| Ordinamenti | feed, ricerca e directory hanno un ordine di spareggio per `id` | risultati stabili tra una pagina e l'altra |
| `/users/me/{posts,media,links,publications}` | escludono i blog in cancellazione, come in v1; ora vivono nei moduli proprietari e spariscono con essi | modularità |
| `GET /users/{username}`, `/users/{username}/{blogs,posts,comments}` | `404` per un account disattivato (sospeso o cancellato) | in v1 il profilo di un account anonimizzato restava raggiungibile |
| `GET /users` | `q` ignora `%` e `_` come jolly (sono letterali); ordinamento di spareggio per `id`; senza il modulo `discovery` `follower_count` è 0 | coerenza con le altre ricerche |
| `PATCH /users/me` | `bio`, nome, cognome e nome pubblico vuoti o di soli spazi diventano `null` (v1 salvava `""` per `bio`); `post_author_name_style` non valido → `422`; limiti: `bio` ≤ 5000, nome/cognome ≤ 100, nome pubblico ≤ 255 | v1 non limitava la bio e restituiva `500` oltre la lunghezza delle colonne |
| `PATCH /users/me` | `fallback_languages` senza duplicati (ordine preservato); `interests` senza duplicati e vuoti; senza il modulo `interests` si possono solo svuotare | coerenza con il resto delle liste |
| `POST /users/me/social-links` | l'URL deve essere `http://` o `https://` (`400`); etichetta e URL ≤ 100 / 2048 caratteri; il tetto di 5 è serializzato sulla riga utente | in v1 `javascript:` e `data:` finivano in un `href` del profilo pubblico; due richieste concorrenti superavano il tetto |
| `POST /users/me/email/*` | i codici sono derivati e non salvati (come l'OTP di reset); limite di 5 richieste e 10 verifiche ogni 10 minuti per utente | v1 salvava l'hash del codice e non limitava i tentativi |
| `POST /users/me/avatar` | tipo reale dell'immagine verificato (PNG/JPEG/WEBP, max 2 MiB) | in v1 si fidava del `Content-Type` dichiarato |
| `POST /newsletter/subscribe` | `blog_slug` non visibile al chiamante → `404`; `email` e `locale` non validi → `400` (in v1 `422`); l'email è salvata in minuscolo | coerenza con il resto dell'API |
| `GET /newsletter/confirm` | conferma solo iscrizioni **in attesa**: un vecchio link di un iscritto già disiscritto non lo riporta in lista (`invalid`); `?token=` mancante → `422`; il token non è più un valore casuale salvato ma firmato, e una nuova richiesta di iscrizione invalida il link precedente | in v1 un vecchio link riattivava un'iscrizione disdetta |
| `POST /newsletter/unsubscribe*` | i link già spediti dalla v1 restano validi (stessa firma HMAC); `reason` ≤ 500 caratteri (`400`) | continuità dei link nelle email |
| Campagne pianificate | partono davvero alla scadenza (`newsletter.campaigns.dispatch`, accodato dal pianificatore della F7) | in v1 restavano `scheduled` per sempre |
| Campagne | blog sospeso, in pausa o cancellato, o post non più pubblicato → `canceled` invece di spedire; SMTP assente → `failed`; un post pianificato nel futuro dà una campagna pianificata alla stessa ora | in v1 si spediva comunque e senza SMTP la campagna risultava inviata a 0 destinatari |
| `PATCH /blogs/{slug}/newsletter/settings` | `newsletter_banner_url` solo http(s) (`400`) | il valore finisce in un `<img src>` |
| `POST /blogs/{slug}/report`, `/posts/{id}/report` | `reason` non valido → `400`; `note` più lunga di 500 caratteri è tagliata (come in v1) | un solo formato d'errore |
| `POST /users/me/domain` | dominio con etichette oltre 63 caratteri o con schema/percorso → `400`; la verifica DNS ha timeout configurabile | v1 accettava solo il formato dell'espressione regolare |
| `PATCH /users/me` | `post_author_name_style = verified_domain` (valore già previsto in v1): se il dominio non c'è più si firma con lo username | coerenza |
| `DELETE /users/me`, `/admin/gdpr/{id}/execute` | un Super Admin non si cancella, né da solo né con una richiesta | senza, la piattaforma non si amministra più (in v1 solo il percorso admin lo impediva) |
| `GET /users/me/export-data` | le sezioni dei moduli spenti non compaiono; in più `memberships`, `sso_identities`, `reports_made` e `user.data_exported` nell'audit; mai token, hash o segreti | portabilità completa e verificabile |
| `DELETE /users/me` | anche le etichette nelle liste dei bloccati dei blog altrui e gli inviti pendenti ricevuti sono ripuliti; limite di 5 richieste ogni 10 minuti | in v1 restavano il nome nell'etichetta e gli inviti |
| `GET /admin/gdpr` | `status` non valido → `422`; ordine: aperte, poi approvate, poi per scadenza | stabile |
| `POST /auth/mfa/totp/setup` | il nuovo segreto è «in attesa» fino a `confirm`; non sostituisce quello attivo | in v1 rigenerarlo con l'MFA già attiva la rompeva finché non si confermava |
| `POST /auth/mfa/verify`, `confirm` | lo stesso codice TOTP non si può usare due volte; il metodo della sfida deve essere quello attuale | replay di un codice intercettato |
| `POST /auth/mfa/{email/setup,disable}` | `auth.mfa_enabled` e `auth.mfa_disabled` finiscono nell'audit; limite di richieste per utente | assenti in v1 |
| `GET /auth/sso/{provider}/callback` | collega un account esistente solo se il provider garantisce l'email; per GitHub solo l'email primaria verificata; un nuovo account solo a registrazioni aperte; errori sempre come `/login?sso_error=1` | in v1 un provider che non verificava l'email permetteva di prendere un account altrui, e l'SSO aggirava le modalità di registrazione |
| `GET /auth/sso/{provider}/login` | state e PKCE legati al browser da un cookie `noct_sso_state` (httpOnly, `SameSite=Lax`, path `/api/v1/auth/sso`) | in v1 era la sessione firmata di Starlette |

## Suggerite (non applicate)

* **N+1 sul feed e sulle liste**: la v1 serializzava ogni post con più query; in Rust ogni elenco usa una query per modulo. Nessun cambio di contratto, ma si potrebbe aggiungere `limit/offset` a `GET /blogs/{slug}/posts` (oggi restituisce tutti i post).
* **Paginazione** di `GET /blogs/{slug}/media` e delle bibliografie.
* **`GET /blogs/{slug}/members`** potrebbe restituire anche il proprietario con ruolo `autore`, per semplificare il pannello collaboratori.
* **`DELETE /blogs/{slug}`** con corpo: alcuni proxy scartano il corpo dei `DELETE`; si potrebbe accettare `?confirm_slug=`.
* **`POST /auth/mfa/disable`** dovrebbe chiedere un codice valido (o la password) invece di spegnere l'MFA a chiunque abbia un access token: un token rubato oggi basta a togliere il secondo fattore. Richiede un campo nel corpo, quindi una modifica del frontend.
