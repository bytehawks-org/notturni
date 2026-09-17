# Notturni frontend kit

Drop-in for `frontend/src`. Tailwind v4 (`@import "tailwindcss"` + `@theme inline`), Next.js App Router, TypeScript.

- `globals.css` — replaces the palette block in `src/app/globals.css` (adds `--surface`, new fonts, dark values). Keep the existing `.notturni-prose` rules; append the new ones at the end of this file.
- `layout-fonts.tsx` — snippet for `src/app/layout.tsx`: Lora + Source Sans 3.
- `components/ui/*` — primitives: Button, Card, Field, Pill, EmptyState, Skeleton, Toast, ConfirmDialog, Toggle, SegmentedControl.
- `components/shell/*` — DashboardShell (sidebar ≥lg, tab bar <lg), BlogHeader (public blog nav), SiteHeader.
- `components/blog/*` — VisibilityBand, BlogCard, StatusPill, FootnoteRef, FragmentMark, FragmentMenu, LinkPreviewCard.
- `app/page.tsx`, `app/blogs/page.tsx` — platform home and blog directory (server components).
- `components/home/*` — Manifesto, TrendingTags, BlogDirectory (list + grid). `components/shell/SiteHeader|SiteFooter`, `components/FeedPostCard` replace the current ones.
- `lib/types.additions.ts` — new types + the `getPublicBlogs` helper to add to server-api.
- Note: the folder `app/dashboard/blogs/-slug-/` stands for Next's `[slug]` (brackets can't be stored here) — rename on import.
- `I18N.md`, `i18n/*`, `messages/en.json|it.json`, `components/shell/UiLanguagePicker.tsx` — interface i18n with next-intl; default language from install env or Platform settings, per-user override in profile.
- `ADMIN-IA.md` — information architecture for blog admin and platform admin (tabs, routes, roles).
- `app/dashboard/blogs/[slug]/layout.tsx` + `page.tsx`, `components/dashboard/blog/*` — BlogTabs (role-gated), OverviewTab, CommentsTab (queue + policy), SettingsTab (form + danger zone).
- `app/admin/page.tsx`, `components/admin/*` — AdminOverview (queues, services), BlogsTable + ReportPanel, GdprRequestsTable, PlatformSettingsForm.
- `components/bibliography/*` — BibliographyList, MediaGrid, LinksList, PublicationIndex, ChapterNav.

All components read only the 5 palette tokens (+ surface/shadow) via Tailwind classes `bg-background bg-surface text-foreground text-primary text-muted border-border`. No new colours except the functional ones declared in `globals.css` (`--ok --warn --info --danger --sensitive`).

Mockup ↔ component map: see the "Layout → route map" in `../HANDOFF.md`.
