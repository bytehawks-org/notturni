# Notturni redesign — inventory & migration guide

Audience: humans and coding agents adapting `bytehawks-org/notturni` (`frontend/src`) to the new design.
Everything referenced here is in this archive. Mockup ids (`1a`, `5f`…) refer to `Notturni Interfaces.dc.html` (open in a browser; newest turn at the top).

## 0. Ground rules (from the repo's own guidelines)
- Palette: 5 colours (`background`, `foreground`, `primary`, `muted`, `border`) + `surface` tint + functional colours. Fonts: max 3 — **Lora** (headings, serif), **Source Sans 3** (body/UI, sans), system mono for meta labels. Both fonts are in the backend's curated lists (`SERIF_FONTS`/`SANS_SERIF_FONTS`).
- Theme: light / dark / auto (sunrise–sunset, client-only) via `data-theme` on `<html>` — unchanged mechanism, new values.
- Styling: Tailwind v4 utility classes reading CSS variables. No component library.
- Copy: interface strings live in `messages/{locale}.json` (next-intl). Content i18n (posts, pages) is untouched.
- Everything below is additive to the existing frontend; existing routes keep their paths (Italian slugs like `/bibliografia`, `/admin/utenti` stay).

## 1. Archive map

| Path | What it is | Replaces / extends in repo |
| --- | --- | --- |
| `Notturni Interfaces.dc.html` | All 35 mockups, desktop + mobile, light + dark. Plain HTML with inline styles: any block can be copied and converted to Tailwind 1:1. | — |
| `HANDOFF.md` | Tokens, recurring Tailwind patterns, mockup → route → component map | — |
| `github.md` | Source repo association + screen map | — |
| `frontend-kit/` | React/Tailwind implementation of the shared pieces (below) | `frontend/src` |

## 2. `frontend-kit/` file by file

### 2.1 Foundations
| File | Purpose | Migration action |
| --- | --- | --- |
| `globals.css` | CSS variables (light/dark), `@theme inline` Tailwind tokens (`bg-surface`, `text-ok`, `shadow-soft`…), reading-column `.notturni-prose` rules, fragment mark, footnotes, sensitive-image blur | Replace the palette/`@theme` block of `src/app/globals.css`; keep the existing `.notturni-prose` table/code/link rules and merge |
| `layout-fonts.tsx` | Font loaders (Lora + Source Sans 3) and the `NextIntlClientProvider` wiring note | Edit `src/app/layout.tsx`: swap Inter→Source_Sans_3, wrap children in the provider, `lang={locale}` |
| `next.config.snippet.ts` | `createNextIntlPlugin("./src/i18n/request.ts")` | Merge into `next.config.ts` |
| `lib/types.additions.ts` | `TrendingTag`, `PublicBlog`, optional `Post` fields (`blog_title`, `author_avatar_url`, `reading_minutes`, `footnote_count`), `getPublicBlogs` helper signature | Append to `src/lib/types.ts` and `src/lib/server-api.ts` |
| `messages/en.json`, `messages/it.json` | 291 interface strings, namespaced per component (`Nav`, `Site`, `Settings`, `AdminBlogs`…). ICU plurals. | Copy to `src/messages/`. New language = new file + entry in `i18n/config.ts` |
| `i18n/config.ts` | `LOCALES = ["en","it"]`, cookie name, type guard | Copy to `src/i18n/` |
| `i18n/request.ts` | UI locale resolution: cookie → platform `default_locale` (`GET /config`) → Accept-Language → `en` | Copy; requires `getPlatformConfig` in server-api |
| `i18n/actions.ts` | Server action `setUiLocale(locale)` (cookie; also PATCH `/users/me { ui_locale }` when signed in) | Copy |
| `I18N.md` | i18n design, default-language rules, backend additions (`platform_config.default_locale`, `users.ui_locale`, `NOCT_DEFAULT_LOCALE`) | Read before touching i18n |
| `ADMIN-IA.md` | Information architecture: blog-admin tabs and platform-admin sections, routes, role gating | Read before building admin routes |
| `README.md` | Short index of the kit | — |

### 2.2 `components/ui/` — primitives (no data fetching, no i18n except noted)
| Component | Props | Mockup | Notes |
| --- | --- | --- | --- |
| `Button` | `variant: primary/secondary/danger/ghost`, `size: sm/md/lg` | everywhere | Replaces `ui/Button.tsx` (same variants + `size`) |
| `Card`, `CardTitle`, `SectionLabel` | `accent?` | everywhere | Replaces `ui/Card.tsx`; `SectionLabel` = mono uppercase eyebrow |
| `Input`, `TextArea`, `Label` (`hint?`), `FieldGroup` | — | forms | Replaces `ui/Field.tsx`; focus ring `border-primary ring-primary/20` |
| `Pill` (`tone`), `TagPill`, `FilterChip` (`active`, `onClick`) | — | status pills, #tags, filter rows | New |
| `EmptyState`, `ErrorState`, `SkeletonRows` | title/body/action; `requestId` | 3d | New; skeleton mirrors row layout, no spinners |
| `Toast`, `ToastStack` | `tone: ok/warn/danger` | 3d | New; fixed bottom-left, caller handles the 5 s timer |
| `ConfirmDialog` | `confirmText` (typed to enable), `confirmLabel` | 3d, 5c | New; destructive confirmations |
| `Toggle`, `SegmentedControl<T>` | controlled | 5c, 5f | New |

### 2.3 `components/shell/` — page frames
| Component | Mockup | Notes |
| --- | --- | --- |
| `DashboardShell` + `AUTHOR_NAV`, `ADMIN_NAV` | 1e, 1f, 1g, 5a–5g | Sidebar 240px ≥`lg`, 5-item bottom tab bar <`lg`. Nav `label` is a `Nav.*` message key. Replaces the header nav in `app/dashboard/layout.tsx` and `app/admin/layout.tsx`; keep their auth/role guards |
| `SiteHeader` (`current`) | 1c, 4a, 4c | Replaces `components/SiteHeader.tsx`; keeps `useAuth` logic |
| `SiteFooter` | 4a | New |
| `BlogHeader` (`slug, name, current, hasPublications, actions`) | 1a, 2a–2c, 3a, 3f | New public blog header with secondary nav; scrollable tabs on mobile |
| `UiLanguagePicker` | header / profile | New; changes interface language only |

### 2.4 `components/blog/` — reading & blog primitives
| Component | Mockup | Notes |
| --- | --- | --- |
| `VisibilityBand`, `VisibilityLabel`, `BlogCard` | 1e, 1f, 5c | Band colours per `todo/BLOG.md` (green / orange / red-black stripes) via `--vis-*` vars |
| `StatusPill` | 1e, 3g | Post status → tone |
| `FootnoteRef`, `Footnotes` | 1a | Markup matches `lib/markdown.ts` output (`.footnote-ref`, `#nota-n`, backref) |
| `FragmentMenu` | 1a, 1b | Desktop floating menu / mobile bottom sheet. Plug into `FragmentReader.tsx`: it owns selection + `highlight-fragments.ts`; this component only renders |

### 2.5 `components/bibliography/`
| Component | Route | Mockup |
| --- | --- | --- |
| `BibliographyList` (`BibEntry[]`) | `/[blogSlug]/bibliografia` | 2a, 3a |
| `MediaGrid` (`editable`, `onOpen`) | `/[blogSlug]/media`, dashboard media library | 2b, 3a, 3c |
| `LinksList` + `groupByHost()` | `/[blogSlug]/link` | 2c, 3a |
| `PublicationIndex`, `ChapterNav`, `ChapterProgress` | `/[blogSlug]/pub/[name]`, `/[chapter]` | 2d, 3g — **new feature** (`todo/PUBLICATIONS.md`), needs backend `publications` + `chapters` |

### 2.6 `components/home/` + `components/FeedPostCard.tsx`
| Component | Mockup | Notes |
| --- | --- | --- |
| `Manifesto` | 4a, 4b | Hero; manifesto text verbatim from current `page.tsx`, pillars translated |
| `TrendingTags` | 4a | Same data as today (`getTrendingTags`), active tag highlighted |
| `BlogDirectoryList`, `BlogDirectoryGrid` | 4a, 4c | Need `getPublicBlogs` (indexed + public only) |
| `FeedPostCard` | 1c, 4a, 4b, 3e | Replaces `components/FeedPostCard.tsx`; tolerant to missing optional fields |

### 2.7 `components/dashboard/blog/` — blog admin
| Component | Mockup | Notes |
| --- | --- | --- |
| `BlogTabs` (`role`, `pendingComments`) | 5a–5c, 5g | Role-gated per `ADMIN-IA.md` |
| `OverviewTab`: `KpiRow`, `AttentionList`, `ReadsChart`, `ActivityList`, `StorageCard` | 5a, 5g | `ReadsChart` takes daily aggregates only (privacy) |
| `CommentsQueue`, `CommentPolicyCard` | 5b | Extends `CommentsTab.tsx`: states, bulk actions, escalation, policy radio |
| `SettingsForm` (`BlogSettings`) | 5c | Extends `SettingsTab.tsx`: identity, visibility, languages, features, domain (disabled), danger zone with `ConfirmDialog` |
| existing `AppearanceTab`, `CollaboratorsTab`, `PostsTab`, `PagesTab` | 2e, 1e | Keep; restyle with the primitives above (mockup 2e shows target) |

### 2.8 `components/admin/` — platform admin
| Component | Mockup | Notes |
| --- | --- | --- |
| `QueuesCard`, `ServicesCard` | 5d | Overview; needs `GET /admin/overview` |
| `BlogsTable`, `ReportPanel` | 5e | Suspend/hide/deactivate/dismiss with **mandatory audit note** |
| `GdprRequestsTable` | 5f | Export automatic; deletion needs second admin (API-enforced) |
| `PlatformSettingsForm` (`PlatformConfig`) | 5f | Super admin; includes `defaultLocale` |

### 2.9 `app/` — page entry points (server components)
| File | Route | Mockup | Data |
| --- | --- | --- | --- |
| `app/page.tsx` | `/` | 4a, 4b | `getPublicFeed`, `getTrendingTags` (existing) + `getPublicBlogs` (new) |
| `app/blogs/page.tsx` | `/blogs` | 4c | `getPublicBlogs` |
| `app/dashboard/blogs/-slug-/layout.tsx` | `/dashboard/blogs/[slug]` | 5a–5c | Header + `BlogTabs`; rename folder to `[slug]` |
| `app/dashboard/blogs/-slug-/page.tsx` | overview tab | 5a | `GET /blogs/{slug}/overview` (new) |
| `app/admin/page.tsx` | `/admin` | 5d | `GET /admin/overview` (new) |

## 3. Mockups without code yet (build from the HTML + primitives)
1d editor chrome (toolbar, right rail, note popover, mention autocomplete) · 1h login/MFA · 2f profile (identity, sign-as, privacy & data) · 2g fragments shelf · 3b notes management · 3c media lightbox · 3e public profile · 3f blog home with custom palette (apply `blog_configs` palette as CSS vars on the page root) · 3g publications management (drag-order) · 1g users table.

## 4. Backend additions implied by the design
- `GET /blogs?public=1&indexed=1` (directory), `GET /blogs/{slug}/overview`, `GET /admin/overview`
- `publications`, `chapters` (order, status) + public routes `/pub/{name}`
- Notes as first-class entities with kind/URL, duplicate detection, "cited in"
- Comment policy `registered | open_moderated | off`, block list, escalation to platform
- Blog `paused` state, ownership transfer, 30-day soft delete
- Platform config: `default_locale`, `registration`, `sso_providers`, `mfa_for_admins`, `reserved_names`, `moderation_threshold`, `max_blogs_per_user`, `anonymous_comments`
- `users.ui_locale`; GDPR request queue with deadlines and second-admin approval
- Audit register entries for every admin action, with note

## 5. Suggested migration order
1. `globals.css` + fonts + `layout.tsx` (theme values, `NextIntlClientProvider`) — whole app changes look instantly.
2. `components/ui/*` replace existing primitives (same names, superset API).
3. `DashboardShell` in dashboard/admin layouts; `SiteHeader`/`SiteFooter`/`FeedPostCard` on public pages; `BlogHeader` on `/[blogSlug]/*`.
4. Home (`app/page.tsx`) and `/blogs`.
5. Blog admin tabs (layout + Overview, Comments, Settings), then platform admin.
6. Publications and notes management (new backend work).
