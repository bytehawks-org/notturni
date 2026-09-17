repo: bytehawks-org/notturni
branch: main
path: frontend/src

## Last sync
date: 2026-09-12T15:50:38Z

### Updated in this project
- Letti README, CLAUDE.md, globals.css, layout dashboard/admin, ui/*, todo/*, app/page.tsx, app/[blogSlug]/page.tsx
- Mockup completi (turni 1–4) in `Notturni Interfaces.dc.html`
- Kit React/Tailwind in `frontend-kit/` (globals.css, shell, ui, blog, bibliography, home, app pages)

## Screen map
| Screen | Repo files |
| --- | --- |
| 1a/1b Post, 3f Blog home | frontend/src/app/[blogSlug]/[postSlug]/page.tsx, frontend/src/app/[blogSlug]/page.tsx, components/FragmentReader.tsx, components/CommentsSection.tsx |
| 1c, 4a/4b Home, 4c Blogs | frontend/src/app/page.tsx, components/FeedPostCard.tsx, components/SiteHeader.tsx |
| 1d Editor | components/editor/RichTextEditor.tsx, app/dashboard/blogs/[slug]/posts/* |
| 1e/1f Dashboard, 2e Settings, 3b/3c/3g | app/dashboard/layout.tsx, app/dashboard/page.tsx, app/dashboard/blogs/[slug]/page.tsx, components/dashboard/blog/* |
| 1g Admin | app/admin/layout.tsx, app/admin/* |
| 1h Auth | app/login/page.tsx, app/register/page.tsx |
| 2a/2b/2c, 3a | app/[blogSlug]/bibliografia|media|link/page.tsx |
| 2d Publication | (nuovo) todo/PUBLICATIONS.md |
| 2f Profile, 3e /u | app/dashboard/profile/page.tsx, app/u/[username]/page.tsx |
| 2g Fragments | app/dashboard/frammenti/page.tsx |
