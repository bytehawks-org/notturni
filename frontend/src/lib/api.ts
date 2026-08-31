import type {
  AdminUser,
  Blog,
  BlogConfig,
  Comment,
  CurrentUser,
  LoginResponse,
  Page,
  Post,
  PostTranslationSummary,
  Profile,
  SessionResponse,
  SocialLink,
} from "./types";

const API_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000";

export class ApiClientError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

interface RequestOptions {
  method?: string;
  body?: unknown;
  token?: string | null;
  formData?: FormData;
}

async function request<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const headers: Record<string, string> = {};
  if (options.token) headers["Authorization"] = `Bearer ${options.token}`;

  let body: BodyInit | undefined;
  if (options.formData) {
    body = options.formData;
  } else if (options.body !== undefined) {
    headers["Content-Type"] = "application/json";
    body = JSON.stringify(options.body);
  }

  const res = await fetch(`${API_URL}${path}`, {
    method: options.method ?? "GET",
    headers,
    body,
  });

  if (res.status === 204) {
    return undefined as T;
  }

  const contentType = res.headers.get("content-type") ?? "";
  const data: unknown = contentType.includes("application/json") ? await res.json() : undefined;

  if (!res.ok) {
    const message =
      data && typeof data === "object" && "detail" in data
        ? String((data as { detail: unknown }).detail)
        : `Errore ${res.status}`;
    throw new ApiClientError(res.status, message);
  }

  return data as T;
}

export const api = {
  auth: {
    register: (payload: { username: string; email: string; password: string }) =>
      request<CurrentUser>("/api/v1/auth/register", { method: "POST", body: payload }),
    login: (payload: { email: string; password: string }) =>
      request<LoginResponse>("/api/v1/auth/login", { method: "POST", body: payload }),
    verifyMfa: (payload: { challenge: string; code: string }) =>
      request<SessionResponse>("/api/v1/auth/mfa/verify", { method: "POST", body: payload }),
    refresh: (refresh_token: string) =>
      request<SessionResponse>("/api/v1/auth/refresh", { method: "POST", body: { refresh_token } }),
    logout: (refresh_token: string) =>
      request<void>("/api/v1/auth/logout", { method: "POST", body: { refresh_token } }),
    me: (token: string) => request<CurrentUser>("/api/v1/auth/me", { token }),
    totpSetup: (token: string) =>
      request<{ secret: string; provisioning_uri: string }>("/api/v1/auth/mfa/totp/setup", {
        method: "POST",
        token,
      }),
    totpConfirm: (token: string, code: string) =>
      request<void>("/api/v1/auth/mfa/totp/confirm", { method: "POST", token, body: { code } }),
    emailSetup: (token: string) =>
      request<void>("/api/v1/auth/mfa/email/setup", { method: "POST", token }),
    emailConfirm: (token: string, code: string) =>
      request<void>("/api/v1/auth/mfa/email/confirm", { method: "POST", token, body: { code } }),
    disableMfa: (token: string) => request<void>("/api/v1/auth/mfa/disable", { method: "POST", token }),
  },

  blogs: {
    listMine: (token: string) => request<Blog[]>("/api/v1/blogs/mine", { token }),
    get: (slug: string) => request<Blog>(`/api/v1/blogs/${slug}`),
    create: (token: string, payload: { slug: string; title: string; default_locale?: string }) =>
      request<Blog>("/api/v1/blogs", { method: "POST", token, body: payload }),
    update: (
      token: string,
      slug: string,
      payload: { title?: string; allow_anonymous_comments?: boolean }
    ) => request<Blog>(`/api/v1/blogs/${slug}`, { method: "PATCH", token, body: payload }),
    getConfig: (slug: string) => request<BlogConfig>(`/api/v1/blogs/${slug}/config`),
    updateConfig: (token: string, slug: string, config: BlogConfig) =>
      request<BlogConfig>(`/api/v1/blogs/${slug}/config`, { method: "PUT", token, body: config }),
    follow: (token: string, slug: string) =>
      request<void>(`/api/v1/blogs/${slug}/follow`, { method: "POST", token }),
    unfollow: (token: string, slug: string) =>
      request<void>(`/api/v1/blogs/${slug}/follow`, { method: "DELETE", token }),
    followers: (slug: string) => request<{ username: string }[]>(`/api/v1/blogs/${slug}/followers`),
    /** Immagine da incorporare nel contenuto o da usare come cover di un post. */
    uploadMedia: (token: string, slug: string, file: File) => {
      const formData = new FormData();
      formData.append("file", file);
      return request<{ url: string }>(`/api/v1/blogs/${slug}/media`, { method: "POST", token, formData });
    },
  },

  posts: {
    /** Pubblico: solo pubblicati. Con token e accesso in scrittura: anche le bozze. */
    list: (token: string | null, blogSlug: string, locale?: string) =>
      request<Post[]>(`/api/v1/blogs/${blogSlug}/posts${locale ? `?locale=${locale}` : ""}`, { token }),
    get: (token: string | null, postId: string) => request<Post>(`/api/v1/posts/${postId}`, { token }),
    /** Risolve il permalink pubblico /{blogSlug}/{date}/{postSlug} (niente UUID nell'URL). */
    getByPermalink: (token: string | null, blogSlug: string, date: string, postSlug: string) =>
      request<Post>(`/api/v1/blogs/${blogSlug}/posts/${date}/${postSlug}`, { token }),
    create: (
      token: string,
      blogSlug: string,
      payload: {
        slug: string;
        title: string;
        content: string;
        author_display_name?: string;
        locale?: string;
        cover_image_url?: string | null;
      }
    ) => request<Post>(`/api/v1/blogs/${blogSlug}/posts`, { method: "POST", token, body: payload }),
    update: (
      token: string,
      postId: string,
      payload: { title?: string; content?: string; cover_image_url?: string | null }
    ) => request<Post>(`/api/v1/posts/${postId}`, { method: "PATCH", token, body: payload }),
    publish: (token: string, postId: string) =>
      request<Post>(`/api/v1/posts/${postId}/publish`, { method: "POST", token }),
    translations: (postId: string) =>
      request<PostTranslationSummary[]>(`/api/v1/posts/${postId}/translations`),
    addTranslation: (
      token: string,
      postId: string,
      payload: {
        slug: string;
        locale: string;
        title: string;
        content: string;
        author_display_name?: string;
        cover_image_url?: string | null;
      }
    ) => request<Post>(`/api/v1/posts/${postId}/translations`, { method: "POST", token, body: payload }),
  },

  comments: {
    listApproved: (postId: string) => request<Comment[]>(`/api/v1/posts/${postId}/comments`),
    listPending: (token: string, postId: string) =>
      request<Comment[]>(`/api/v1/posts/${postId}/comments/pending`, { token }),
    create: (
      token: string | null,
      postId: string,
      payload: { content: string; author_display_name?: string; author_email?: string }
    ) => request<Comment>(`/api/v1/posts/${postId}/comments`, { method: "POST", token, body: payload }),
    approve: (token: string, commentId: string) =>
      request<Comment>(`/api/v1/comments/${commentId}/approve`, { method: "POST", token }),
    reject: (token: string, commentId: string) =>
      request<Comment>(`/api/v1/comments/${commentId}/reject`, { method: "POST", token }),
  },

  pages: {
    /** Pubblico: solo pubblicate. Con token admin: anche le bozze. */
    list: (token: string | null, locale: string) =>
      request<Page[]>(`/api/v1/pages?locale=${locale}`, { token }),
    get: (token: string | null, slug: string, locale: string) =>
      request<Page>(`/api/v1/pages/${slug}?locale=${locale}`, { token }),
    create: (
      token: string,
      payload: { slug: string; locale: string; title: string; content: string; is_published: boolean }
    ) => request<Page>("/api/v1/pages", { method: "POST", token, body: payload }),
    addTranslation: (
      token: string,
      pageId: string,
      payload: { slug: string; locale: string; title: string; content: string; is_published: boolean }
    ) => request<Page>(`/api/v1/pages/${pageId}/translations`, { method: "POST", token, body: payload }),
    update: (
      token: string,
      pageId: string,
      payload: Partial<{ slug: string; title: string; content: string; is_published: boolean }>
    ) => request<Page>(`/api/v1/pages/${pageId}`, { method: "PATCH", token, body: payload }),
  },

  users: {
    profile: (username: string) => request<Profile>(`/api/v1/users/${username}`),
    updateMe: (token: string, payload: { bio?: string }) =>
      request<Profile>("/api/v1/users/me", { method: "PATCH", token, body: payload }),
    uploadAvatar: (token: string, file: File) => {
      const formData = new FormData();
      formData.append("file", file);
      return request<{ avatar_url: string | null }>("/api/v1/users/me/avatar", {
        method: "POST",
        token,
        formData,
      });
    },
    deleteAvatar: (token: string) =>
      request<void>("/api/v1/users/me/avatar", { method: "DELETE", token }),
    addSocialLink: (token: string, payload: { label: string; url: string }) =>
      request<SocialLink>("/api/v1/users/me/social-links", { method: "POST", token, body: payload }),
    deleteSocialLink: (token: string, linkId: string) =>
      request<void>(`/api/v1/users/me/social-links/${linkId}`, { method: "DELETE", token }),
    follow: (token: string, username: string) =>
      request<void>(`/api/v1/users/${username}/follow`, { method: "POST", token }),
    unfollow: (token: string, username: string) =>
      request<void>(`/api/v1/users/${username}/follow`, { method: "DELETE", token }),
    followers: (username: string) =>
      request<{ username: string }[]>(`/api/v1/users/${username}/followers`),
    following: (username: string) =>
      request<{ username: string }[]>(`/api/v1/users/${username}/following`),
  },

  admin: {
    listUsers: (token: string) => request<AdminUser[]>("/api/v1/admin/users", { token }),
    updateUser: (
      token: string,
      userId: string,
      payload: Partial<{ platform_role: string; is_active: boolean }>
    ) => request<AdminUser>(`/api/v1/admin/users/${userId}`, { method: "PATCH", token, body: payload }),
  },
};
