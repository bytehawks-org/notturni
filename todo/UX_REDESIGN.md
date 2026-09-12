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
| 2 | Primitives `components/ui/*` (Button, Card, Field, Pill, Controls, States, ConfirmDialog, Toast) | ⚪ Da fare |
| 3 | Shell: `DashboardShell` (dashboard+admin layout), `SiteHeader`/`SiteFooter`, `BlogHeader` | ⚪ Da fare |
| 4 | Home (`app/page.tsx`) e nuova `/blogs` (directory pubblica) | ⚪ Da fare |
| 5 | Tab dashboard blog (Overview nuovo, Comments/Settings estesi) e admin piattaforma | ⚪ Da fare |
| 6 | Publications + gestione Note come entità di prima classe (richiede backend nuovo) | ⚪ Da fare |
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
| 1d Editor | `/dashboard/blogs/[slug]/posts/*` | §1 "Editor pulito..." (✅ Tiptap completo) |
| 1e/1f Dashboard utente | `/dashboard` | §1 "Dashboard utente..." |
| 1g Admin: tabella utenti | `/admin/utenti` | §1 "4 ruoli piattaforma..." |
| 1h Login/MFA | `/login`, `/register` | §4 "Login password...", "MFA TOTP" |
| 2a/2b/2c Bibliografia/media/link | `/[blogSlug]/bibliografia`, `/media`, `/link` | §1 "Note a piè di pagina + bibliografia automatica" |
| 2e Impostazioni blog | `/dashboard/blogs/[slug]` (tab Aspetto/Collaboratori) | §1 "Personalizzazione colori/tipografia...", "Inviti a collaborare" |
| 2f Profilo (identità, privacy & dati) | `/dashboard/profile` | §1 "Export dei dati...", "Cancellazione dell'account...", "Profilo utente" |
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
| 4c Directory blog pubblica | Manca `GET /blogs?public=1&indexed=1` (elenco blog pubblici indicizzabili) | Nessuna riga dedicata oggi in ROADMAP.md — nuovo endpoint |
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
