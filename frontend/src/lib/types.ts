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

export interface SessionResponse {
  access_token: string;
  refresh_token: string;
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
export const INVITABLE_BLOG_ROLES: { value: Extract<BlogRole, "co_autore" | "mediatore">; label: string }[] = [
  { value: "co_autore", label: "Co-autore" },
  { value: "mediatore", label: "Mediatore" },
];

export const MAX_BLOG_SUBTITLE = 64;
export const MAX_BLOG_DESCRIPTION = 256;

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
  allow_anonymous_comments: boolean;
  /** todo/EDITOR.md: @menzioni nel contenuto trasformate in link (default: true). */
  mentions_enabled: boolean;
  default_locale: string;
  /** Nome pubblico predefinito per i testi scritti su questo blog — vedi Post.author_display_name. */
  default_author_display_name: string | null;
  owner_id: string;
  created_at: string;
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
  typography?: Record<string, string>;
  layout?: string;
  [key: string]: unknown;
}

export type PostStatus = "draft" | "published";

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
  status: PostStatus;
  published_at: string | null;
  created_at: string;
  /** Permalink leggibile /{blog_slug}/{YYYYMMDD}/{slug}, senza UUID. */
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
  citations: BibliographyCitation[];
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

export type CommentStatus = "pending" | "approved" | "rejected";

export interface Comment {
  id: string;
  post_id: string;
  author_id: string | null;
  author_display_name: string;
  status: CommentStatus;
  content: string;
  created_at: string;
}

export interface Page {
  id: string;
  slug: string;
  locale: string;
  translation_group_id: string;
  title: string;
  content: string;
  is_published: boolean;
  created_at: string;
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
}

export interface ApiError {
  detail: string;
}
