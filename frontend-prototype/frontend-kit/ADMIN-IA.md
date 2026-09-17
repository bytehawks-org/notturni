# Admin information architecture

## Blog admin — `/dashboard/blogs/[slug]` (tabs, role-gated)

| Tab | Route | Who | Content |
| --- | --- | --- | --- |
| Overview | `/` | all members | KPIs (posts, followers, reads 30d, pending comments, newsletter), "Needs your attention" list, reads sparkline (aggregated, no per-reader tracking), activity, storage |
| Posts | `/posts` | author+ | table with status/lang filters, bulk actions, schedule |
| Pages | `/pages` | owner, co-author | static pages (opt-in feature) |
| Publications | `/publications` | owner, co-author | list; `/publications/[id]` chapters drag-to-order + settings |
| Categories & tags | `/taxonomy` | owner, co-author | categories CRUD (one per post), tag merge/rename |
| Notes · Media · Links | `/notes`, `/media`, `/links` | author+ | bibliography assets (mockups 3b, 3c) |
| Comments | `/comments` | owner, mediator | queue Pending/Approved/Hidden/Reported, bulk approve/hide, reply, block author, escalate to platform; policy (registered only / + anonymous pre-moderated / off), notify, auto-close |
| Collaborators | `/collaborators` | owner | roles Author, Co-author, Reviewer, Mediator; invite by username; per-blog alias; pending invites |
| Followers | `/followers` | owner | total (username + alias), anonymous share as a number only, newsletter subscribers, export CSV |
| Appearance | `/appearance` | owner | palette (5), fonts (3, curated), size/measure/layout, live preview |
| Settings | `/settings` | owner | Identity · Visibility (+ indexing) · Languages · Features toggles · Newsletter · Domain (custom CNAME, disabled until routing lands) · Danger zone (transfer, pause, export, delete w/ 30-day grace + slug confirm) |

Visibility of tabs by blog role: Reviewer sees Overview, Posts (review filter). Mediator sees Overview, Comments. Co-author sees everything except Collaborators, Followers, Settings.

## Platform admin — `/admin` (sidebar, platform-role gated)

| Section | Route | Who | Content |
| --- | --- | --- | --- |
| Overview | `/admin` | admin+ | KPIs, open queues with SLA, audit today, services health |
| Users | `/admin/utenti` | admin (roles: super admin) | search, role, activation, MFA status, sessions revoke, GDPR shortcuts; hidden in `solo` |
| All blogs | `/admin/blog` | admin+ | table (owner, visibility, posts, reports), suspend/restore with mandatory audit note, reserved-name check, report detail panel |
| Moderation | `/admin/moderazione` | admin+ | flagged images (model score, threshold), reported posts (hide/show), reported blogs, appeals |
| Comments | `/admin/moderazione-commenti` | moderator+ | escalations from blog mediators |
| Static pages | `/admin/pagine` | admin+ | site pages with RichTextEditor |
| Audit register | `/admin/registro` | admin+ | filterable log, CSV export |
| GDPR requests | `/admin/gdpr` | admin+ | export (automatic) and deletion (second-admin approval, 30-day grace) with legal deadline |
| Platform settings | `/admin/impostazioni` | super admin | Access (mode, registration, SSO providers, MFA for admins) · Blogs & moderation (reserved names, threshold, max blogs, anonymous comments) · Infrastructure (read-only NOCT_* summary) |

Every destructive admin action requires a note that lands in the audit register.
