import type { SensitivityCategory } from "./content-media";
import type {
  BibliographyEntry,
  Blog,
  BlogConfig,
  BlogInvitation,
  BlogMember,
  BlogRole,
  BlogVisibility,
  Category,
  Comment,
  CurrentUser,
  FollowStats,
  FragmentCollectionEntry,
  LinkBibliographyEntry,
  LoginResponse,
  MediaBibliographyEntry,
  MembershipBlog,
  Page,
  PageTranslationSummary,
  Post,
  PostAuthorNameStyle,
  PostFragment,
  PostNote,
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
    /** Blog altrui su cui l'utente ha una membership (todo/BLOG.md #3). */
    memberOf: (token: string) => request<MembershipBlog[]>("/api/v1/blogs/member-of", { token }),
    /** Il token è opzionale ma necessario per i blog `members`/`private`. */
    get: (slug: string, token?: string | null) =>
      request<Blog>(`/api/v1/blogs/${slug}`, { token }),
    create: (
      token: string,
      payload: {
        slug: string;
        title: string;
        default_locale?: string;
        subtitle?: string | null;
        description?: string | null;
        visibility?: BlogVisibility;
        default_author_display_name?: string | null;
      }
    ) => request<Blog>("/api/v1/blogs", { method: "POST", token, body: payload }),
    update: (
      token: string,
      slug: string,
      payload: {
        title?: string;
        /** "" azzera; assente non tocca. */
        subtitle?: string;
        description?: string;
        visibility?: BlogVisibility;
        allow_anonymous_comments?: boolean;
        mentions_enabled?: boolean;
        static_pages_enabled?: boolean;
        /** "" azzera (torna allo username di chi scrive); assente non tocca. */
        default_author_display_name?: string;
      }
    ) => request<Blog>(`/api/v1/blogs/${slug}`, { method: "PATCH", token, body: payload }),
    getConfig: (slug: string, token?: string | null) =>
      request<BlogConfig>(`/api/v1/blogs/${slug}/config`, { token }),
    /** Bibliografia automatica del blog: tutte le note dei post pubblicati. */
    bibliography: (slug: string, token?: string | null) =>
      request<BibliographyEntry[]>(`/api/v1/blogs/${slug}/bibliography`, { token }),
    /** CLAUDE.md #4: come sopra, per i media (immagini) citati nei post pubblicati. */
    mediaBibliography: (slug: string, token?: string | null) =>
      request<MediaBibliographyEntry[]>(`/api/v1/blogs/${slug}/media-bibliography`, { token }),
    /** CLAUDE.md #4: come sopra, per i link citati nei post pubblicati. */
    linksBibliography: (slug: string, token?: string | null) =>
      request<LinkBibliographyEntry[]>(`/api/v1/blogs/${slug}/links-bibliography`, { token }),
    /** Suggerimenti per l'autocomplete delle @menzioni nell'editor. */
    mentionableUsers: (token: string, slug: string, q: string) =>
      request<{ username: string; display_name: string | null }[]>(
        `/api/v1/blogs/${slug}/mentionable-users?q=${encodeURIComponent(q)}`,
        { token }
      ),
    updateConfig: (token: string, slug: string, config: BlogConfig) =>
      request<BlogConfig>(`/api/v1/blogs/${slug}/config`, { method: "PUT", token, body: config }),
    follow: (token: string, slug: string) =>
      request<void>(`/api/v1/blogs/${slug}/follow`, { method: "POST", token }),
    unfollow: (token: string, slug: string) =>
      request<void>(`/api/v1/blogs/${slug}/follow`, { method: "DELETE", token }),
    followers: (slug: string) => request<{ username: string }[]>(`/api/v1/blogs/${slug}/followers`),
    /** Immagine da incorporare nel contenuto o da usare come cover di un post.
     * `is_sensitive`: risultato della moderazione automatica (nudità/contenuti
     * sensibili) fatta lato backend al momento dell'upload — vedi API.md. */
    uploadMedia: (token: string, slug: string, file: File) => {
      const formData = new FormData();
      formData.append("file", file);
      return request<{ url: string; is_sensitive: boolean }>(`/api/v1/blogs/${slug}/media`, {
        method: "POST",
        token,
        formData,
      });
    },
    listCategories: (slug: string) => request<Category[]>(`/api/v1/blogs/${slug}/categories`),
    createCategory: (token: string, slug: string, payload: { name: string; slug: string }) =>
      request<Category>(`/api/v1/blogs/${slug}/categories`, { method: "POST", token, body: payload }),
    updateCategory: (
      token: string,
      slug: string,
      categoryId: string,
      payload: { name?: string; slug?: string }
    ) =>
      request<Category>(`/api/v1/blogs/${slug}/categories/${categoryId}`, {
        method: "PATCH",
        token,
        body: payload,
      }),
    deleteCategory: (token: string, slug: string, categoryId: string) =>
      request<void>(`/api/v1/blogs/${slug}/categories/${categoryId}`, { method: "DELETE", token }),

    // --- Pagine statiche del blog: feature opt-in (Blog.static_pages_enabled) ---
    listPages: (slug: string, locale: string, token?: string | null) =>
      request<Page[]>(`/api/v1/blogs/${slug}/pages?locale=${locale}`, { token }),
    getPage: (slug: string, pageSlug: string, locale: string, token?: string | null) =>
      request<Page>(`/api/v1/blogs/${slug}/pages/${pageSlug}?locale=${locale}`, { token }),
    /** Per l'editor di dashboard: recupera per id (bozza inclusa), non per slug/locale. */
    getPageById: (token: string | null, slug: string, pageId: string) =>
      request<Page>(`/api/v1/blogs/${slug}/pages/by-id/${pageId}`, { token }),
    createPage: (
      token: string,
      slug: string,
      payload: { slug: string; locale: string; title: string; content: string; is_published: boolean }
    ) => request<Page>(`/api/v1/blogs/${slug}/pages`, { method: "POST", token, body: payload }),
    addPageTranslation: (
      token: string,
      slug: string,
      pageId: string,
      payload: { slug: string; locale: string; title: string; content: string; is_published: boolean }
    ) =>
      request<Page>(`/api/v1/blogs/${slug}/pages/${pageId}/translations`, {
        method: "POST",
        token,
        body: payload,
      }),
    pageTranslations: (slug: string, pageId: string) =>
      request<PageTranslationSummary[]>(`/api/v1/blogs/${slug}/pages/${pageId}/translations`),
    updatePage: (
      token: string,
      slug: string,
      pageId: string,
      payload: { slug?: string; title?: string; content?: string; is_published?: boolean }
    ) =>
      request<Page>(`/api/v1/blogs/${slug}/pages/${pageId}`, { method: "PATCH", token, body: payload }),
    deletePage: (token: string, slug: string, pageId: string) =>
      request<void>(`/api/v1/blogs/${slug}/pages/${pageId}`, { method: "DELETE", token }),

    // --- Collaboratori: membership e inviti (todo/BLOG.md #3) ---
    members: (token: string, slug: string) =>
      request<BlogMember[]>(`/api/v1/blogs/${slug}/members`, { token }),
    updateMemberRole: (token: string, slug: string, userId: string, role: BlogRole) =>
      request<BlogMember>(`/api/v1/blogs/${slug}/members/${userId}`, {
        method: "PATCH",
        token,
        body: { role },
      }),
    removeMember: (token: string, slug: string, userId: string) =>
      request<void>(`/api/v1/blogs/${slug}/members/${userId}`, { method: "DELETE", token }),
    /** Il collaboratore imposta il proprio alias per questo blog (todo/BLOG.md #4). */
    updateMyMembership: (token: string, slug: string, authorDisplayName: string) =>
      request<MembershipBlog>(`/api/v1/blogs/${slug}/my-membership`, {
        method: "PATCH",
        token,
        body: { author_display_name: authorDisplayName },
      }),
    listInvitations: (token: string, slug: string) =>
      request<BlogInvitation[]>(`/api/v1/blogs/${slug}/invitations`, { token }),
    createInvitation: (token: string, slug: string, username: string, role: BlogRole) =>
      request<BlogInvitation>(`/api/v1/blogs/${slug}/invitations`, {
        method: "POST",
        token,
        body: { username, role },
      }),
    revokeInvitation: (token: string, slug: string, invitationId: string) =>
      request<void>(`/api/v1/blogs/${slug}/invitations/${invitationId}`, {
        method: "DELETE",
        token,
      }),
    /** Inviti a collaborare ricevuti dall'utente corrente, ancora in attesa. */
    receivedInvitations: (token: string) =>
      request<BlogInvitation[]>("/api/v1/blogs/received-invitations", { token }),
    acceptInvitation: (token: string, invitationId: string) =>
      request<BlogInvitation>(`/api/v1/blogs/received-invitations/${invitationId}/accept`, {
        method: "POST",
        token,
      }),
    declineInvitation: (token: string, invitationId: string) =>
      request<BlogInvitation>(`/api/v1/blogs/received-invitations/${invitationId}/decline`, {
        method: "POST",
        token,
      }),
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
        locale?: string;
        cover_image_url?: string | null;
        cover_image_is_sensitive?: boolean;
        cover_image_categories?: SensitivityCategory[];
        tags?: string[];
        category_id?: string | null;
        notes?: PostNote[];
      }
    ) => request<Post>(`/api/v1/blogs/${blogSlug}/posts`, { method: "POST", token, body: payload }),
    update: (
      token: string,
      postId: string,
      payload: {
        title?: string;
        content?: string;
        cover_image_url?: string | null;
        cover_image_is_sensitive?: boolean;
        /** assente: non tocca le categorie; lista (anche []): le sostituisce
         * — indipendente da cover_image_url, a differenza di
         * cover_image_is_sensitive (vedi backend/API.md). */
        cover_image_categories?: SensitivityCategory[];
        tags?: string[];
        /** assente: non tocca la categoria; null: la rimuove; id: la imposta. */
        category_id?: string | null;
        /** assente: non tocca le note; lista (anche []): le sostituisce. */
        notes?: PostNote[];
      }
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
        cover_image_url?: string | null;
        cover_image_categories?: SensitivityCategory[];
        tags?: string[];
        category_id?: string | null;
        notes?: PostNote[];
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

  /** CLAUDE.md #1: anteprima di un link (titolo/descrizione/immagine Open
   * Graph), usata sia dall'editor sia dal rendering pubblico del post per i
   * link salvati come card. Pubblico, nessun token. */
  linkPreview: {
    get: (url: string) =>
      request<{ url: string; title: string | null; description: string | null; image: string | null }>(
        `/api/v1/link-preview?url=${encodeURIComponent(url)}`
      ),
  },

  users: {
    profile: (username: string) => request<Profile>(`/api/v1/users/${username}`),
    updateMe: (
      token: string,
      payload: {
        /** Citabile ovunque come @username; unico, minuscolo (vedi
         * backend/app/domain/usernames.py). Assente non tocca. */
        username?: string;
        bio?: string;
        first_name?: string;
        last_name?: string;
        /** "" azzera l'alias globale; assente non tocca. */
        display_name?: string;
        post_author_name_style?: PostAuthorNameStyle;
        country?: string;
        native_language?: string;
        fallback_languages?: string[];
      }
    ) => request<Profile>("/api/v1/users/me", { method: "PATCH", token, body: payload }),
    followStats: (token: string) => request<FollowStats>("/api/v1/users/me/follow-stats", { token }),
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

  fragments: {
    /** Frammenti già salvati dall'utente corrente su questo post — per
     * ri-evidenziarli ad ogni lettura. */
    listForPost: (token: string, postId: string) =>
      request<PostFragment[]>(`/api/v1/posts/${postId}/fragments`, { token }),
    create: (token: string, postId: string, text: string) =>
      request<PostFragment>(`/api/v1/posts/${postId}/fragments`, {
        method: "POST",
        token,
        body: { text },
      }),
    /** Raccolta unificata di tutti i frammenti salvati dall'utente. */
    listMine: (token: string) => request<FragmentCollectionEntry[]>("/api/v1/users/me/fragments", { token }),
    remove: (token: string, fragmentId: string) =>
      request<void>(`/api/v1/fragments/${fragmentId}`, { method: "DELETE", token }),
  },
};
