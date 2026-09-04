// Sottoinsieme di frontend/src/lib/types.ts: solo i tipi usati dall'app
// admin (auth, utenti, pagine statiche, blog di piattaforma). Duplicato
// deliberatamente — frontend/admin/ è un progetto Next.js a sé, come
// backend/frontend/moderation non condividono codice tra loro.

export type PlatformRole = "super_admin" | "amministratore" | "moderatore" | "utente";
export type MfaMethod = "totp" | "email";

export interface CurrentUser {
  id: string;
  username: string;
  email: string;
  display_name: string | null;
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

export interface AdminUser {
  id: string;
  username: string;
  email: string;
  platform_role: PlatformRole;
  is_active: boolean;
  mfa_enabled: boolean;
  created_at: string;
}

export type BlogVisibility = "public" | "members" | "private";

export const BLOG_VISIBILITY_LABELS: Record<BlogVisibility, string> = {
  public: "Pubblico",
  members: "Solo iscritti alla piattaforma",
  private: "Privato (diario)",
};

export interface AdminBlog {
  id: string;
  slug: string;
  title: string;
  owner_username: string;
  visibility: BlogVisibility;
  is_suspended: boolean;
  created_at: string;
}

export interface Page {
  id: string;
  blog_id: string | null;
  slug: string;
  locale: string;
  translation_group_id: string;
  title: string;
  content: string;
  is_published: boolean;
  created_at: string;
  permalink: string | null;
  mentions_enabled: boolean;
}

export interface PageTranslationSummary {
  id: string;
  locale: string;
  slug: string;
  is_published: boolean;
}

export interface InstanceConfig {
  deployment_mode: "solo" | "platform";
}

export interface ApiError {
  detail: string;
}
