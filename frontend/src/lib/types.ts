import type { SensitivityCategory } from "./content-media";

export type PlatformRole = "super_admin" | "amministratore" | "moderatore" | "utente";
export type MfaMethod = "totp" | "email";

export interface CurrentUser {
  id: string;
  username: string;
  email: string;
  /** Alias pubblico globale scelto dall'utente — vedi Profile.display_name. */
  display_name: string | null;
  mfa_enabled: boolean;
  platform_role: PlatformRole;
  /** B6: lingua dell'interfaccia scelta (null = default di piattaforma). */
  ui_locale: string | null;
}

/** todo/USERS.md #2: cosa mostrare come nome autore sui propri post quando il
 * blog non impone un alias. */
export type PostAuthorNameStyle = "username" | "full_name" | "display_name";

export const POST_AUTHOR_NAME_STYLE_LABELS: Record<PostAuthorNameStyle, string> = {
  username: "Username",
  full_name: "Nome e cognome",
  display_name: "Alias del profilo",
};

export const PLATFORM_ADMIN_ROLES: PlatformRole[] = ["super_admin", "amministratore"];
/** Ruoli con accesso al pannello di moderazione commenti trasversale
 * (ROADMAP.md §1): gli admin di piattaforma più il ruolo Moderatore, che
 * non vede invece le altre sezioni di amministrazione. */
export const PLATFORM_MODERATION_ROLES: PlatformRole[] = ["super_admin", "amministratore", "moderatore"];

/** Il refresh token non è più qui: viaggia solo in un cookie httpOnly
 * impostato dal backend (vedi backend/app/api/v1/auth.py), mai leggibile da
 * JS/localStorage — ROADMAP.md "Sessione in localStorage". */
export interface SessionResponse {
  access_token: string;
  token_type: string;
}

export interface MfaRequiredResponse {
  mfa_required: true;
  method: MfaMethod;
  challenge: string;
}

export type LoginResponse = SessionResponse | MfaRequiredResponse;

export function isMfaRequired(res: LoginResponse): res is MfaRequiredResponse {
  return (res as MfaRequiredResponse).mfa_required === true;
}

/** todo/BLOG.md #2. public: tutti; members: solo utenti autenticati; private: diario del solo proprietario. */
export type BlogVisibility = "public" | "members" | "private";

export const BLOG_VISIBILITY_LABELS: Record<BlogVisibility, string> = {
  public: "Pubblico",
  members: "Solo iscritti alla piattaforma",
  private: "Privato (diario)",
};

/** Ruolo utente specifico di un blog (CLAUDE.md #1). */
export type BlogRole = "autore" | "co_autore" | "revisore" | "mediatore";

/** Ruoli assegnabili invitando un collaboratore (todo/BLOG.md #3). */
export const INVITABLE_BLOG_ROLES: Extract<BlogRole, "co_autore" | "mediatore">[] = [
  "co_autore",
  "mediatore",
];

export const MAX_BLOG_SUBTITLE = 64;
export const MAX_BLOG_DESCRIPTION = 256;

/** Chi può commentare (CLAUDE.md #1): default di blog, con eventuale
 * override per singolo post (vedi Post.comments_mode/effective_comments_mode). */
export type CommentsMode = "everyone" | "members" | "closed";

export const COMMENTS_MODES: CommentsMode[] = ["everyone", "members", "closed"];

export interface Blog {
  id: string;
  slug: string;
  title: string;
  /** Sottotitolo breve (max 64 caratteri). */
  subtitle: string | null;
  /** Descrizione breve del blog (max 256 caratteri). */
  description: string | null;
  visibility: BlogVisibility;
  custom_domain: string | null;
  comments_mode: CommentsMode;
  /** todo/EDITOR.md: @menzioni nel contenuto trasformate in link (default: true). */
  mentions_enabled: boolean;
  /** Pagine statiche del blog: feature opt-in, disattiva di default. */
  static_pages_enabled: boolean;
  /** Opt-in per crawler (backend/app/domain/seo.py), attivi di default.
   * Escludere il blog esclude anche tutti i suoi post, indipendentemente da
   * un eventuale override di Post.search_indexing_enabled/ai_crawling_enabled. */
  search_indexing_enabled: boolean;
  ai_crawling_enabled: boolean;
  default_locale: string;
  /** Lingue secondarie del blog (informative), oltre a default_locale. */
  extra_locales: string[];
  /** B4: chiusura automatica dei commenti N giorni dopo la pubblicazione (null = mai). */
  comments_auto_close_days: number | null;
  /** Pausa volontaria del proprietario: i lettori vedono una pagina "in pausa". */
  is_paused: boolean;
  /** Sospensione da parte di un admin di piattaforma. */
  is_suspended: boolean;
  /** Cancellazione con tolleranza: ripristinabile finché il purge (30 giorni) non passa. */
  deleted_at: string | null;
  /** Nome pubblico predefinito per i testi scritti su questo blog — vedi Post.author_display_name. */
  default_author_display_name: string | null;
  /** `null` per chiunque non sia il proprietario stesso (CLAUDE.md #8): non
   * correla un blog che usa un alias con l'id dell'utente reale dietro. */
  owner_id: string | null;
  created_at: string;
}

/** Voce di `GET /blogs` (directory pubblica): blog più i conteggi delle card. */
export interface PublicBlog extends Blog {
  post_count: number;
  follower_count: number;
  last_published_at: string | null;
}

/** `GET /blogs/{slug}/overview` (todo/UX_REDESIGN.md B1): conteggi per la tab Panoramica. */
export interface BlogOverview {
  posts_total: number;
  posts_published: number;
  posts_scheduled: number;
  posts_draft: number;
  posts_in_review: number;
  followers: number;
  members: number;
  pending_comments: number;
  approved_comments: number;
  media: number;
  last_published_at: string | null;
  /** Letture aggregate per giorno UTC, ultimi 30 giorni (giorni vuoti a 0). */
  reads_30d: { day: string; reads: number }[];
  reads_total_30d: number;
  /** Byte su storage (media + backup); `null` se non calcolabile. */
  storage_bytes: number | null;
}

export interface MembershipBlog {
  blog: Blog;
  role: BlogRole;
  /** Alias con cui l'utente firma i post su questo specifico blog. */
  author_display_name: string | null;
}

export interface BlogMember {
  user_id: string;
  username: string;
  role: BlogRole;
  author_display_name: string | null;
  created_at: string;
}

export type BlogInvitationStatus = "pending" | "accepted" | "declined" | "revoked";

export interface BlogInvitation {
  id: string;
  blog_slug: string;
  blog_title: string;
  role: BlogRole;
  status: BlogInvitationStatus;
  invited_username: string;
  invited_by_username: string;
  created_at: string;
  responded_at: string | null;
}

export interface BlogConfig {
  palette?: Record<string, string>;
  /** Variante scura (stessi vincoli della palette), applicata in tema scuro sulle pagine pubbliche del blog. */
  palette_dark?: Record<string, string>;
  typography?: Record<string, string>;
  layout?: string;
  [key: string]: unknown;
}

/** Stessi elenchi curati di `backend/app/domain/blog_config.py` (CLAUDE.md
 * §5 Estetica: titoli in serif, corpo/link in sans-serif) — tenerli in
 * sincronia se cambia uno dei due lati. */
export const SERIF_FONTS = ["Lora", "Merriweather", "Playfair Display", "Source Serif 4", "Crimson Pro"];
export const SANS_SERIF_FONTS = ["Inter", "Nunito Sans", "Work Sans", "Source Sans 3", "Karla"];

/** Stati persistiti dal backend (app/models/post.py). "Pianificato" non è uno
 * stato a sé: è `published` con `published_at` nel futuro — vedi lib/post-status.ts. */
export type PostStatus = "draft" | "pending_review" | "published";

export interface Post {
  id: string;
  blog_id: string;
  author_id: string;
  author_display_name: string;
  locale: string;
  translation_group_id: string;
  title: string;
  slug: string;
  content: string;
  cover_image_url: string | null;
  /** Risultato della moderazione automatica al momento dell'upload — vedi API.md. */
  cover_image_is_sensitive: boolean;
  /** Categorie di avviso scelte manualmente dal modal stile Bluesky (CLAUDE.md #3). */
  cover_image_categories: SensitivityCategory[];
  status: PostStatus;
  published_at: string | null;
  created_at: string;
  /** Permalink leggibile /{blog_slug}/{slug}, senza UUID. */
  blog_slug: string;
  permalink: string;
  /** Se il blog ha le @menzioni attive: il rendering le trasforma in link. */
  mentions_enabled: boolean;
  /** Note a piè di pagina (todo/EDITOR.md), ordinate per `idx`. */
  notes: PostNote[];
  /** Solo i tag del campo dedicato (per ripresentarli in modifica). */
  manual_tags: string[];
  /** Insieme effettivo: manual_tags + hashtag nel testo. Massimo 5 in tutto. */
  tags: string[];
  /** Tassonomia del blog: al più una per post, a differenza dei tag. */
  category: Category | null;
  /** B9: pubblicazione di appartenenza (al più una) e posizione esplicita del capitolo. */
  publication: { id: string; name: string; title: string } | null;
  chapter_order: number | null;
  /** Override di Blog.comments_mode per questo post: `null` eredita dal blog. */
  comments_mode: CommentsMode | null;
  /** Sempre valorizzato: comments_mode se impostato, altrimenti quello del blog. */
  effective_comments_mode: CommentsMode;
  /** Override di Blog.search_indexing_enabled/ai_crawling_enabled per questo
   * post: `null` eredita dal blog (backend/app/domain/seo.py). */
  search_indexing_enabled: boolean | null;
  ai_crawling_enabled: boolean | null;
  /** Sempre valorizzati: tengono già conto del blocco a cascata se il blog
   * stesso è escluso — un override "true" sul post non può riaprirlo. */
  effective_search_indexing_enabled: boolean;
  effective_ai_crawling_enabled: boolean;
}

export interface Category {
  id: string;
  name: string;
  slug: string;
}

/** Nota a piè di pagina di un post: testo Markdown inline + numero (1-based).
 * Nel corpo del post il riferimento è il marcatore `[idx](#nota-idx)`. */
export interface PostNote {
  idx: number;
  content: string;
}

export const MAX_NOTE_LENGTH = 2000;

export interface BibliographyCitation {
  post_title: string;
  post_slug: string;
  permalink: string;
  locale: string;
  idx: number;
}

export interface BibliographyEntry {
  content: string;
  /** B8: tipo e URL dalla libreria note (null per voci legacy). */
  kind: NoteKind | null;
  url: string | null;
  citations: BibliographyCitation[];
}

export type NoteKind = "book" | "article" | "web" | "note";

/** Pubblicazioni (B9, mockup 2d/3g). */
export interface Publication {
  id: string;
  name: string;
  title: string;
  description: string | null;
  chapters_total: number;
  chapters_published: number;
  created_at: string;
}

export interface Chapter {
  n: number;
  post_id: string;
  slug: string;
  title: string;
  locale: string;
  status: PostStatus;
  published_at: string | null;
  permalink: string;
  reading_minutes: number;
  is_public: boolean;
}

export interface PublicationDetail extends Publication {
  chapters: Chapter[];
}

/** Libreria note del blog (B8, mockup 3b). */
export interface NoteUsage {
  post_id: string;
  post_slug: string;
  post_title: string;
  idx: number;
}

export interface BlogNote {
  id: string;
  content: string;
  kind: NoteKind;
  url: string | null;
  created_at: string;
  updated_at: string;
  used_in: NoteUsage[];
  possible_duplicates: string[];
}

/** CLAUDE.md #4: come BibliographyCitation, ma con la data di pubblicazione
 * invece del numero della nota — usata da media e link. */
export interface ContentCitation {
  post_title: string;
  post_slug: string;
  permalink: string;
  locale: string;
  used_at: string | null;
}

export interface MediaBibliographyEntry {
  url: string;
  alt_text: string;
  categories: SensitivityCategory[];
  citations: ContentCitation[];
}

export interface LinkBibliographyEntry {
  url: string;
  link_text: string;
  citations: ContentCitation[];
}

export const MAX_TAGS_PER_POST = 5;
export const MAX_FALLBACK_LANGUAGES = 5;

export interface TrendingTag {
  tag: string;
  post_count: number;
}

export interface PostTranslationSummary {
  id: string;
  locale: string;
  slug: string;
  status: PostStatus;
}

export interface PageTranslationSummary {
  id: string;
  locale: string;
  slug: string;
  is_published: boolean;
}

export type CommentStatus = "pending" | "approved" | "rejected";

export interface Comment {
  id: string;
  post_id: string;
  /** Risposta a un altro commento dello stesso post; `null` per un commento
   * di primo livello. */
  parent_id: string | null;
  author_id: string | null;
  author_display_name: string;
  status: CommentStatus;
  content: string;
  created_at: string;
  /** B4: segnalato ai moderatori di piattaforma dal proprietario/mediatore. */
  reported_to_platform: boolean;
  report_note: string | null;
}

/** B4: voce della lista dei bloccati di un blog. */
export interface BlockedAuthor {
  id: string;
  label: string;
  is_anonymous: boolean;
  note: string | null;
  created_at: string;
}

/** Libreria media del blog (B7, mockup 3c). */
export interface MediaUsage {
  post_id: string;
  post_slug: string;
  post_title: string;
  permalink: string;
}

export interface MediaFile {
  id: string;
  url: string;
  content_type: string;
  size_bytes: number;
  alt_text: string;
  caption: string | null;
  categories: SensitivityCategory[];
  is_sensitive: boolean;
  uploader_username: string | null;
  created_at: string;
  used_in: MediaUsage[];
}

export interface MediaLibrary {
  items: MediaFile[];
  total_bytes: number;
}

/** GET /blogs/{slug}/comments — moderazione trasversale nel dashboard:
 * commenti di tutti i post del blog, ciascuno con titolo/slug del post. */
export interface BlogComment extends Comment {
  post_title: string;
  post_slug: string;
}

/** Solo per il proprietario (GET /users/me/follow-stats, CLAUDE.md #8):
 * l'unico posto dove identità reale e alias di blog compaiono insieme, per
 * sapere quante persone lo seguono in tutto sotto qualunque identità. */
export interface BlogFollowerCount {
  blog_slug: string;
  blog_title: string;
  alias: string | null;
  followers: number;
}

export interface FollowStats {
  user_followers: number;
  blogs: BlogFollowerCount[];
  total_followers: number;
}

export interface Page {
  id: string;
  /** `null` = pagina del sito principale; valorizzato = pagina di un blog. */
  blog_id: string | null;
  slug: string;
  locale: string;
  translation_group_id: string;
  title: string;
  content: string;
  is_published: boolean;
  created_at: string;
  /** Permalink pubblico: `/p/{slug}` (piattaforma) o `/{blog_slug}/pagina/{slug}` (blog). */
  permalink: string | null;
  /** Mirror di Blog.mentions_enabled (sempre true per le pagine di piattaforma). */
  mentions_enabled: boolean;
}

export interface SocialLink {
  id: string;
  /** Chiave di piattaforma (vedi lib/social-platforms.tsx), non più un'etichetta libera. */
  label: string;
  url: string;
  position: number;
}

export interface Profile {
  username: string;
  bio: string | null;
  first_name: string | null;
  last_name: string | null;
  /** Alias pubblico globale, alternativo a username / nome e cognome. */
  display_name: string | null;
  /** Cosa mostrare come nome autore sui propri post (se il blog non impone un alias). */
  post_author_name_style: PostAuthorNameStyle;
  country: string | null;
  native_language: string | null;
  fallback_languages: string[];
  avatar_url: string | null;
  social_links: SocialLink[];
  created_at: string;
}

export interface AdminUser {
  id: string;
  username: string;
  email: string;
  platform_role: PlatformRole;
  is_active: boolean;
  mfa_enabled: boolean;
  created_at: string;
  /** Blog di proprietà. */
  blogs_count: number;
  /** Ultimo uso di una sessione di refresh; `null` se mai usata. */
  last_seen_at: string | null;
}

export interface ServiceStatus {
  name: string;
  status: "ok" | "down" | "unconfigured";
  detail?: string | null;
}

/** `GET /admin/overview` (mockup 5d). */
export interface AdminOverview {
  users_total: number;
  users_new_7d: number;
  blogs_total: number;
  blogs_suspended: number;
  posts_published: number;
  queue_pending_comments: number;
  queue_posts_in_review: number;
  queue_hidden_posts: number;
  queue_open_reports: number;
  audit_today: number;
  services: ServiceStatus[];
  deployment_mode: "solo" | "platform";
}

/** `GET /users/{username}/comments`: commenti approvati firmati con lo username. */
export interface PublicComment {
  id: string;
  content: string;
  created_at: string;
  post_title: string;
  permalink: string;
}

/** Elenco di piattaforma (dashboard/blog, riservato ad Amministratore/Super
 * Admin) — distinto da `Blog`, che include tutti i campi di gestione del
 * proprio blog non necessari qui. */
export interface AdminBlog {
  id: string;
  slug: string;
  title: string;
  owner_username: string;
  visibility: BlogVisibility;
  is_suspended: boolean;
  is_paused: boolean;
  deleted_at: string | null;
  created_at: string;
  posts_count: number;
  reports_open: number;
}

export type ReportReason = "spam" | "abuse" | "illegal" | "other";
export type BlogAdminAction = "suspend" | "restore" | "hide_reported_posts" | "deactivate_owner" | "dismiss";

/** Segnalazione di un lettore (B5), come vista dal pannello admin. */
export interface ReportDetail {
  id: string;
  target_type: "blog" | "post";
  target_id: string;
  post_slug: string | null;
  post_title: string | null;
  reason: ReportReason;
  note: string | null;
  reporter_username: string;
  created_at: string;
}

export interface BlogReports {
  blog: AdminBlog;
  owner_mfa_enabled: boolean;
  owner_email_domain: string;
  reports: ReportDetail[];
}

/** Elenco di piattaforma (admin/moderazione, riservato ad
 * Amministratore/Super Admin) — a differenza di `Post`, include i tre stati
 * possibili (`PostStatus` sopra ne definisce solo due, per l'uso corrente
 * negli altri punti dell'app) e i soli campi utili a moderare, non l'intero
 * contenuto del post. */
export interface AdminPost {
  id: string;
  title: string;
  slug: string;
  blog_slug: string;
  blog_title: string;
  author_username: string;
  status: "draft" | "pending_review" | "published";
  is_hidden: boolean;
  published_at: string | null;
  created_at: string;
  reports_open: number;
}

export const ADMIN_POST_STATUS_LABELS: Record<AdminPost["status"], string> = {
  draft: "Bozza",
  pending_review: "In revisione",
  published: "Pubblicato",
};

/** GET /admin/comments (admin/moderazione-commenti, ROADMAP.md §1):
 * come `BlogComment`, ma su tutti i blog della piattaforma — riservato ad
 * Amministratore/Super Admin/Moderatore, non solo a proprietario/mediatore
 * del singolo blog. */
export interface AdminComment extends BlogComment {
  blog_id: string;
  blog_slug: string;
  blog_title: string;
  reported_at: string | null;
}

export type AuditActorType = "user" | "core_token" | "user_token" | "system" | "anonymous";
/** Canale da cui è partita l'azione, calcolato server-side da `actor_type`
 * (nessuna colonna dedicata, vedi `backend/app/api/v1/admin.py`). */
export type AuditChannel = "web" | "api" | "system";

/** `GET /api/v1/admin/audit-log` (admin/registro). Registro append-only
 * delle azioni sensibili; solo gli eventi ancora nel database (quelli oltre
 * la retention sono archiviati su storage). */
export interface AuditLogEntry {
  id: string;
  occurred_at: string;
  actor_type: AuditActorType;
  channel: AuditChannel;
  actor_id: string | null;
  actor_label: string | null;
  action: string;
  target_type: string | null;
  target_id: string | null;
  blog_id: string | null;
  ip: string | null;
  user_agent: string | null;
  payload: Record<string, unknown>;
}

/** Azioni note del registro di audit (`backend/app/domain/audit.py` e
 * chiamanti): solo gli identificativi, l'etichetta è in
 * `messages/{it,en}.json` → `AdminAuditLog.action.<entità>.<evento>`
 * (navigazione a percorso annidato sullo stesso punto dell'id). Un'azione
 * non elencata qui (o senza etichetta tradotta) resta mostrata per intero —
 * vedi `admin/registro/page.tsx::actionLabel`. */
export const AUDIT_ACTIONS: string[] = [
  "auth.login",
  "auth.login_failed",
  "user.role_change",
  "user.activated",
  "user.deactivated",
  "user.account_deleted",
  "blog.suspended",
  "blog.unsuspended",
  "blog.invitation_created",
  "blog.invitation_accepted",
  "blog.invitation_declined",
  "blog.invitation_revoked",
  "blog.member_role_changed",
  "blog.member_removed",
  "post.hidden",
  "post.unhidden",
  "comment.approved",
  "comment.rejected",
  "api_token.created",
  "api_token.revoked",
  "page.created",
  "page.updated",
  "page.deleted",
];

export const AUDIT_CHANNELS: AuditChannel[] = ["web", "api", "system"];

/** `/api/v1/tokens` (dashboard/token). Non include mai il valore in chiaro né
 * l'hash: quello arriva solo nella risposta di creazione (`token`), una
 * sola volta. */
export interface ApiToken {
  id: string;
  name: string;
  token_prefix: string;
  owner_type: "core" | "user";
  created_at: string;
  last_used_at: string | null;
  expires_at: string | null;
  revoked_at: string | null;
}

export interface ApiTokenCreated {
  id: string;
  name: string;
  token: string;
  token_prefix: string;
}

/** `GET /api/v1/config`, pubblico: per sapere se nascondere le sezioni
 * multi-utente (admin/utenti) in modalità "solo" senza dover già avere
 * una sessione. */
export interface InstanceConfig {
  deployment_mode: "solo" | "platform";
  /** Site key pubblica di Cloudflare Turnstile (mai la secret key): `null`
   * se l'istanza non ha il captcha configurato — in quel caso i commenti
   * aperti a tutti non sono selezionabili (vedi CommentsMode). */
  turnstile_site_key: string | null;
  /** B6: valori pubblici di platform_config. */
  default_locale: string;
  registration_mode: "open" | "invite" | "closed";
  sso_providers: string[];
}

/** `GET/PATCH /admin/config` (B6, mockup 5f), solo super admin. */
export interface PlatformConfig {
  default_locale: string;
  registration_mode: "open" | "invite" | "closed";
  sso_providers: string[];
  sso_configured: string[];
  mfa_required_for_admins: boolean;
  reserved_blog_names: string[];
  reserved_builtin: string[];
  moderation_threshold: number;
  max_blogs_per_user: number;
  anonymous_comments_allowed: boolean;
  updated_at: string | null;
  infrastructure: Record<string, string | boolean | null>;
}

export type GdprRequestType = "export" | "deletion";
export type GdprRequestStatus = "open" | "approved" | "completed" | "rejected";

export interface GdprRequest {
  id: string;
  username: string;
  type: GdprRequestType;
  status: GdprRequestStatus;
  deadline_at: string;
  note: string | null;
  created_by_username: string | null;
  approved_by_username: string | null;
  approved_at: string | null;
  completed_at: string | null;
  created_at: string;
}

export interface ApiError {
  detail: string;
}

/** Porzione di testo evidenziata e salvata da un lettore su un post
 * pubblicato — raccolta unificata in /dashboard/frammenti. Non supera mai il
 * 15% del testo del post (vinto lato client alla selezione, ricontrollato
 * lato server). */
export interface PostFragment {
  id: string;
  post_id: string;
  text: string;
  /** Scelto al salvataggio, modificabile ex-post: pubblico = visibile ad
   * altri utenti iscritti alla piattaforma, mai a visitatori anonimi. */
  is_public: boolean;
  created_at: string;
}

export const MAX_FRAGMENT_RATIO = 0.15;

export interface FragmentCollectionEntry {
  id: string;
  text: string;
  is_public: boolean;
  created_at: string;
  post_title: string;
  author_display_name: string;
  /** Permalink pubblico /{blog}/{slug} del post di provenienza. */
  permalink: string;
}
