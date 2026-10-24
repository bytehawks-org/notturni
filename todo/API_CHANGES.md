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

## Suggerite (non applicate)

* **N+1 sul feed e sulle liste**: la v1 serializzava ogni post con più query; in Rust ogni elenco usa una query per modulo. Nessun cambio di contratto, ma si potrebbe aggiungere `limit/offset` a `GET /blogs/{slug}/posts` (oggi restituisce tutti i post).
* **Paginazione** di `GET /blogs/{slug}/media` e delle bibliografie.
* **`GET /blogs/{slug}/members`** potrebbe restituire anche il proprietario con ruolo `autore`, per semplificare il pannello collaboratori.
* **`DELETE /blogs/{slug}`** con corpo: alcuni proxy scartano il corpo dei `DELETE`; si potrebbe accettare `?confirm_slug=`.
