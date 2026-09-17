# Notturni UI — handoff notes

Source of truth for the mockups: `Notturni Interfaces.dc.html` (open in any browser; every screen is plain HTML with inline styles, so any block can be copied and converted to Tailwind classes 1:1).

## Tokens (drop into `globals.css`, replaces the current 5 colours)

```css
:root {                     /* light */
  --background: #faf8f4;
  --surface:    #ffffff;    /* cards, sidebars — same hue family as background, counts as a tint not a 6th colour */
  --foreground: #232220;
  --primary:    #3d6b5e;
  --muted:      #857e74;
  --border-color: #e5dfd5;
  --fragment-highlight: rgb(56 189 248 / 22%);
  --fragment-highlight-border: rgb(14 165 233 / 50%);
  --shadow: 0 1px 2px rgb(35 34 32 / 5%), 0 8px 24px rgb(35 34 32 / 6%);
}
:root[data-theme="dark"], @media (prefers-color-scheme: dark) :root:not([data-theme="light"]) {
  --background: #18191b;
  --surface:    #1f2023;
  --foreground: #ebe6de;
  --primary:    #83b8a5;
  --muted:      #968e82;
  --border-color: #2f2e2b;
  --fragment-highlight: rgb(56 189 248 / 18%);
  --fragment-highlight-border: rgb(56 189 248 / 45%);
  --shadow: 0 1px 2px rgb(0 0 0 / 40%), 0 8px 24px rgb(0 0 0 / 35%);
}
@theme inline { --color-surface: var(--surface); /* add next to the existing @theme entries */ }
```

Fonts: headings `Lora` (400/500/600 + italic), body `Source Sans 3` (400/500/600) — both already in the backend's curated lists. Mono for meta labels: system `ui-monospace`. Swap `Inter` → `Source_Sans_3` in `layout.tsx`.

Functional colours (not part of the palette, same rule as the existing fragment highlight):
status Published `#3f8a5f`, Review `#9a6b1a` / `rgba(217,164,65,.16)`, Scheduled `#3d6b8a`, danger `#b3462e`, sensitive `#c9873a`.
Blog visibility band (todo/BLOG.md): public `#3f8a5f`, members `#d9a441`, private `repeating-linear-gradient(135deg,#8b2e2e 0 6px,#232220 6px 12px)`.

## Recurring Tailwind patterns

- Page shell: `bg-background text-foreground font-sans text-[15px] leading-normal antialiased`
- Card: `rounded-xl border border-border bg-surface`
- Primary button: `rounded-lg bg-primary px-3.5 py-2 text-sm font-semibold text-background`
- Secondary: `rounded-lg border border-border px-3.5 py-2 text-sm font-medium`
- Field: `rounded-lg border border-border bg-surface px-3 py-2.5` · focus `border-primary ring-[3px] ring-primary/20`
- Section label: `font-mono text-[11px] uppercase tracking-[.08em] text-muted`
- Status pill: `rounded-full px-2 py-0.5 text-xs font-semibold`
- Tag pill: `rounded-full bg-primary/10 px-2.5 py-1 text-[13px] text-primary`
- Sidebar nav item: `flex justify-between rounded-lg px-3 py-2 text-sm text-muted` · active `bg-primary/10 font-semibold text-foreground`
- Fragment mark: `bg-[var(--fragment-highlight)] shadow-[inset_0_-2px_0_0_var(--fragment-highlight-border)] rounded-[3px]`
- Footnote ref: `<sup class="ml-0.5 text-[.7em] leading-none text-primary cursor-help" title="…">`
- Reading column: `max-w-[680px]`, body `text-lg leading-[1.7]`, h1 `font-serif text-[44px] font-medium leading-[1.12] tracking-tight`

## Layout → route map

| Screen | Route | Notes |
| --- | --- | --- |
| 1a/1b Post | `/[blogSlug]/[postSlug]` | 3-col grid `1fr 680px 1fr` ≥1280; single column <1024 with fragment bottom-sheet |
| 1c Home | `/` | `minmax(0,1fr) 300px`, sidebar collapses below 900 |
| 1d Editor | `/dashboard/blogs/[slug]/posts/*` | sticky toolbar in header; right rail 300px becomes a drawer on mobile |
| 1e/1f Dashboard | `/dashboard` | new `DashboardShell`: 240px sidebar ≥1024, 5-item tab bar below |
| 1g Admin | `/admin/*` | same shell, admin nav set, dark by default optional |
| 1h Auth | `/login`, `/register` | MFA as step 2 in the same page |
| 2a/2b/2c | `/[blogSlug]/bibliografia`, `/media`, `/link` | shared blog header with the new secondary nav |
| 2d Publication | `/[blogSlug]/pub/[name]` | new (todo/PUBLICATIONS.md) |
| 2e Blog settings | `/dashboard/blogs/[slug]` | tabs → Appearance + Collaborators |
| 2f Profile | `/dashboard/profile` | adds Privacy & data (GDPR) block |
| 2g Fragments | `/dashboard/frammenti` | mobile-first list |

| 3a Mobile bib/media/links | same routes | `BlogHeader` tab row + `BibliographyList`/`MediaGrid`/`LinksList` mobile variants |
| 3b Notes management | `/dashboard/blogs/[slug]/notes` | new tab; table + side panel (expanded note, duplicate merge) |
| 3c Media library | `/dashboard/blogs/[slug]/media` | `MediaGrid editable` + lightbox with alt/caption/warning |
| 3d States | — | `EmptyState`, `SkeletonRows`, `ErrorState`, `ConfirmDialog`, `Toast`, suspended-blog notice |
| 3e Public profile | `/u/[username]` | header, tabs Posts/Blogs/Comments, blog card |
| 3f Blog home | `/[blogSlug]` | custom palette via `blog_configs` → CSS vars on the page root |
| 3g Publications mgmt + chapter | `/dashboard/blogs/[slug]/publications/[id]`, `/[blogSlug]/pub/[name]/[chapter]` | drag-to-order rows; `ChapterProgress` + `ChapterNav` |

| 4a/4b Platform home | `/` | `frontend-kit/app/page.tsx`: Manifesto + TrendingTags + FeedPostCard + BlogDirectoryList; same feed/trending/filter data as today's page.tsx |
| 4c Blog directory | `/blogs` | `frontend-kit/app/blogs/page.tsx` + BlogDirectoryGrid (needs `getPublicBlogs`, see `lib/types.additions.ts`) |

| 5a–5c Blog admin | `/dashboard/blogs/[slug]`, `/comments`, `/settings` | `BlogTabs` role-gated; `OverviewTab`, `CommentsQueue`+`CommentPolicyCard`, `SettingsForm` (danger zone + ConfirmDialog) |
| 5d–5f Platform admin | `/admin`, `/admin/blog`, `/admin/gdpr`, `/admin/impostazioni` | `QueuesCard`/`ServicesCard`, `BlogsTable`+`ReportPanel` (mandatory audit note), `GdprRequestsTable`, `PlatformSettingsForm` |
| 5g Mobile admin | same | tab bar via `DashboardShell`; blog tabs scroll horizontally |

Full IA: `frontend-kit/ADMIN-IA.md`.

React/Tailwind implementations of the shared pieces are in `frontend-kit/` (see its README).

Breakpoints used: 390 (mobile), 1024 (sidebar appears), 1280 (design width).
