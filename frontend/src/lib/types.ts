export type PlatformRole = "super_admin" | "amministratore" | "moderatore" | "utente";
export type MfaMethod = "totp" | "email";

export interface CurrentUser {
  id: string;
  username: string;
  email: string;
  mfa_enabled: boolean;
  platform_role: PlatformRole;
}

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

export interface Blog {
  id: string;
  slug: string;
  title: string;
  custom_domain: string | null;
  allow_anonymous_comments: boolean;
  default_locale: string;
  /** Nome pubblico predefinito per i testi scritti su questo blog — vedi Post.author_display_name. */
  default_author_display_name: string | null;
  owner_id: string;
  created_at: string;
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
