# Redesign UI (frontend-kit) — inventario e roadmap

Riferimento: prototipo consegnato in `frontend-prototype/` (non tracciato —
cartella di lavoro locale, non fa parte del repository). Contiene
`Notturni Interfaces.dc.html` (35 mockup, desktop+mobile, chiaro+scuro),
`HANDOFF.md`/`INVENTORY.md`/`ADMIN-IA.md`/`I18N.md` e l'implementazione
React/Tailwind di riferimento in `frontend-prototype/frontend-kit/`.

Questo documento traccia **l'integrazione di quel prototipo in `frontend/src`**
a blocchi, secondo l'ordine suggerito in `frontend-kit/INVENTORY.md §5`, e
separa i mockup che sono **solo un restyling** (funzionalità già presente e
funzionante, cambia l'aspetto) da quelli che richiedono **lavoro reale**
(backend nuovo e/o funzionalità di dominio non ancora costruita).

## Stato di avanzamento della migrazione

| Fase | Contenuto | Stato |
|---|---|---|
| 1 | Token colore (light/dark), font (Lora + Source Sans 3), `globals.css`, `layout.tsx` | ✅ Fatto — vedi commit su `feat/new-ux-ui` |
| 2 | Primitives `components/ui/*` (Button, Card, Field, Pill, Controls, States, ConfirmDialog, Toast) | ✅ Fatto — vedi commit su `feat/new-ux-ui`. `ConfirmDialog`/`Toast`/`States` adattati senza `next-intl` (stringhe italiane hardcoded, coerenti col resto dell'app oggi); `Card` e `FieldGroup` mantengono il padding/margine di default del componente precedente (il kit li lascia al chiamante, ma tutti gli usi esistenti — 50 per `Card`, 11 file per `FieldGroup` — si affidano al valore implicito: verrà sovrascritto man mano che le singole schermate vengono restilizzate nelle fasi 3-5) |
| 3 | Shell: `DashboardShell` (dashboard+admin layout), `SiteHeader`/`SiteFooter`, `BlogHeader` | ✅ Fatto — vedi commit su `feat/new-ux-ui`. `SiteFooter` minimale (solo nome sito + link al repository): i link a pagine statiche del kit (chi siamo/contatti/privacy/...) non sono stati aggiunti perché sono contenuto libero su `/p/{slug}`, nessuno slug è riservato — servono dati reali o una convenzione di slug per popolarli senza rischiare link rotti. `SiteHeader` senza le voci "Blog"/"Pubblicazioni" (route non ancora esistenti, arriveranno con le fasi 4 e 6). `BlogHeader` senza tab "About" (nessuno slug di pagina blog è garantito, stessa ragione del footer) |
| 4 | Home (`app/page.tsx`) e nuova `/blogs` (directory pubblica) | 🟡 Parziale — vedi commit su `feat/new-ux-ui`. Aggiunta `/blogs` con `GET /api/v1/blogs` (nuovo endpoint pubblico, non c'era: filtra `visibility=public`, non sospesi, `search_indexing_enabled`) e link "Blog" in `SiteHeader`. **Non fatto**: il restyling della home in stile mockup 4a/4b (Manifesto a due colonne, `BlogDirectoryList` in sidebar) — la home resta nella struttura attuale (hero + tendenze + feed), solo la palette/font della Fase 1 si applicano già lì |
| 5 | Tab dashboard blog (Overview nuovo, Comments/Settings estesi) e admin piattaforma | 🟡 Parziale — vedi commit su `feat/new-ux-ui`. Restilizzati con le primitive delle fasi 1-2 (Pill per gli stati, card per palette/collaboratori, layout dei form) i quattro tab già funzionanti: `AppearanceTab`, `CollaboratorsTab`, `PostsTab`, `PagesTab` — nessun campo nuovo (niente preset/generatore variante scura/contrasto AA/anteprima live del mockup 2e, non supportati da `blog_configs`). **Non fatto, richiede prima decisioni di prodotto** (vedi §3 sotto): `OverviewTab`, `CommentsQueue`/`CommentPolicyCard` estesi, `SettingsForm` con pausa/trasferimento/soft-delete, tutto l'admin di piattaforma (`AdminOverview`, `BlogsTable`+`ReportPanel` con nota di audit obbligatoria, `GdprRequestsTable`, `PlatformSettingsForm`) |
| 6 | Publications + gestione Note come entità di prima classe (richiede backend nuovo) | ⚪ Da fare |
| — | 1h Login/MFA (`/login`, `/register`): brand + headline stile mockup, caselle OTP per cifra invece di un campo unico | ✅ Fatto — vedi commit su `feat/new-ux-ui`. **Non aggiunti**: pulsanti SSO (il mockup li mostra ma `GET /api/v1/auth/sso/{provider}/callback` oggi risponde con JSON grezzo invece di reindirizzare al frontend con una sessione — collegarli avrebbe portato l'utente su una pagina JSON nuda, peggio che ometterli; richiede prima un redirect callback→frontend lato backend), link "Password dimenticata" (nessun endpoint di reset password esiste), "Invia un codice via email"/"Usa un codice di recupero" nello step MFA e l'avviso "nuovo dispositivo rilevato" (nessuna di queste funzionalità esiste lato backend: `method` nella sfida MFA è fisso, non c'è un canale alternativo né codici di recupero né rilevamento dispositivo) |
| — | 1d Editor chrome (`/dashboard/blogs/[slug]/posts/*`): rail destro a tab (Post/Traduzioni) al posto dei controlli accumulati sopra la toolbar, popover per la nota al posto del `window.prompt`, avatar colorato nel menu di autocomplete delle @menzioni | ✅ Fatto — vedi commit su `feat/new-ux-ui`. Nuovo `components/editor/EditorRail.tsx` (tab generiche, riusabile). Stato del post ora `SegmentedControl` (primitiva della Fase 2, prima inutilizzata) invece di un `<select>`, ma resta **2 vie** (Bozza/Pubblica ora): il mockup ne mostra 3 (+ Review, + Scheduled) ma l'invio in revisione (`POST .../submit-for-review`, esiste lato backend) e la pianificazione (`published_at`) non sono mai stati collegati lato frontend — funzionalità reale non ancora costruita, non solo stile, va trattata come blocco a sé. Non aggiunti nel rail: "Allow @mentions" (`Blog.mentions_enabled` è un'impostazione di blog, già in `SettingsTab`, non per-post) e "Content warning" (già gestito dalle pillole in sovraimpressione di `CoverImageUpload`/`SensitiveImageNodeView`) — duplicarli nel rail avrebbe rischiato di disallinearli dalla fonte reale. **Non verificato dal vivo** sullo stack podman (nessun container in esecuzione in questa sessione): solo `npm run build`/`eslint` puliti — nessuna nuova chiamata API, comportamento invariato a parità di dati. |
| — | 2f Profilo utente (`/dashboard/profile`): nav interna sticky (Identità/Lingue/Link social/Sicurezza/Privacy), sezione Identità con avatar+campi in griglia, "Firma i miei post come" come 3 card selezionabili invece di un `<select>`, "Privacy e dati" come lista titolo/sottotitolo/azione | ✅ Fatto — vedi commit su `feat/new-ux-ui`. Stessi campi/endpoint di prima, solo riorganizzati: un unico `<form>` copre ancora Identità+Lingue (un solo salvataggio, `PATCH /users/me`); Link social/MFA/Privacy restano azioni indipendenti come oggi. **Non aggiunti**: "Lingua dell'interfaccia" (richiede `next-intl`/`users.ui_locale`, non ancora costruito — vedi riga i18n sotto) e "Sessioni · N" (nessuna gestione multi-sessione lato backend, solo un cookie di refresh) — il mockup li mostra ma non esiste nulla da collegare. |
| — | i18n dell'interfaccia (`next-intl`, non presente oggi come dipendenza) | ⚪ Rimandato a blocco dedicato, non legato a una fase specifica sopra |

Ogni fase è un blocco a sé (vedi CLAUDE.md §2): non anticipare la fase
successiva senza indicazione esplicita.

## 1. Mockup che sono solo restyling (nessun gap funzionale)

La funzionalità sottostante esiste già e funziona; il prototipo cambia solo
aspetto/interazione. Migrabili nelle fasi 2-5 senza toccare il backend.

| Mockup | Route reale | Riferimento ROADMAP.md |
|---|---|---|
| 1a/1b Post pubblico | `/[blogSlug]/[postSlug]` | §1 righe "Note a piè di pagina...", "Frammenti" |
| 1c Home | `/` | §1 "notturni.eu: raccolta articoli..." |
| ~~1d Editor~~ | ~~`/dashboard/blogs/[slug]/posts/*`~~ | ✅ Fatto — vedi tabella fasi sopra |
| 1e/1f Dashboard utente | `/dashboard` | §1 "Dashboard utente..." |
| 1g Admin: tabella utenti | `/admin/utenti` | §1 "4 ruoli piattaforma..." |
| ~~1h Login/MFA~~ | ~~`/login`, `/register`~~ | ✅ Fatto — vedi tabella fasi sopra |
| 2a/2b/2c Bibliografia/media/link | `/[blogSlug]/bibliografia`, `/media`, `/link` | §1 "Note a piè di pagina + bibliografia automatica" |
| 2e Impostazioni blog | `/dashboard/blogs/[slug]` (tab Aspetto/Collaboratori) | §1 "Personalizzazione colori/tipografia...", "Inviti a collaborare" |
| ~~2f Profilo (identità, privacy & dati)~~ | ~~`/dashboard/profile`~~ | ✅ Fatto — vedi tabella fasi sopra |
| 2g Frammenti | `/dashboard/frammenti` | §1 "Frammenti: raccolta personale..." |
| 3a Bib/media/link mobile | stesse route | come sopra |
| 3c Media lightbox | editor / libreria media | §1 "Moderazione automatica delle immagini" |
| 3e Profilo pubblico | `/u/[username]` | §1 "Follow tra utenti...", "Profilo utente" |

## 2. Mockup con un gap noto già in ROADMAP.md (restyling + piccolo lavoro reale)

| Mockup | Gap | Dettaglio |
|---|---|---|
| 3f Home blog con palette custom | Palette/tipografia di `blog_configs` salvata e validata ma **non applicata al rendering pubblico** del blog | ROADMAP.md §1, riga "Personalizzazione colori/tipografia/presentazione per blog": va letta `blog_configs` e iniettata come variabili CSS sulla root della pagina `/[blogSlug]` |

## 3. Mockup che richiedono lavoro reale (backend nuovo o esteso)

Nessuno di questi va anticipato senza indicazione esplicita — sono blocchi a
sé, coerenti con l'ordine di priorità del backlog in ROADMAP.md §5 quando
applicabile.

| Mockup | Cosa manca | Riferimento |
|---|---|---|
| 2d, 3g Publications (indice, capitoli, drag-to-order) | Funzionalità non iniziata: tabelle `publications`/`chapters` (ordine, stato), route pubbliche `/[blogSlug]/pub/[nome]`/`[capitolo]`, gestione in dashboard | ROADMAP.md §1 riga "Pubblicazioni: serie di articoli..." (⚪); intento di prodotto in `todo/PUBLICATIONS.md` |
| 3b Gestione note (tabella, merge duplicati, "citato in") | Oggi le note sono un elenco per-post (`post_notes`, ✅ per la bibliografia), non entità di prima classe con tipo/URL, deduplica assistita o pannello di gestione dedicato | ROADMAP.md §1 riga "Note a piè di pagina + bibliografia automatica" (✅ solo per l'uso attuale, non per questa gestione avanzata) |
| ~~4c Directory blog pubblica~~ | ✅ Fatto in fase 4: `GET /api/v1/blogs` (`backend/app/api/v1/blogs/crud.py::list_public_blogs`), `/blogs` (`frontend/src/app/blogs/page.tsx`) | `backend/API.md` |
| 5a/5g Overview blog (KPI, grafico letture, storage) | Manca `GET /blogs/{slug}/overview`; `ReadsChart` richiede solo aggregati giornalieri (privacy) | Nuovo |
| 5d Overview piattaforma (code, servizi) | Manca `GET /admin/overview` | Nuovo |
| 5b Coda commenti estesa (block list, escalation al piattaforma) | Oggi: 3 stati (`members`/`everyone`/`closed`) e approvazione/rifiuto (✅). Mancano: lista utenti bloccati per blog, escalation di un commento al moderatore di piattaforma | ROADMAP.md §1 riga "Sistema di commenti con moderazione dell'autore" (✅ solo per lo scope attuale) |
| 5c Impostazioni blog: pausa, trasferimento proprietà, cancellazione soft (30gg) | Oggi esiste solo la sospensione **imposta da admin** (`Blog.is_suspended`). Pausa volontaria del proprietario, trasferimento ownership e soft-delete non esistono | ROADMAP.md §1 righe 24-26 (GDPR/cancellazione account) lo notano esplicitamente come fuori scope oggi |
| 5f Impostazioni piattaforma (form completo) | `GET /api/v1/config` espone solo alcuni valori read-only oggi; manca un form admin per `default_locale`, `registration`, `sso_providers`, `mfa_for_admins`, `reserved_names`, `moderation_threshold`, `max_blogs_per_user`, `anonymous_comments` | Nuovo — richiede anche persistenza lato backend per questi campi |
| 5f Coda richieste GDPR con scadenze e doppia approvazione admin | Oggi export/cancellazione sono self-service immediati (✅), non una coda mediata da admin con SLA e secondo approvatore | ROADMAP.md §1 righe 25-26 — funzionalità diversa da quella già costruita, non un'estensione diretta |
| Registro di controllo: nota obbligatoria per azione admin | L'audit log oggi è automatico, senza campo di nota libera per l'azione | ROADMAP.md §3 riga "Audit log delle azioni sensibili" (🟡) |
| i18n interfaccia (`next-intl`, `users.ui_locale`) | Non presente come dipendenza; richiede `next.config.ts`, routing locale, 291 stringhe in `messages/{it,en}.json`, colonna `users.ui_locale` | `frontend-kit/I18N.md` |

## 4. Prossimo blocco suggerito

Fase 2 (primitives `components/ui/*`): stesso rischio basso della Fase 1,
nessuna dipendenza nuova, nessuna route nuova — sostituzione di componenti
esistenti con superset della stessa API (vedi `frontend-kit/INVENTORY.md §2.2`).
Da confermare con l'utente prima di iniziare (CLAUDE.md §2).
