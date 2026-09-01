# Specifiche e stato di avanzamento

Questo documento è il riferimento pratico per capire **cosa prevede il progetto**,
**cosa è già stato implementato** e **cosa resta da fare**. Fa anche da workplan:
le righe segnate 🟡 e ⚪ sono, di fatto, i task aperti.

Il documento "sorgente" delle intenzioni di prodotto (visione, tono, esempi di
paragone, vincoli architetturali di partenza) resta ad uso interno del team/degli
agenti che generano il codice e non è linkato dalla documentazione pubblica:
questo file ne è la trascrizione verificabile, specifica per specifica, contro
lo stato reale del codice.

Legenda stato:

- ✅ **Implementato** — presente nel codice e coperto da test e/o verifica manuale.
- 🟡 **Parziale** — presente ma con limitazioni note, o infrastruttura pronta e non
  ancora collegata a logica applicativa.
- ⚪ **Da fare** — non ancora iniziato.

## 1. Prodotto e regole di dominio

| Specifica | Stato | Note / task residuo |
|---|---|---|
| Multiutente & multilingua, enfasi GDPR/EU | 🟡 | Multilingua implementato (vedi sotto). Nessuna funzionalità GDPR dedicata ancora (export/cancellazione dati account, registro consensi, informativa privacy strutturata oltre alla pagina statica libera). |
| Massimo 5 blog per utente | ✅ | `MAX_BLOGS_PER_USER` in `backend/app/domain/blog_rules.py`. |
| Nome autore in pubblico diverso dal nome utente reale | ✅ | Calcolato alla scrittura del post (`backend/app/api/v1/posts.py::_resolve_author_display_name`), **senza override per singolo post** (todo/USERS.md #2): alias per-blog del collaboratore (`blog_memberships.author_display_name`) → nome pubblico predefinito del blog (`Blog.default_author_display_name`) — se uno dei due esiste è imposto — → altrimenti la preferenza di profilo `User.post_author_name_style` (`username` di default \| `full_name` \| `display_name`). `User.display_name` è anche l'intestazione del profilo pubblico quando valorizzato. |
| Username univoco e citabile come `@username` | ✅ | Formato validato in `backend/app/domain/usernames.py` (minuscole/cifre, `-`/`_` interni, 3–32 caratteri) più blacklist. Le `@username` nel contenuto dei post vengono trasformate in link al profilo dal rendering frontend (`frontend/src/lib/markdown.ts`), con autocomplete nell'editor (`GET /api/v1/blogs/{slug}/mentionable-users`: proprietario, collaboratori, follower del blog). Disattivabile per blog con `Blog.mentions_enabled` (attivo di default). Riferimenti `[[articolo]]`/`[[blog:articolo]]` e menu `/` restano da fare — vedi `todo/EDITOR.md`. |
| Sottotitolo (max 64) e descrizione breve (max 256) del blog | ✅ | `Blog.subtitle` / `Blog.description`, validati in `backend/app/domain/blog_rules.py`; modificabili da `PATCH /api/v1/blogs/{slug}` e in creazione. |
| Visibilità del blog: pubblica / solo iscritti / privata (diario) | ✅ | `Blog.visibility` (`BlogVisibility`). `members` limita le pagine pubbliche del blog a chi è autenticato; `private` le limita al proprietario e ai collaboratori, e la scrittura al **solo proprietario** (`backend/app/domain/authorization.py`). Il feed della homepage mostra solo blog `public`. In dashboard l'elenco blog mostra la visibilità con una banda colorata sul lato destro della card (verde / arancione / bande diagonali rosso-nere). |
| Inviti a collaborare (co-autore / mediatore) con accettazione | ✅ | Tabella `blog_invitations`. Il proprietario crea un invito (`POST /api/v1/blogs/{slug}/invitations`), l'invitato lo accetta/rifiuta dalla propria dashboard (`/api/v1/blogs/received-invitations/...`); solo all'accettazione nasce la `BlogMembership`. Gestione collaboratori (ruolo, rimozione) e revoca inviti lato proprietario. |
| 4 ruoli piattaforma: Super Admin, Amministratore, Moderatore, Utente (default Utente) | 🟡 | Enum `PlatformRole` completo (`backend/app/models/user.py`). Super Admin/Amministratore gestiscono utenti e pagine statiche. **Il ruolo piattaforma Moderatore è definito ma non ancora collegato a nessuna capacità reale** (nessun endpoint/permesso lo usa). |
| Username e password di Super Admin specificati alla creazione delle risorse (Podman/K8s) | ⚪ | Non implementato: oggi il Super Admin nasce o per auto-promozione del primo utente (`NOCT_DEPLOYMENT_MODE=solo`) o per `UPDATE` manuale a DB in modalità `platform`. Manca un vero bootstrap via env/secret a livello di deploy. |
| 4 ruoli blog: Autore (default), Co-Autore, Revisore, Mediatore | ✅ | Enum `BlogRole` (`backend/app/models/blog.py`), tutti e quattro collegati a capacità reali (`backend/app/domain/authorization.py`): scrittura, revisione (`pending_review`→`published`), moderazione commenti. Co-Autore e Mediatore sono assegnabili dal proprietario tramite il flusso di invito (vedi riga "Inviti a collaborare"); Autore/Revisore restano assegnabili solo via DB. |
| Dashboard utente user-friendly, moderna, responsiva | 🟡 | Prima versione costruita (Next.js + Tailwind, dashboard blog/post/profilo). Da raffinare con uso reale; nessun audit di accessibilità/responsività fatto oltre al testing manuale. |
| Dashboard di amministrazione con tutte le funzionalità di moderazione | 🟡 | Gestione utenti e pagine statiche implementate. **Manca un pannello di moderazione trasversale** (commenti su tutti i blog, segnalazioni, sospensione contenuti) — oggi la moderazione commenti è solo per-blog, ad opera del Mediatore di quel blog. |
| Editor pulito, focalizzato sui contenuti (rif. fika.bar) | ✅ | Editor WYSIWYG (Tiptap) con salvataggio in Markdown: titoli (H1-H3), grassetto/corsivo/barrato/codice/link, citazioni, elenchi, tabelle, immagini (con testo ALT), traduzioni (selettore di lingua dalle lingue di fallback del profilo, invece di scrivere la sigla), immagine di copertina, autocomplete delle `@menzioni`, note a piè di pagina. Riferimenti `[[...]]` e menu `/` sono nel backlog (`todo/EDITOR.md`). |
| Note a piè di pagina + bibliografia automatica del blog | ✅ | Elenco strutturato per post (`post_notes`, riscritto ad ogni salvataggio come i tag), non nel corpo. Nel `content` il riferimento è il marcatore link `[n](#nota-n)` inserito dal pulsante «Nota» dell'editor. La pagina pubblica del post rende l'elenco numerato in fondo + il testo come tooltip (`frontend/src/lib/markdown.ts`); `GET /api/v1/blogs/{slug}/bibliography` e la pagina `/{blog}/bibliografia` raccolgono tutte le note dei post pubblicati, deduplicate per testo, con i post che le citano. |
| Personalizzazione colori/tipografia/presentazione per blog | ✅ | Tabella `blog_configs`, JSON libero con i vincoli della sezione [2](#2-estetica). |
| Sistema di commenti con moderazione dell'autore | ✅ | Aperti di default solo a utenti registrati; il proprietario del blog può aprirli anche a non registrati con moderazione pre-pubblicazione obbligatoria. |
| Multilingua per post e pagine statiche | ✅ | Schema "famiglia di traduzioni" (`translation_group_id` + `locale` + slug per lingua). Il routing `/it/`, `/en/`, ... è lasciato al frontend: **il frontend pubblico multilingua/i18n-routing non è ancora costruito**, oggi si seleziona la lingua via parametro. |
| Pagine statiche del sito principale (Chi siamo, Contatti, Privacy) | ✅ | CRUD gestito da Amministratore/Super Admin. |
| Follow tra utenti e verso i blog | ✅ | |
| Profilo utente: bio, avatar, link social | ✅ | Avatar su MinIO/S3. Bio estesa con nome, cognome, paese, lingua madre e lingue di fallback (queste ultime anche come lingue verso cui l'utente potrà eventualmente tradurre i propri contenuti — stile fika.bar). Link social: elenco piattaforme con icona monocromatica in un file di configurazione lato frontend (`frontend/src/lib/social-platforms.tsx`), facilmente editabile; il backend resta agnostico (stringa libera). |
| Nome pubblico predefinito per gli autori/co-autori di un blog | ✅ | `Blog.default_author_display_name`: se valorizzato è **imposto** come nome autore sui post di quel blog (todo/USERS.md #2), a meno che il singolo collaboratore non abbia un proprio alias di membership che ha la precedenza. Nessun override per singolo post. |
| Gestione contenuti in Markdown, nessun rendering lato backend | ✅ | Rendering a HTML e sanificazione lato frontend (Server Component + `markdown-it`/DOMPurify) sulla pagina pubblica del post — vedi permalink più sotto. |
| UUID come chiave primaria per ogni entità | ✅ | |
| Media su S3 in `.../userdata/{user_uuid}/{blog_uuid}/media/...` | ✅ | Bucket con policy pubblica scoped al solo prefisso `media/*`; stesso endpoint di upload usato sia per le immagini incorporate nel contenuto sia per la cover del post (`Post.cover_image_url`). |
| Moderazione automatica delle immagini (nudità/contenuti sensibili) | ✅ | Servizio self-hosted separato (`moderation/`, modello Falconsai/nsfw_image_detection) — nessuna immagine lascia l'infrastruttura. Fail open: un problema del servizio non blocca mai l'upload. Immagini segnalate: sfocate e cliccabili per rivelarle, sia nel contenuto sia in copertina. Soglia di confidenza non ancora tarata su un campione reale (0.7 di default, scelta prudente). |
| Backup/fallback Markdown dei post su S3 ad ogni salvataggio | ✅ | Privato, via coda RabbitMQ + worker dedicato (`worker-post-backup`). |
| Stato post: draft → pending_review → published, con `published_at` per pianificazione | ✅ | Ruolo Revisore collegato alla transizione `pending_review`→`published`. |
| Hostname/FQDN e modalità di installazione via env | ✅ | `NOCT_INSTANCE_FQDN`, `NOCT_DEPLOYMENT_MODE=solo\|platform`. |

## 2. Estetica

| Specifica | Stato | Note / task residuo |
|---|---|---|
| Palette di massimo 5 colori | ✅ | Validato server-side in `backend/app/domain/blog_config.py`. |
| Massimo 3 font | ✅ | Idem. |
| Titoli in serif, testo/link in sans-serif | 🟡 | Applicato nel tema di default del frontend; non ancora imposto come vincolo quando l'utente personalizza la tipografia del proprio blog. |
| Toni calmi, non aggressivi | 🟡 | Scelta soggettiva applicata al tema di default; nessun vincolo automatico sulla palette scelta dall'utente oltre al numero massimo di colori. |

## 3. Architettura, stack e infrastruttura

| Specifica | Stato | Note / task residuo |
|---|---|---|
| Stack di base: Next.js, FastAPI, PostgreSQL 16+, Redis, RabbitMQ, MinIO | ✅ | Tutti i servizi presenti in `compose.yaml` e nei manifest `k8s/`. |
| Redis per rate limiting e lock distribuiti | 🟡 | Servizio deployato ma **non ancora usato da nessuna logica applicativa** — nessun rate limiting né lock distribuito implementato oggi. |
| Dockerfile/manifest agnostici rispetto all'architettura (x86_64/ARM64) | ✅ | Immagini multi-arch, nessuna dipendenza hard-coded da un'architettura. |
| Compatibilità S3 con endpoint custom iniettato (MinIO/AWS/Cloudflare) | ✅ | `backend/app/core/storage.py`, sempre via `ENDPOINT_URL` configurabile. |
| Prefisso env `NOCT_`/`noct_` | ✅ | |
| Clusterizzazione dei componenti in produzione | ⚪ | Manifest K8s attuali sono un primo draft a singolo nodo; nessun lavoro di clustering/HA fatto. |
| Traefik come ingress | 🟡 | `k8s/ingress.yaml` usa `ingressClassName: traefik`, ma è **routing path-based su un solo host** — manca l'`IngressRoute` dinamico per sottodominio-per-utente. |
| Longhorn come storage class | ✅ | Usato in `k8s/postgres.yaml` e `k8s/minio.yaml` (draft). |
| Backup su S3 esterno di database e MinIO | ⚪ | Non implementato: esiste solo il backup applicativo dei singoli post (`worker-post-backup`), non un backup infrastrutturale di Postgres/MinIO nel loro complesso. |
| Blog utente su `https://nomeutente.notturni.eu` | ⚪ | Non implementato: routing per sottodominio ancora assente (backend, frontend, manifest K8s). Un post è però già raggiungibile **senza sottodominio**, via `https://notturni.eu/{blog_slug}/{post_slug}` — vedi riga dedicata sotto. |
| Permalink leggibili per i post, senza UUID nell'URL | ✅ | `/{blog_slug}/{YYYYMMDD}/{post_slug}` (stile WordPress; la data è solo leggibilità/disambiguazione, l'unicità reale resta su blog+slug+locale) — `backend/app/domain/permalinks.py`, endpoint pubblico `GET /blogs/{slug}/posts/{date}/{slug}`. Pagina pubblica del post lato frontend (Server Component, Markdown renderizzato a HTML e sanificato — vedi `frontend/src/lib/markdown.ts`) su `/{blog_slug}/{YYYYMMDD}/{post_slug}`, raggiungibile senza passare dal sottodominio del blog. |
| `notturni.eu`: raccolta articoli nella lingua dell'utente (stile dev.to) | ✅ | Homepage con feed cronologico di tutti i blog (`GET /api/v1/feed/posts`, filtro `locale`/`tag` opzionale) e sezione "di tendenza" per tag più usati negli ultimi 7 giorni (`GET /api/v1/feed/trending`) — vedi riga "Tag sui post" sotto. |
| Tag sui post (massimo 5, da campo dedicato e/o `#hashtag` nel testo) | ✅ | `app/domain/tags.py` + tabelle `tags`/`post_tags`; oltre il limite: errore esplicito, mai troncamento silenzioso. Usati per il filtro `?tag=` del feed e per la sezione "di tendenza" della homepage. |
| Categorie sui post, oltre ai tag | ✅ | Tassonomia per-blog (`categories`, `posts.category_id`), a differenza dei tag definita in anticipo da proprietario/autori e al più una per post. CRUD in `POST/blogs/{slug}/categories`; filtro `?category=` anche sul feed della homepage. |
| Pubblicazioni: serie di articoli ordinati cronologicamente (stile libro/saggio) | ⚪ | Non ancora iniziato. Richiesto oltre le specifiche originali del blueprint, non presente in CLAUDE.md: concetto volutamente diverso dalle "Publication" di Medium. |
| `blog.notturni.eu`: blog ufficiale della piattaforma | ⚪ | Non implementato (nessuna distinzione oggi tra blog "di piattaforma" e blog utente). |
| Blocco nome blog: minimo 4 caratteri alfanumerici (`-` ammesso) | ✅ | `backend/app/domain/blog_rules.py`. |
| Nomi di blog di 3 caratteri o meno riservati alla piattaforma | ✅ | Idem — bloccati oggi, nessuna funzionalità premium di sblocco ancora costruita. |
| Blacklist nomi riservati (blog, www, mail, admin, ...) | ✅ | `backend/app/domain/blog_rules.py`, più blacklist separata per gli username (`backend/app/domain/usernames.py`). |
| cert-manager / Let's Encrypt in produzione | 🟡 | Annotazione `cert-manager.io/cluster-issuer` presente in `k8s/ingress.yaml`, presuppone un `ClusterIssuer` già installato nel cluster: non fa parte dei manifest del progetto. |
| DNS wildcard `*.notturni.eu` | ⚪ | Prerequisito infrastrutturale esterno al repository, da configurare quando si affronta il routing per sottodominio. |
| Migrazione progressiva dei microservizi a Rust (orizzonte 3 anni) | ⚪ | Non iniziata, coerente con la roadmap di lungo periodo. |

## 4. Autenticazione e SSO

| Specifica | Stato | Note / task residuo |
|---|---|---|
| Login password + sessione JWT (access + refresh con rotation) | ✅ | |
| MFA TOTP (app authenticator) | ✅ | Via `pyotp`. |
| MFA Email OTP asincrono | 🟡 | Accodato su RabbitMQ e funzionante end-to-end lato coda; il consumer (`app/workers/email_otp_consumer.py`) è ancora un placeholder che **logga il codice invece di inviarlo** — nessun provider SMTP/transazionale collegato. |
| SSO Google/Microsoft/GitHub/LinkedIn (OAuth2/OIDC via Authlib) | 🟡 | Implementato a livello di codice; **non testabile end-to-end** senza credenziali OAuth reali per ciascun provider — verificato solo a livello di logica di dominio. |
| Account linking basato su email, con gate 2FA | ✅ | |
| Token API (`api_tokens`, prefisso `noct_`, hash sha256) | 🟡 | Tabella e logica di dominio pronte per token "core" (motore, machine-to-machine) e token "utente", ma **manca ancora l'interfaccia utente** per generare/revocare un proprio token: oggi è raggiungibile solo via API diretta o script di bootstrap. |
| GDPR: MFA/2FA come rafforzamento sicurezza account | ✅ | Vedi sopra; nessuna funzionalità GDPR-specifica oltre a questo (vedi riga dedicata in [1](#1-prodotto-e-regole-di-dominio)). |
| Sessione in `localStorage` | 🟡 | Scelta pragmatica per la fase attuale di sviluppo; da sostituire con cookie `httpOnly` + CSRF prima di un uso in produzione. |

## 5. Sviluppi futuri (non ancora iniziati)

Elencati nel blueprint come esplicitamente fuori dall'ambito attuale. Restano qui
come backlog, nell'ordine di priorità indicato:

| Specifica | Stato | Note |
|---|---|---|
| Federazione AT Protocol (Bluesky) | ⚪ | Priorità 1. Unico prerequisito già presente: UUID come chiave primaria su tutte le entità. |
| Federazione ActivityPub (Mastodon) | ⚪ | Priorità 2, dopo AT Protocol. Stack di federazione incompatibile con AT Protocol: richiederà un'implementazione separata. |
| Conteggio like, citazioni, condivisioni | ⚪ | |
| Dominio custom dell'utente associato al proprio blog | ⚪ | Dipende dal lavoro di routing per sottodominio (vedi [3](#3-architettura-stack-e-infrastruttura)). |

## Come usare questo documento

- Prima di iniziare un nuovo blocco di lavoro, controlla qui se è già presente
  (anche parzialmente) per evitare di duplicare cose fatte.
- Quando un task passa da 🟡/⚪ a ✅ (o viceversa, se si scopre una regressione),
  aggiorna la riga corrispondente nella stessa modifica che tocca il codice.
- Le righe 🟡 e ⚪ sono, nell'insieme, il backlog: per proporre un prossimo
  passo, si parte da lì.
