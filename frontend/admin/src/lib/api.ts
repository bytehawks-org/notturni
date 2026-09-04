import type {
  AdminBlog,
  AdminUser,
  CurrentUser,
  InstanceConfig,
  LoginResponse,
  Page,
  PageTranslationSummary,
  PlatformRole,
  SessionResponse,
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
}

async function request<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const headers: Record<string, string> = {};
  if (options.token) headers["Authorization"] = `Bearer ${options.token}`;

  let body: BodyInit | undefined;
  if (options.body !== undefined) {
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

function withQuery(path: string, params: Record<string, string | undefined>): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value) search.set(key, value);
  }
  const qs = search.toString();
  return qs ? `${path}?${qs}` : path;
}

export const api = {
  config: {
    get: () => request<InstanceConfig>("/api/v1/config"),
  },

  auth: {
    login: (payload: { email: string; password: string }) =>
      request<LoginResponse>("/api/v1/auth/login", { method: "POST", body: payload }),
    verifyMfa: (payload: { challenge: string; code: string }) =>
      request<SessionResponse>("/api/v1/auth/mfa/verify", { method: "POST", body: payload }),
    refresh: (refresh_token: string) =>
      request<SessionResponse>("/api/v1/auth/refresh", { method: "POST", body: { refresh_token } }),
    logout: (refresh_token: string) =>
      request<void>("/api/v1/auth/logout", { method: "POST", body: { refresh_token } }),
    me: (token: string) => request<CurrentUser>("/api/v1/auth/me", { token }),
  },

  admin: {
    listUsers: (token: string, q?: string) =>
      request<AdminUser[]>(withQuery("/api/v1/admin/users", { q }), { token }),
    updateUser: (
      token: string,
      userId: string,
      payload: Partial<{ platform_role: PlatformRole; is_active: boolean }>
    ) => request<AdminUser>(`/api/v1/admin/users/${userId}`, { method: "PATCH", token, body: payload }),
    listBlogs: (token: string, q?: string) =>
      request<AdminBlog[]>(withQuery("/api/v1/admin/blogs", { q }), { token }),
    updateBlog: (token: string, blogId: string, payload: { is_suspended: boolean }) =>
      request<AdminBlog>(`/api/v1/admin/blogs/${blogId}`, { method: "PATCH", token, body: payload }),
  },

  pages: {
    list: (token: string | null, locale: string, q?: string) =>
      request<Page[]>(withQuery("/api/v1/pages", { locale, q }), { token }),
    create: (
      token: string,
      payload: { slug: string; locale: string; title: string; content: string; is_published: boolean }
    ) => request<Page>("/api/v1/pages", { method: "POST", token, body: payload }),
    update: (
      token: string,
      pageId: string,
      payload: Partial<{ slug: string; title: string; content: string; is_published: boolean }>
    ) => request<Page>(`/api/v1/pages/${pageId}`, { method: "PATCH", token, body: payload }),
    addTranslation: (
      token: string,
      pageId: string,
      payload: { slug: string; locale: string; title: string; content: string; is_published: boolean }
    ) => request<Page>(`/api/v1/pages/${pageId}/translations`, { method: "POST", token, body: payload }),
    translations: (pageId: string) =>
      request<PageTranslationSummary[]>(`/api/v1/pages/${pageId}/translations`),
  },
};
