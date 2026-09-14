# Notturni frontend

Next.js (App Router) + Tailwind CSS.

Il pannello di amministrazione è parte di questa stessa app, non un
progetto a sé: `dashboard/pagine`, `dashboard/utenti`, `dashboard/blog`
(voci di menu visibili solo ad Amministratore/Super Admin — riusano
`RichTextEditor` e gli altri componenti già usati per i post, nessuna
duplicazione). Vedi [backend/API.md](../backend/API.md#amministrazione-di-piattaforma)
per gli endpoint usati.

## Setup

```bash
npm install
cp .env.local.example .env.local  # opzionale: il default punta già a localhost:8000
```

Serve il backend in esecuzione (vedi `../backend/README.md`) — questo frontend
non funziona a sé stante, ogni pagina autenticata chiama l'API direttamente
dal browser.

## Avvio

```bash
npm run dev
```

Apri [http://localhost:3000](http://localhost:3000).

## Struttura

```text
src/
├── app/
│   ├── login/, register/          # autenticazione (con step MFA)
│   ├── dashboard/                  # area autore: blog, post, profilo
│   │   ├── blogs/[slug]/            # dettaglio blog: tab post/commenti/aspetto/impostazioni
│   │   ├── pagine/, utenti/, blog/, moderazione/  # amministrazione (solo Amministratore/Super Admin): pagine statiche, utenti, elenco blog di piattaforma, moderazione post
│   └── u/[username]/                # profilo pubblico + follow
├── i18n/                            # lingua dell'interfaccia (next-intl): config, risoluzione, cookie
├── messages/{it,en}.json            # stringhe dell'interfaccia, per namespace
├── lib/
│   ├── api.ts                       # client HTTP tipizzato verso il backend
│   ├── auth-context.tsx             # sessione (access/refresh token, auto-refresh su 401)
│   ├── theme-context.tsx            # tema chiaro/scuro/automatico
│   └── sun.ts                       # calcolo alba/tramonto (locale, nessuna chiamata esterna)
└── components/
    ├── ui/                          # Button, Input, Card, Pill, States (Empty/Skeleton/Error), Toast, ConfirmDialog, ...
    ├── shell/                       # DashboardShell, SiteHeader/SiteFooter, BlogHeader, UiLanguagePicker
    ├── blog/                        # pezzi del blog pubblico: BlogPageShell (palette custom), FragmentMenu, PostActions, StatusPill, VisibilityBand
    ├── SearchInput.tsx               # campo di ricerca con debounce, usato dalle sezioni di amministrazione
    └── ThemeToggle.tsx
```

## Autenticazione

`AuthProvider` (`src/lib/auth-context.tsx`) tiene l'access token **solo in
memoria** (mai in `localStorage`): un reload completo lo perde, per cui
all'avvio l'app tenta sempre un refresh silenzioso. Il refresh token vero e
proprio non è mai visibile a JS — vive in un cookie `httpOnly` impostato dal
backend (`backend/API.md`), inviato automaticamente dal browser con
`credentials: "include"` sulle sole chiamate a `/auth/refresh`/`/auth/logout`
(`src/lib/api.ts`). Refresh automatico su `401` come prima, via `authFetch`.

## Tema chiaro/scuro/automatico

`ThemeProvider` (`src/lib/theme-context.tsx`) applica il tema tramite
l'attributo `data-theme` su `<html>` (vedi `globals.css`), con uno script
inline in `layout.tsx` che lo imposta prima dell'idratazione React per
evitare un flash del tema sbagliato.

In modalità **automatica** (default), usa la geolocalizzazione del browser
per calcolare alba/tramonto reali della posizione dell'utente (formula
astronomica standard in `lib/sun.ts`, calcolata interamente lato client — la
posizione non viene mai salvata né inviata al backend, resta solo in memoria
per il calcolo). Se la geolocalizzazione non è concessa o non è disponibile,
ripiega sulle preferenze di sistema (`prefers-color-scheme`) invece di
fallire. Ricalcola ogni 10 minuti e al ritorno in foreground della scheda.

## Lingua dell'interfaccia (next-intl)

Etichette, pulsanti e messaggi vivono in `src/messages/{it,en}.json`, per
namespace (`Nav`, `Auth`, `PostPage`, …). La lingua è risolta in
`src/i18n/request.ts`: cookie `notturni_ui_locale` (impostato dal selettore
nell'header o dal profilo via server action `setUiLocale`) → `Accept-Language`
→ `it`. Nessun segmento di lingua nell'URL: la lingua dei **contenuti** (post,
pagine) ha già il proprio modello locale+slug lato backend e resta separata.
Server Component: `getTranslations()`; Client Component: `useTranslations()`.
Nuova lingua = nuovo file JSON + voce in `LOCALES` (`src/i18n/config.ts`).
Le stringhe non ancora migrate (editor, alcune tab e pagine admin) sono in
italiano hardcoded: vedi `todo/UX_REDESIGN.md` A0.

## Tipografia e palette

Definite in `src/app/globals.css` secondo le linee guida estetiche del
progetto (vedi [ROADMAP.md](../ROADMAP.md#2-estetica)):
titoli in serif (Lora), corpo e link in sans-serif (Source Sans 3), palette di 5 colori
(`background`, `foreground`, `primary`, `muted`, `border`) — questa è la
palette *di piattaforma* (dashboard, admin); ogni blog può personalizzare la
propria (vedi `blogs.getConfig`/`updateConfig` in `lib/api.ts` e la tab
"Aspetto" nella dashboard); la palette del blog è applicata alle sue pagine
pubbliche da `components/blog/BlogPageShell.tsx` come variabili CSS (solo tema
chiaro: in tema scuro vale la palette scura di piattaforma).
