# Notturni frontend

Next.js (App Router) + Tailwind CSS.

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
│   │   └── blogs/[slug]/            # dettaglio blog: tab post/commenti/aspetto/impostazioni
│   ├── admin/                       # area amministrativa (super_admin/amministratore)
│   └── u/[username]/                # profilo pubblico + follow
├── lib/
│   ├── api.ts                       # client HTTP tipizzato verso il backend
│   ├── auth-context.tsx             # sessione (access/refresh token, auto-refresh su 401)
│   ├── theme-context.tsx            # tema chiaro/scuro/automatico
│   └── sun.ts                       # calcolo alba/tramonto (locale, nessuna chiamata esterna)
└── components/
    ├── ui/                          # Button, Input, Card, Alert, ...
    └── ThemeToggle.tsx
```

## Autenticazione (scelta pragmatica, da rivedere prima della produzione)

Sessione (access + refresh token) tenuta in `localStorage` e gestita da
`AuthProvider` (`src/lib/auth-context.tsx`), con refresh automatico su `401`.
Più semplice da costruire di un flusso con cookie `httpOnly`, ma esposta a
furto del token via XSS — da rafforzare (cookie `httpOnly` + CSRF token)
prima di un uso in produzione reale.

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

## Tipografia e palette

Definite in `src/app/globals.css` secondo le linee guida estetiche del
progetto (vedi [ROADMAP.md](../ROADMAP.md#2-estetica)):
titoli in serif (Lora), corpo e link in sans-serif (Inter), palette di 5 colori
(`background`, `foreground`, `primary`, `muted`, `border`) — questa è la
palette *di piattaforma* (dashboard, admin); ogni blog può personalizzare la
propria (vedi `blogs.getConfig`/`updateConfig` in `lib/api.ts` e la tab
"Aspetto" nella dashboard).
