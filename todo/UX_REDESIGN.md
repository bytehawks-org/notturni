# Redesign UI (frontend-kit) — inventario e roadmap

Riferimento: prototipo consegnato in `frontend-prototype/` (non tracciato —
cartella di lavoro locale, non fa parte del repository). Contiene
`Notturni Interfaces.dc.html` (35 mockup, desktop+mobile, chiaro+scuro),
`HANDOFF.md`/`INVENTORY.md`/`ADMIN-IA.md`/`I18N.md` e l'implementazione
React/Tailwind di riferimento in `frontend-prototype/frontend-kit/`.

Questo documento traccia **l'integrazione di quel prototipo in `frontend/src`**
a blocchi e separa i mockup che sono **solo un restyling** (funzionalità già
presente, cambia l'aspetto) da quelli che richiedono **lavoro reale** (backend
nuovo e/o funzionalità di dominio non ancora costruita).

## Stato di avanzamento della migrazione

| Blocco | Contenuto | Stato |
|---|---|---|
| 1 | Token colore (light/dark), font (Lora + Source Sans 3), `globals.css`, `layout.tsx` | ✅ |
| 2 | Primitives `components/ui/*` (Button, Card, Field, Pill, Controls, States, ConfirmDialog, Toast) | ✅ — `Toast` ha ora anche `ToastProvider`/`useToast()` (montato in `layout.tsx`, auto-dismiss 5 s); `States` ha anche `SkeletonCards` |
| 3 | Shell: `DashboardShell`, `SiteHeader`/`SiteFooter`, `BlogHeader` | ✅ — `SiteHeader` con `UiLanguagePicker`; `BlogHeader` è un Server Component (`getTranslations`) con slot `actions` (tema, accesso, `FollowBlogButton`) |
| A0 | i18n dell'interfaccia con `next-intl` | ✅ — `src/i18n/{config,request,actions}.ts`, `src/messages/{it,en}.json` (le 291 stringhe del kit + quelle delle schermate migrate), `next.config.ts` con `createNextIntlPlugin`, `NextIntlClientProvider` in `layout.tsx`, `<html lang>` dinamico. Risoluzione: cookie `notturni_ui_locale` → `Accept-Language` → `it` (fallback italiano, non `en` come nel kit: le stringhe non ancora migrate sono italiane e un fallback inglese mischierebbe le lingue). Selettore nell'header pubblico e in Profilo → Lingue. **Non fatto**: `platform_config.default_locale` e `users.ui_locale` (blocco B6), quindi la scelta è per browser, non per account. **Stringhe ancora hardcoded in italiano**: editor (`posts/new`, `posts/[postId]` tranne lo stato, `RichTextEditor`, `TranslationsBar`, `CoverImageUpload`), tab `PagesTab`/`CollaboratorsTab`/`SettingsTab`/`MyMembershipCard`, `components/dashboard/blog/shared.ts`, pagine admin `blog`/`moderazione`/`moderazione-commenti`/`pagine`/`registro`, pagine statiche `/p` e `/[blog]/pagina`, `LanguagePicker`, `Alert`. Si migrano man mano che le schermate vengono toccate |
| A1 | 1a/1b Post pubblico | ✅ — griglia `1fr 680px 1fr` da `xl`, indice "In questo post" dai titoli h2/h3 (`renderPost` in `lib/markdown.ts` assegna gli id e restituisce `headings`; sotto `xl` è un `<details>`), rail destro con note/categoria/traduzioni (`GET /posts/{id}/translations`), meta con iniziale/autore/data/tempo di lettura (`lib/format.ts::readingMinutes`, ~200 parole/min), `PostActions` (Condividi via Web Share API o copia link; Cita copia "Autore, “Titolo”, Blog, data. URL"), `FragmentMenu` del kit dentro `FragmentReader` (desktop flottante, mobile bottom-sheet; Salva/Salva pubblico/Rimuovi, Copia link, Cita con conteggio parole e %; funziona anche da anonimi per link/citazione), riquadro "I tuoi frammenti qui · n · apri lo scaffale", `CommentsSection` con conteggio e riga di policy, nota privacy in fondo. **Non fatto**: "Save" del mockup (nessuna funzione "salva post" esiste — coperto dallo scaffale frammenti), didascalia/alt della copertina (nessun campo sul post), numero follower nella riga meta (è nel pulsante Segui dell'header) |
| A2 | 4a/4b/1c Home di piattaforma, 4c `/blogs` | ✅ — `Manifesto` a due colonne con i 4 pilastri, `TrendingTags`, feed con chip lingua (`?locale=`, filtro già supportato da `GET /feed/posts`) e filtri tag/categoria, sidebar `BlogDirectoryList` + card "Tuo, da portare via"; `/blogs` con ricerca `?q=` e filtro lingua `?locale=` **applicati lato pagina** sulla lista (max 100) perché `GET /blogs` non li supporta; nuova `FeedPostCard` (Server Component: autore, blog, data, estratto, categoria, minuti, note, tag, copertina). **Non fatto**: "Pubblicazioni in corso" (B9), "Dal blog di piattaforma", paginazione "Post precedenti" e RSS. Tab "Seguiti", conteggi e ricerca/lingua lato API: fatti in B1 |
| A3 | 3f Home del blog con palette custom | ✅ — hero (iniziale colorata, titolo, sottotitolo, descrizione, host), chip delle categorie (`?category=` filtrato lato pagina sui post del blog), feed con `FeedPostCard`, card "Su questo blog". **Palette custom applicata**: `components/blog/BlogPageShell.tsx` legge `GET /blogs/{slug}/config` (pubblico) e inietta `palette.*` come variabili CSS sulla root di **tutte** le pagine pubbliche del blog (home, post, bibliografia, media, link); la palette di default del backend non viene iniettata (blog mai personalizzato = identico allo shell). In tema scuro il CSS `.blog-palette` ripristina la palette scura di piattaforma. **Non fatto**: font del blog sul rendering pubblico (richiederebbe caricare Google Fonts a runtime, contro la promessa "nessun font di terze parti nelle pagine pubbliche" — da decidere), colonna destra del mockup (Autori: `GET /blogs/{slug}/members` è solo autenticato; Pubblicazioni: B9; Newsletter: non esiste), RSS |
| A4 | 2a/2b/2c/3a Bibliografia, media, link | ✅ — bibliografia numerata con "Citata in", conteggio citazioni e ordinamento `?sort=cited`; media in griglia 2/4 colonne con vista `?view=post` (raggruppata per primo post) e sfocatura CSS delle immagini segnalate; link raggruppati per host e ordinati per numero. Tutte sotto `BlogPageShell` (palette). **Non fatto**: filtro per tipo di nota (libro/articolo/web — le note non hanno `kind`, B8), "Export BibTeX" (B8), vista "Timeline" dei media |
| A5 | 1e/1f Dashboard utente | ✅ — saluto in base all'ora, riga riepilogo (bozze, inviti, commenti da moderare), KPI (post, follower da `follow-stats`, blog), "Post recenti" aggregati client-side dai propri blog con filtri Tutti/Bozze/In revisione/Pianificati/Pubblicati e `StatusPill`, card invito, "Commenti da moderare" (prime 3, link alla tab), card dei blog con `BlogCard`/`VisibilityBand` del kit (colori `--vis-*` di HANDOFF), stati vuoti/skeleton. Letture: fatte in B2 (tab Panoramica del blog). **Non fatto**: "nuovi follower questa settimana" (nessuna data sul follow esposta) |
| A6 | 2e Aspetto | ✅ — preset di palette calme (Notturni/Carta/Ardesia/Lavanda), verifica contrasto AA (`lib/contrast.ts`, WCAG: testo ≥ 4.5, primario/attenuato ≥ 3), corpo 17/18/19 e misura stretta/normale (chiavi libere `typography.body_size`/`measure`, accettate dal backend ma **non ancora applicate al rendering pubblico**), layout come `SegmentedControl`, anteprima live vista lettore. "Genera variante scura": fatta in B1 (`palette_dark`) |
| A7 | 3e Profilo pubblico | ✅ — header con avatar, `@username`, paese e lingue nella lingua dell'interfaccia, bio, follower e "su Notturni da", link social, nota privacy. Tab Post/Blog/Commenti: fatte in B1 (solo contenuti firmati con lo username) |
| A8 | 1g/5d Admin | ✅ — panoramica con code aperte (commenti in attesa, post in revisione, post nascosti, blog sospesi) calcolate dagli endpoint admin esistenti, scorciatoie alle sezioni; tabella utenti con MFA, stato, data iscrizione, pillole. KPI, servizi, audit di oggi, colonne blog/ultimo accesso ed export CSV: fatti in B1. **Non fatto**: tema scuro di default per l'admin |
| A9 | 3d Stati | ✅ — `EmptyState`/`SkeletonRows`/`SkeletonCards`/`ErrorState` usati in dashboard, scheda blog, `PostsTab`, `CommentsTab`, token, admin, frammenti, profilo pubblico; `ToastProvider` per le conferme (frammenti, copia link/citazione). Avviso "blog sospeso"/"in pausa" sulla pagina pubblica: fatto in B3 (`BlogStateNotice`) |
| A10 | 1d Editor: stato a 3 vie + pianificazione | ✅ — `components/editor/PostStatusControl.tsx`: Bozza · In revisione · Pubblica ora (`submit-for-review`/`return-to-draft`/`publish`), "Pianifica la pubblicazione…" con `datetime-local` → `publish` con `published_at` futuro; `lib/post-status.ts::displayPostStatus` deriva "pianificato" da `published` + data futura; tipo `PostStatus` allineato al backend (`pending_review`). `PostsTab` con filtri per stato |
| — | 1h Login/MFA, 1d rail editor, 2f Profilo, 2g Frammenti | ✅ (sessioni precedenti) — ora migrati a `next-intl`; 2f ha in più "Lingua dell'interfaccia" |
| B1 | Endpoint leggeri di supporto | ✅ — backend: `GET /blogs/{slug}/overview` (conteggi, proprietario/membri), `GET /admin/overview` (KPI, code, audit di oggi, salute di postgres/redis/rabbitmq/storage/moderazione), `GET /users/{username}/blogs\|posts\|comments` (solo contenuti firmati con lo username — CLAUDE.md #8), `GET /feed/posts?following=true`, `GET /blogs?q=&locale=&sort=` con `post_count`/`follower_count`/`last_published_at`, `blogs_count`/`last_seen_at` su `AdminUser`, `palette_dark` validata in `blog_configs`; test in `backend/tests/test_overview.py`. Frontend: tab Panoramica del blog (`OverviewTab`, default), tab "Seguiti" in home (`FollowingFeed`, client), tab Post/Blog/Commenti nel profilo pubblico, directory con conteggi e ricerca/lingua lato API, panoramica admin con KPI/servizi, colonne blog/ultimo accesso + export CSV utenti, "Genera variante scura" in Aspetto (`lib/contrast.ts::deriveDarkPalette`) applicata da `BlogPageShell` in tema scuro. **Rimandato**: grafico letture (B2) |
| B2 | 5a/5g Letture aggregate e spazio | ✅ — tabella `post_reads_daily` (post, giorno UTC, contatore; migrazione `a7c1d2e3f4b5`), `POST /posts/{id}/read` pubblico senza cookie/identificativi (rate limit Redis 1 per IP+post ogni 6 h, oltre il limite 204 senza contare), `reads_30d`/`reads_total_30d`/`storage_bytes` nell'overview (spazio = somma degli oggetti sotto `userdata/{utente}/{blog}/` di proprietario e collaboratori, `core/storage.py::blog_storage_bytes`). Frontend: `ReadBeacon` (sendBeacon dopo 8 s sulla pagina del post), `ReadsChart` SVG e card Spazio in `OverviewTab`. Test in `backend/tests/test_reads.py`. **Non fatto**: `BlogTabs` con gating per ruolo dei membri (oggi le tab dipendono da proprietario/non proprietario; i ruoli Revisore/Mediatore vedono tutto in sola lettura come prima), quota di spazio per blog (nessuna quota esiste) |
| B3 | 5c Impostazioni blog + danger zone | ✅ — backend: colonne `is_paused`, `deleted_at`, `extra_locales` (migrazione `b8d2e3f4a5c6`); `PATCH /blogs/{slug}` accetta `is_paused`/`extra_locales`; `POST /blogs/{slug}/transfer` (a un coautore, chi cede resta coautore), `GET /blogs/{slug}/export` (ZIP generato al volo: Markdown per post/pagina con front matter e note, JSON per commenti/categorie/media/link — **niente link a scadenza né job asincrono**, scelta più semplice del mockup), `DELETE /blogs/{slug}` con `confirm_slug` (cancellazione soft) e `POST /blogs/{slug}/restore`; purge definitivo dopo 30 giorni nel worker `audit_maintenance` (`domain/blog_lifecycle.py::purge_deleted_blogs`, contenuti + oggetti su storage). Regole di visibilità: blog in pausa/cancellato fuori da feed, directory, sitemap ed elenchi pubblici; `GET /blogs/{slug}` di un blog pubblico in pausa o sospeso risponde 200 con i flag (prima 404) così la pagina pubblica mostra l'avviso (mockup 3d/5c). Test in `backend/tests/test_blog_lifecycle.py`. Frontend: `SettingsTab` riscritto a sezioni (identità, visibilità a card + crawler, lingue secondarie dalle lingue del profilo, funzionalità e policy commenti a card, dominio disattivato, zona pericolosa con trasferimento/pausa/export/cancellazione via `ConfirmDialog`), `BlogStateNotice` sulle pagine pubbliche del blog, badge "in pausa"/"in cancellazione" in dashboard. **Non fatto**: newsletter (non esiste), dominio personalizzato (routing per sottodominio non ancora costruito) |
| B4 | 5b Coda commenti estesa | ✅ — backend: tabella `blog_blocked_authors` (utente o sha256 dell'email anonima, mai email/IP in chiaro), colonne `reported_to_platform`/`report_note`/`reported_at` su `comments`, `comments_auto_close_days` su `blogs` (migrazione `c9e3f4a5b6d7`); `POST /comments/{id}/report` (nota obbligatoria, audit), `POST /comments/{id}/block-author` (blocca + nasconde, audit), `GET/DELETE /blogs/{slug}/blocked`, `?reported=true` su elenco per blog e admin, chiusura automatica in `domain/comments_mode.py` (`effective_comments_mode` → `closed` dopo N giorni). Lo stato "nascosto" è il `rejected` esistente. Test in `backend/tests/test_comment_moderation_ext.py`. Frontend: `CommentsTab` con code In attesa/Approvati/Nascosti/Segnalati e conteggi, selezione multipla, risposta inline, blocco, segnalazione con nota, card regole (policy + chiusura a 90 giorni) e lista bloccati; `/admin/moderazione-commenti` con filtro Segnalati e nota. **Non fatto**: "Avvisami via email" (nessun canale email transazionale oltre all'OTP) |
| B5–B9 | Funzionalità che richiedono tabelle nuove | ⚪ — vedi §4 |

Ogni blocco è a sé (vedi CLAUDE.md §2): non anticipare il successivo senza
indicazione esplicita.

## 1. Mockup che sono solo restyling — tutti ✅

1a/1b, 1c/4a/4b, 1d, 1e/1f, 1g, 1h, 2a/2b/2c, 2e, 2f, 2g, 3a, 3d, 3e, 3f, 4c:
fatti (vedi tabella sopra per i dettagli e le parti rimandate perché
richiedono backend). Restano fuori, per scelta: 3c lightbox media (richiede
una libreria media per blog, B7) e le voci del mockup senza nulla da collegare
(SSO nel login, reset password, sessioni multiple, RSS, newsletter).

## 2. Gap noti già in ROADMAP.md

| Mockup | Gap | Stato |
|---|---|---|
| 3f Palette custom sul rendering pubblico | `blog_configs` era solo salvata/validata | ✅ Palette applicata (A3). Tipografia (`heading_font`/`body_font`, `body_size`, `measure`) e `layout` ancora **non** applicati al rendering pubblico |

## 3. Mockup che richiedono lavoro reale (backend nuovo o esteso)

| Mockup | Cosa manca | Riferimento |
|---|---|---|
| 2d, 3g Publications | Tabelle `publications`/`chapters`, route `/[blog]/pub/[nome]/[capitolo]`, gestione in dashboard con drag-to-order | ROADMAP.md §1 "Pubblicazioni" (⚪); `todo/PUBLICATIONS.md` |
| 3b Gestione note | Note come entità di blog con `kind`/URL, duplicati, "citato in", import/export BibTeX | ROADMAP.md §1 "Note a piè di pagina + bibliografia automatica" |
| 3c Libreria media + lightbox | `GET /blogs/{slug}/media`, alt/didascalia/avviso per immagine, quota storage | Nuovo |
| 5a/5g Overview blog | `GET /blogs/{slug}/overview`, letture aggregate giornaliere (privacy) | Nuovo |
| 5d Overview piattaforma | `GET /admin/overview` (code, salute servizi, audit di oggi) | Nuovo |
| 5e Segnalazioni + nota di audit | `blog_reports`/segnalazioni post dai lettori, campo `note` obbligatorio nelle azioni admin | ROADMAP.md §3 "Audit log" (🟡) |
| 5f Impostazioni piattaforma + coda GDPR | Tabella `platform_config` (oggi solo env), coda richieste con scadenza e seconda approvazione, `users.ui_locale`/`default_locale` | Nuovo |

## 4. Parte B — piano concordato (un blocco per sessione, in ordine di dipendenza)

| Blocco | Mockup | Backend | Frontend (sblocca) |
|---|---|---|---|
| B1 | supporto A2/A5–A8 | ✅ fatto — vedi tabella sopra | ✅ |
| B2 | 5a/5g | ✅ fatto — vedi tabella sopra | ✅ (gating per ruolo delle tab rimandato) |
| B3 | 5c | ✅ fatto — vedi tabella sopra (export sincrono invece che via worker) | ✅ |
| B4 | 5b | ✅ fatto — vedi tabella sopra | ✅ |
| B5 | 5e/5d | `blog_reports`/segnalazioni post dai lettori, `note` obbligatoria in `audit_log` per le azioni admin, segnale "blog sospeso" sulle route pubbliche | `BlogsTable`, `ReportPanel`, `AdminOverview` completo, avviso blog sospeso (3d) |
| B6 | 5f | Tabella `platform_config` (default_locale, registrazione, SSO, MFA admin, nomi riservati, soglia moderazione, max blog, commenti anonimi); coda GDPR con scadenza e seconda approvazione; `users.ui_locale` | `PlatformSettingsForm`, `GdprRequestsTable`, lingua dell'interfaccia per account |
| B7 | 3c | `GET /blogs/{slug}/media`, alt/didascalia/avviso su `post_media`, quota | Libreria media + lightbox |
| B8 | 3b | Note come entità (`kind`, URL/DOI), duplicati, "citato in", import/export BibTeX | Gestione note; filtro per tipo in 2a |
| B9 | 2d/3g | `publications`/`chapters`, route `/pub/[nome]/[capitolo]` | `PublicationIndex`, `ChapterNav`, drag-to-order, voce "Pubblicazioni" in header/home |

Decisioni di prodotto ancora aperte, mostrate dal mockup ma non coperte da
nessun blocco: RSS, newsletter con double opt-in, sessioni multiple, reset
password, pulsanti SSO nel login (il callback risponde JSON grezzo), invio
del codice MFA via email come canale alternativo.
