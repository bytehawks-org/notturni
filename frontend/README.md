# Notturni frontend

Next.js (App Router) + Tailwind CSS.

The administration panel is part of this same app, not a separate project:
`dashboard/pagine`, `dashboard/utenti`, `dashboard/blog` (menu entries
visible only to Amministratore / Super Admin). They reuse `RichTextEditor`
and the other components already used for posts, with no duplication. See
[docs/API.md](../docs/API.md#platform-administration) for the endpoints used.

## Setup

```bash
npm install
cp .env.local.example .env.local  # optional: the default already points to localhost:8000
```

The backend must be running (see `../docs/BACKEND.md`). This frontend does
not work on its own: every authenticated page calls the API directly from
the browser.

## Start

```bash
npm run dev
```

Open [http://localhost:3000](http://localhost:3000).

## Structure

```text
src/
├── app/
│   ├── login/, register/          # authentication (with MFA step)
│   ├── dashboard/                  # author area: blogs, posts, profile
│   │   ├── blogs/[slug]/            # blog detail: tabs posts/comments/aspetto (appearance)/impostazioni (settings)
│   │   ├── pagine/, utenti/, blog/, moderazione/  # administration (Amministratore/Super Admin only): static pages, users, platform blog list, post moderation
│   └── u/[username]/                # public profile + follow
├── i18n/                            # UI language (next-intl): config, resolution, cookie
├── messages/{it,en}.json            # UI strings, per namespace
├── lib/
│   ├── api.ts                       # typed HTTP client for the backend
│   ├── auth-context.tsx             # session (access/refresh token, auto-refresh on 401)
│   ├── theme-context.tsx            # light/dark/automatic theme
│   └── sun.ts                       # sunrise/sunset calculation (local, no external call)
└── components/
    ├── ui/                          # Button, Input, Card, Pill, States (Empty/Skeleton/Error), Toast, ConfirmDialog, ...
    ├── shell/                       # DashboardShell, SiteHeader/SiteFooter, BlogHeader, UiLanguagePicker
    ├── blog/                        # public blog pieces: BlogPageShell (custom palette), FragmentMenu, PostActions, StatusPill, VisibilityBand
    ├── SearchInput.tsx               # search field with debounce, used by the administration sections
    └── ThemeToggle.tsx
```

## Search

- **Modal** (`components/SearchModal.tsx`, opened from the header): suggestions appear while typing. Constants in
  `src/lib/search.ts`: minimum 2 characters (shorter input sends no request), input limited to 100 characters, 8 suggestions,
  300 ms debounce. Each new query aborts the previous request. On a blog, suggestions cover that blog's posts; on the portal, all public posts.
- **Scope in `solo` mode**: the header modal uses the primary blog, and the proxy serves `/search` from `/{primary}/search`. If the
  primary slug is not available, search covers all public posts. The API route `/search/posts` itself is not filtered by mode.
- **Results pages**: `/search` (portal) and `/{blogSlug}/search` (one blog). They are plain `?q=` GET forms with no client-side
  JavaScript. Results are paginated with `?page=`: 20 per page, pages 1 to 1000. The pages fetch on the server, so a `429`
  from the API shows a dedicated message.
- **Limits**: the API caps `q` at 200 characters and rate-limits both search routes to 120 requests per minute per IP (see
  [docs/API.md](../docs/API.md#search)). The server-side fetch forwards `X-Forwarded-For` unchanged, so the rate limit is
  per visitor only behind a reverse proxy that rewrites that header.

## Authentication

`AuthProvider` (`src/lib/auth-context.tsx`) keeps the access token **in
memory only** (never in `localStorage`). A full reload loses it, so on
startup the app always tries a silent refresh. The refresh token itself is
never visible to JS. It lives in an `httpOnly` cookie set by the backend
(see `docs/API.md`). The browser sends it automatically with
`credentials: "include"`, and only on the `/auth/refresh` and `/auth/logout`
calls (`src/lib/api.ts`). Automatic refresh on `401` works as before, via
`authFetch`.

## Light / dark / automatic theme

`ThemeProvider` (`src/lib/theme-context.tsx`) applies the theme through the
`data-theme` attribute on `<html>` (see `globals.css`). An inline script in
`layout.tsx` sets it before React hydration, to avoid a flash of the wrong
theme.

In **automatic** mode (default), the app uses the browser geolocation to
compute the real sunrise and sunset for the user's position. The formula is
the standard astronomical one in `lib/sun.ts`, and it runs entirely on the
client. The position is never stored or sent to the backend. It stays in
memory for the calculation only. If geolocation is denied or unavailable,
the app falls back to the system preference (`prefers-color-scheme`)
instead of failing. It recalculates every 10 minutes and when the tab
returns to the foreground.

## UI language (next-intl)

Labels, buttons and messages live in `src/messages/{it,en}.json`, per
namespace (`Nav`, `Auth`, `PostPage`, ...). The language is resolved in
`src/i18n/request.ts`: cookie `notturni_ui_locale` (set by the picker in the
header or by the profile, via the `setUiLocale` server action) →
`Accept-Language` → `it`. The URL has no language segment. The **content**
language (posts, pages) already has its own locale+slug model on the
backend and stays separate. Server Components use `getTranslations()`;
Client Components use `useTranslations()`. To add a language, add a new
JSON file and an entry in `LOCALES` (`src/i18n/config.ts`). Strings not yet
migrated (editor, some tabs and admin pages) are hardcoded in Italian.

## Typography and palette

Defined in `src/app/globals.css` following the project aesthetic guidelines
(see [ROADMAP.md](../ROADMAP.md#2-aesthetics)): serif headings (Lora),
sans-serif body and links (Source Sans 3), and a 5-color palette
(`background`, `foreground`, `primary`, `muted`, `border`). This is the
*platform* palette (dashboard, admin). Each blog can customize its own
palette (see `blogs.getConfig`/`updateConfig` in `lib/api.ts` and the
"Aspetto" (Appearance) tab in the dashboard). The blog palette is applied to
its public pages by `components/blog/BlogPageShell.tsx` as CSS variables.
It applies to the light theme only. In dark theme, the dark platform
palette applies.
