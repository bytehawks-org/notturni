import type { SensitivityCategory } from "./content-media";
import type {
  AdminBlog,
  AdminComment,
  AdminOverview,
  AdminPost,
  AdminUser,
  ApiToken,
  ApiTokenCreated,
  AuditChannel,
  AuditLogEntry,
  BibliographyEntry,
  BlockedAuthor,
  Blog,
  BlogAdminAction,
  BlogComment,
  BlogConfig,
  BlogInvitation,
  BlogMember,
  BlogNote,
  BlogOverview,
  BlogReports,
  BlogRole,
  BlogVisibility,
  Category,
  Comment,
  CommentStatus,
  CommentsMode,
  CurrentUser,
  FollowStats,
  FragmentCollectionEntry,
  GdprRequest,
  GdprRequestStatus,
  GdprRequestType,
  InstanceConfig,
  Interest,
  LinkBibliographyEntry,
  LoginResponse,
  MediaBibliographyEntry,
  MediaFile,
  MediaLibrary,
  MembershipBlog,
  NewsletterCampaign,
  NewsletterStats,
  NoteKind,
  Page,
  PageTranslationSummary,
  DomainOut,
  MeProfile,
  PlatformConfig,
  PlatformRole,
  Post,
  PostAuthorNameStyle,
  PostFragment,
  PostNote,
  PostTranslationSummary,
  Profile,
  PublicComment,
  Publication,
  PublicationDetail,
  ReportReason,
  SessionResponse,
  SocialLink,
} from "./types";

export const API_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000";

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
  /** Solo per refresh/logout: cookie di sessione, mai per le altre richieste
   * (autenticate via header Authorization con l'access token in memoria). */
  withCredentials?: boolean;
}

const CSRF_COOKIE_NAME = "noct_csrf_token";

/** Letto dal cookie (non httpOnly per costruzione, pattern double-submit —
 * vedi backend/app/api/v1/auth.py) e riecheggiato come header per le sole
 * richieste autenticate dal cookie di refresh. */
function readCsrfCookie(): string | null {
  if (typeof document === "undefined") return null;
  const match = document.cookie.match(new RegExp(`(?:^|; )${CSRF_COOKIE_NAME}=([^;]*)`));
  return match ? decodeURIComponent(match[1]) : null;
}

async function request<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const headers: Record<string, string> = {};
  if (options.token) headers["Authorization"] = `Bearer ${options.token}`;
  if (options.withCredentials) {
    const csrf = readCsrfCookie();
    if (csrf) headers["X-CSRF-Token"] = csrf;
  }

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
    credentials: options.withCredentials ? "include" : "same-origin",
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
  auth: {
    register: (payload: { username: string; email: string; password: string }) =>
      request<CurrentUser>("/api/v1/auth/register", { method: "POST", body: payload }),
    login: (payload: { email: string; password: string }) =>
      request<LoginResponse>("/api/v1/auth/login", { method: "POST", body: payload, withCredentials: true }),
    verifyMfa: (payload: { challenge: string; code: string }) =>
      request<SessionResponse>("/api/v1/auth/mfa/verify", {
        method: "POST",
        body: payload,
        withCredentials: true,
      }),
    /** Nessun refresh_token da passare: viaggia nel cookie httpOnly, inviato
     * automaticamente dal browser. */
    refresh: () => request<SessionResponse>("/api/v1/auth/refresh", { method: "POST", withCredentials: true }),
    logout: () => request<void>("/api/v1/auth/logout", { method: "POST", withCredentials: true }),
    me: (token: string) => request<CurrentUser>("/api/v1/auth/me", { token }),
    totpSetup: (token: string) =>
      request<{ secret: string; provisioning_uri: string; qr_code_data_uri: string }>(
        "/api/v1/auth/mfa/totp/setup",
        { method: "POST", token }
      ),
    totpConfirm: (token: string, code: string) =>
      request<void>("/api/v1/auth/mfa/totp/confirm", { method: "POST", token, body: { code } }),
    emailSetup: (token: string) =>
      request<void>("/api/v1/auth/mfa/email/setup", { method: "POST", token }),
    emailConfirm: (token: string, code: string) =>
      request<void>("/api/v1/auth/mfa/email/confirm", { method: "POST", token, body: { code } }),
    disableMfa: (token: string) => request<void>("/api/v1/auth/mfa/disable", { method: "POST", token }),
    forgotPassword: (payload: { email: string }) =>
      request<void>("/api/v1/auth/password/forgot", { method: "POST", body: payload }),
    resetPassword: (payload: { email: string; code: string; new_password: string }) =>
      request<void>("/api/v1/auth/password/reset", { method: "POST", body: payload }),
  },

  interests: {
    /** Pubblico, nessun token — elenco di interessi correnti (blocco
     * "interessi utente"): usato dal selettore del profilo e dalla
     * directory utenti, entrambi client component. */
    list: () => request<Interest[]>("/api/v1/interests"),
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
        is_paused?: boolean;
        extra_locales?: string[];
        comments_auto_close_days?: number | null;
        title?: string;
        /** "" azzera; assente non tocca. */
        subtitle?: string;
        description?: string;
        visibility?: BlogVisibility;
        comments_mode?: CommentsMode;
        mentions_enabled?: boolean;
        static_pages_enabled?: boolean;
        search_indexing_enabled?: boolean;
        ai_crawling_enabled?: boolean;
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
      request<{ username: string; display_name: string | null; avatar_url: string | null }[]>(
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
    overview: (token: string, slug: string) => request<BlogOverview>(`/api/v1/blogs/${slug}/overview`, { token }),
    /** B3 (danger zone): trasferimento a un coautore, cancellazione con tolleranza, ripristino. */
    transfer: (token: string, slug: string, username: string) =>
      request<Blog>(`/api/v1/blogs/${slug}/transfer`, { method: "POST", token, body: { username } }),
    softDelete: (token: string, slug: string, confirmSlug: string) =>
      request<Blog>(`/api/v1/blogs/${slug}`, { method: "DELETE", token, body: { confirm_slug: confirmSlug } }),
    restore: (token: string, slug: string) => request<Blog>(`/api/v1/blogs/${slug}/restore`, { method: "POST", token }),
    /** URL dell'export ZIP (solo proprietario): scaricato con fetch + blob, vedi SettingsTab. */
    exportUrl: (slug: string) => `${API_URL}/api/v1/blogs/${slug}/export`,
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
    /** Immagine di copertina del blog (banner della home pubblica, facoltativa):
     * stessa moderazione automatica di uploadMedia. */
    uploadCoverImage: (token: string, slug: string, file: File) => {
      const formData = new FormData();
      formData.append("file", file);
      return request<Blog>(`/api/v1/blogs/${slug}/cover-image`, { method: "POST", token, formData });
    },
    /** Avviso manuale sui contenuti della cover già caricata, senza ricaricarla. */
    updateCoverImageCategories: (token: string, slug: string, categories: string[]) =>
      request<Blog>(`/api/v1/blogs/${slug}/cover-image`, { method: "PATCH", token, body: { categories } }),
    deleteCoverImage: (token: string, slug: string) =>
      request<Blog>(`/api/v1/blogs/${slug}/cover-image`, { method: "DELETE", token }),
    /** Favicon dedicata del blog (facoltativa): nessuna moderazione, icona di identità. */
    uploadFavicon: (token: string, slug: string, file: File) => {
      const formData = new FormData();
      formData.append("file", file);
      return request<Blog>(`/api/v1/blogs/${slug}/favicon`, { method: "POST", token, formData });
    },
    deleteFavicon: (token: string, slug: string) =>
      request<Blog>(`/api/v1/blogs/${slug}/favicon`, { method: "DELETE", token }),
    /** B9: pubblicazioni. */
    listPublications: (slug: string, token?: string | null) => request<Publication[]>(`/api/v1/blogs/${slug}/publications`, { token }),
    getPublication: (slug: string, ref: string, token?: string | null) =>
      request<PublicationDetail>(`/api/v1/blogs/${slug}/publications/${ref}`, { token }),
    createPublication: (token: string, slug: string, payload: { name: string; title: string; description?: string | null }) =>
      request<Publication>(`/api/v1/blogs/${slug}/publications`, { method: "POST", token, body: payload }),
    updatePublication: (token: string, slug: string, ref: string, payload: { name?: string; title?: string; description?: string | null }) =>
      request<Publication>(`/api/v1/blogs/${slug}/publications/${ref}`, { method: "PATCH", token, body: payload }),
    deletePublication: (token: string, slug: string, ref: string) =>
      request<void>(`/api/v1/blogs/${slug}/publications/${ref}`, { method: "DELETE", token }),
    orderPublication: (token: string, slug: string, ref: string, postIds: string[]) =>
      request<PublicationDetail>(`/api/v1/blogs/${slug}/publications/${ref}/order`, { method: "PUT", token, body: { post_ids: postIds } }),
    /** B8: libreria note (proprietario e collaboratori). */
    listNotes: (token: string, slug: string, q?: string) => request<BlogNote[]>(withQuery(`/api/v1/blogs/${slug}/notes`, { q }), { token }),
    createNote: (token: string, slug: string, payload: { content: string; kind: NoteKind; url?: string | null }) =>
      request<BlogNote>(`/api/v1/blogs/${slug}/notes`, { method: "POST", token, body: payload }),
    updateNote: (token: string, slug: string, noteId: string, payload: { content?: string; kind?: NoteKind; url?: string | null }) =>
      request<BlogNote>(`/api/v1/blogs/${slug}/notes/${noteId}`, { method: "PATCH", token, body: payload }),
    deleteNote: (token: string, slug: string, noteId: string) => request<void>(`/api/v1/blogs/${slug}/notes/${noteId}`, { method: "DELETE", token }),
    mergeNote: (token: string, slug: string, noteId: string, intoId: string) =>
      request<BlogNote>(`/api/v1/blogs/${slug}/notes/${noteId}/merge`, { method: "POST", token, body: { into_id: intoId } }),
    importNotes: (token: string, slug: string, bibtex: string) =>
      request<BlogNote[]>(`/api/v1/blogs/${slug}/notes/import`, { method: "POST", token, body: { bibtex } }),
    notesExportUrl: (slug: string) => `${API_URL}/api/v1/blogs/${slug}/notes/export.bib`,
    /** B7: libreria media (proprietario e collaboratori). */
    mediaLibrary: (token: string, slug: string) => request<MediaLibrary>(`/api/v1/blogs/${slug}/media`, { token }),
    syncMediaLibrary: (token: string, slug: string) => request<MediaLibrary>(`/api/v1/blogs/${slug}/media/sync`, { method: "POST", token }),
    updateMedia: (token: string, slug: string, mediaId: string, payload: { alt_text?: string; caption?: string; categories?: SensitivityCategory[] }) =>
      request<MediaFile>(`/api/v1/blogs/${slug}/media/${mediaId}`, { method: "PATCH", token, body: payload }),
    deleteMedia: (token: string, slug: string, mediaId: string) =>
      request<void>(`/api/v1/blogs/${slug}/media/${mediaId}`, { method: "DELETE", token }),
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
    /** Risolve il permalink pubblico /{blogSlug}/{postSlug} (niente UUID nell'URL). */
    getByPermalink: (token: string | null, blogSlug: string, postSlug: string) =>
      request<Post>(`/api/v1/blogs/${blogSlug}/posts/${postSlug}`, { token }),
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
        publication_id?: string | null;
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
        /** B9: stesso schema di category_id per la pubblicazione. */
        publication_id?: string | null;
        /** assente: non tocca le note; lista (anche []): le sostituisce. */
        notes?: PostNote[];
        /** assente: non tocca; null: torna a ereditare da Blog.comments_mode;
         * valore: imposta un override per questo solo post. */
        comments_mode?: CommentsMode | null;
        /** assente: non tocca; null: torna a ereditare da Blog.search_indexing_enabled/
         * ai_crawling_enabled; valore: imposta un override per questo solo post. */
        search_indexing_enabled?: boolean | null;
        ai_crawling_enabled?: boolean | null;
      }
    ) => request<Post>(`/api/v1/posts/${postId}`, { method: "PATCH", token, body: payload }),
    /** Pubblica subito, o pianifica se `publishedAt` (ISO) è nel futuro. */
    publish: (token: string, postId: string, publishedAt?: string) =>
      request<Post>(`/api/v1/posts/${postId}/publish`, {
        method: "POST",
        token,
        body: publishedAt ? { published_at: publishedAt } : undefined,
      }),
    /** Conteggio lettura aggregato (B2): pubblico, nessun dato del lettore. */
    recordRead: (postId: string) => request<void>(`/api/v1/posts/${postId}/read`, { method: "POST" }),
    submitForReview: (token: string, postId: string) =>
      request<Post>(`/api/v1/posts/${postId}/submit-for-review`, { method: "POST", token }),
    returnToDraft: (token: string, postId: string) =>
      request<Post>(`/api/v1/posts/${postId}/return-to-draft`, { method: "POST", token }),
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
    /** Moderazione trasversale: commenti di tutti i post del blog in una sola
     * richiesta (default `pending`), invece di una fetch per post. */
    /** B4: `reported` mostra i segnalati alla piattaforma (qualunque stato). */
    listForBlog: (token: string, blogSlug: string, status: CommentStatus = "pending", reported = false) =>
      request<BlogComment[]>(`/api/v1/blogs/${blogSlug}/comments?status=${status}${reported ? "&reported=true" : ""}`, { token }),
    create: (
      token: string | null,
      postId: string,
      payload: {
        content: string;
        parent_id?: string;
        author_display_name?: string;
        author_email?: string;
        /** Richiesto solo per un commento anonimo su un post/blog con
         * comments_mode "everyone" — token del widget Cloudflare Turnstile. */
        captcha_token?: string;
      }
    ) => request<Comment>(`/api/v1/posts/${postId}/comments`, { method: "POST", token, body: payload }),
    approve: (token: string, commentId: string) =>
      request<Comment>(`/api/v1/comments/${commentId}/approve`, { method: "POST", token }),
    report: (token: string, commentId: string, note: string) =>
      request<Comment>(`/api/v1/comments/${commentId}/report`, { method: "POST", token, body: { note } }),
    blockAuthor: (token: string, commentId: string, note?: string) =>
      request<BlockedAuthor>(`/api/v1/comments/${commentId}/block-author`, { method: "POST", token, body: { note } }),
    listBlocked: (token: string, blogSlug: string) => request<BlockedAuthor[]>(`/api/v1/blogs/${blogSlug}/blocked`, { token }),
    unblock: (token: string, blogSlug: string, blockId: string) =>
      request<void>(`/api/v1/blogs/${blogSlug}/blocked/${blockId}`, { method: "DELETE", token }),
    reject: (token: string, commentId: string) =>
      request<Comment>(`/api/v1/comments/${commentId}/reject`, { method: "POST", token }),
  },

  /** CLAUDE.md #1: anteprima di un link (titolo/descrizione/immagine Open
   * Graph), usata sia dall'editor sia dal rendering pubblico del post per i
   * link salvati come card. Pubblico, nessun token. */
  reports: {
    /** B5: segnalazione di un blog/post ai moderatori (sessione richiesta). */
    reportBlog: (token: string, slug: string, reason: ReportReason, note?: string) =>
      request<{ id: string }>(`/api/v1/blogs/${slug}/report`, { method: "POST", token, body: { reason, note } }),
    reportPost: (token: string, postId: string, reason: ReportReason, note?: string) =>
      request<{ id: string }>(`/api/v1/posts/${postId}/report`, { method: "POST", token, body: { reason, note } }),
  },

  linkPreview: {
    get: (url: string) =>
      request<{ url: string; title: string | null; description: string | null; image: string | null }>(
        `/api/v1/link-preview?url=${encodeURIComponent(url)}`
      ),
  },

  users: {
    profile: (username: string) => request<Profile>(`/api/v1/users/${username}`),
    /** Profilo privato del proprietario (include l'email) — a differenza di
     * `profile()`, mai leggibile su un altro utente. */
    me: (token: string) => request<MeProfile>("/api/v1/users/me", { token }),
    updateMe: (
      token: string,
      payload: {
        ui_locale?: string;
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
        /** Opt-out dalla directory pubblica (`GET /users`); assente non tocca. */
        directory_listed?: boolean;
        /** Chiavi canoniche (blocco "interessi utente"), al più 5; assente non tocca. */
        interests?: string[];
      }
    ) => request<MeProfile>("/api/v1/users/me", { method: "PATCH", token, body: payload }),
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
    publicBlogs: (username: string) => request<Blog[]>(`/api/v1/users/${username}/blogs`),
    publicPosts: (username: string) => request<Post[]>(`/api/v1/users/${username}/posts`),
    publicComments: (username: string) => request<PublicComment[]>(`/api/v1/users/${username}/comments`),
    followers: (username: string) =>
      request<{ username: string }[]>(`/api/v1/users/${username}/followers`),
    following: (username: string) =>
      request<{ username: string }[]>(`/api/v1/users/${username}/following`),
    /** GDPR Art. 20: istantanea di tutti i dati collegati all'account,
     * struttura libera (vedi backend/app/domain/gdpr.py::export_user_data). */
    exportData: (token: string) => request<Record<string, unknown>>("/api/v1/users/me/export-data", { token }),
    /** GDPR Art. 17: anonimizza l'account (non lo cancella fisicamente — vedi
     * backend/app/domain/gdpr.py). Richiede di ridigitare il proprio username. */
    deleteAccount: (token: string, confirmUsername: string) =>
      request<void>("/api/v1/users/me", {
        method: "DELETE",
        token,
        body: { confirm_username: confirmUsername },
      }),
    /** Cambio email, passo 1/3: invia un codice alla casella attuale. */
    requestEmailChange: (token: string, newEmail: string) =>
      request<{ detail: string }>("/api/v1/users/me/email/request", {
        method: "POST",
        token,
        body: { new_email: newEmail },
      }),
    /** Passo 2/3: verifica il codice inviato alla vecchia casella. */
    verifyCurrentEmail: (token: string, code: string) =>
      request<{ detail: string }>("/api/v1/users/me/email/verify-current", {
        method: "POST",
        token,
        body: { code },
      }),
    /** Passo 3/3: verifica il codice inviato alla nuova casella e applica il cambio. */
    verifyNewEmail: (token: string, code: string) =>
      request<MeProfile>("/api/v1/users/me/email/verify-new", {
        method: "POST",
        token,
        body: { code },
      }),
    /** Annulla una richiesta di cambio email pending, a qualunque passo. */
    cancelEmailChange: (token: string) =>
      request<void>("/api/v1/users/me/email/request", { method: "DELETE", token }),
    /** Registra/sostituisce il dominio custom (stato `pending`), ritorna le
     * istruzioni per il record TXT da pubblicare sul DNS. */
    setDomain: (token: string, domain: string) =>
      request<DomainOut>("/api/v1/users/me/domain", { method: "POST", token, body: { domain } }),
    verifyDomain: (token: string) =>
      request<DomainOut>("/api/v1/users/me/domain/verify", { method: "POST", token }),
    deleteDomain: (token: string) =>
      request<void>("/api/v1/users/me/domain", { method: "DELETE", token }),
  },

  fragments: {
    /** Frammenti già salvati dall'utente corrente su questo post — per
     * ri-evidenziarli ad ogni lettura. */
    listForPost: (token: string, postId: string) =>
      request<PostFragment[]>(`/api/v1/posts/${postId}/fragments`, { token }),
    create: (token: string, postId: string, text: string, isPublic: boolean = false) =>
      request<PostFragment>(`/api/v1/posts/${postId}/fragments`, {
        method: "POST",
        token,
        body: { text, is_public: isPublic },
      }),
    /** Raccolta unificata di tutti i frammenti salvati dall'utente. */
    listMine: (token: string) => request<FragmentCollectionEntry[]>("/api/v1/users/me/fragments", { token }),
    /** Cambia la visibilità di un frammento già salvato, anche ex-post. */
    setPublic: (token: string, fragmentId: string, isPublic: boolean) =>
      request<PostFragment>(`/api/v1/fragments/${fragmentId}`, {
        method: "PATCH",
        token,
        body: { is_public: isPublic },
      }),
    remove: (token: string, fragmentId: string) =>
      request<void>(`/api/v1/fragments/${fragmentId}`, { method: "DELETE", token }),
  },

  config: {
    get: () => request<InstanceConfig>("/api/v1/config"),
  },

  /** Pagine statiche del sito principale (admin/pagine, riservato ad
   * Amministratore/Super Admin) — non le pagine di un blog (vedi `blogs.pages` sopra). */
  pages: {
    /** Pubblico: solo pubblicate. Con token admin: anche le bozze. Parametro
     * `q` opzionale: filtra per titolo o slug (sottostringa). */
    list: (token: string | null, locale: string, q?: string) =>
      request<Page[]>(withQuery("/api/v1/pages", { locale, q }), { token }),
    create: (
      token: string,
      payload: { slug: string; locale: string; title: string; content: string; is_published: boolean }
    ) => request<Page>("/api/v1/pages", { method: "POST", token, body: payload }),
    addTranslation: (
      token: string,
      pageId: string,
      payload: { slug: string; locale: string; title: string; content: string; is_published: boolean }
    ) => request<Page>(`/api/v1/pages/${pageId}/translations`, { method: "POST", token, body: payload }),
    translations: (pageId: string) =>
      request<PageTranslationSummary[]>(`/api/v1/pages/${pageId}/translations`),
    update: (
      token: string,
      pageId: string,
      payload: Partial<{ slug: string; title: string; content: string; is_published: boolean }>
    ) => request<Page>(`/api/v1/pages/${pageId}`, { method: "PATCH", token, body: payload }),
  },

  feed: {
    /** Post dei blog/utenti seguiti (mockup 1c "Seguiti"): richiede sessione. */
    following: (token: string, limit = 20) =>
      request<Post[]>(`/api/v1/feed/posts?following=true&limit=${limit}`, { token }),
  },

  admin: {
    overview: (token: string) => request<AdminOverview>("/api/v1/admin/overview", { token }),
    /** B6: impostazioni di piattaforma (solo super admin) e coda GDPR. */
    getConfig: (token: string) => request<PlatformConfig>("/api/v1/admin/config", { token }),
    updateConfig: (token: string, payload: Partial<Omit<PlatformConfig, "sso_configured" | "reserved_builtin" | "updated_at" | "infrastructure">>) =>
      request<PlatformConfig>("/api/v1/admin/config", { method: "PATCH", token, body: payload }),
    listGdpr: (token: string, status?: GdprRequestStatus) => request<GdprRequest[]>(withQuery("/api/v1/admin/gdpr", { status }), { token }),
    createGdpr: (token: string, payload: { username: string; type: GdprRequestType; note: string }) =>
      request<GdprRequest>("/api/v1/admin/gdpr", { method: "POST", token, body: payload }),
    approveGdpr: (token: string, id: string) => request<GdprRequest>(`/api/v1/admin/gdpr/${id}/approve`, { method: "POST", token }),
    rejectGdpr: (token: string, id: string, note: string) =>
      request<GdprRequest>(`/api/v1/admin/gdpr/${id}/reject`, { method: "POST", token, body: { note } }),
    executeGdpr: (token: string, id: string) => request<Record<string, unknown>>(`/api/v1/admin/gdpr/${id}/execute`, { method: "POST", token }),
    listUsers: (token: string, q?: string) =>
      request<AdminUser[]>(withQuery("/api/v1/admin/users", { q }), { token }),
    updateUser: (
      token: string,
      userId: string,
      payload: Partial<{ platform_role: PlatformRole; is_active: boolean; note: string }>
    ) => request<AdminUser>(`/api/v1/admin/users/${userId}`, { method: "PATCH", token, body: payload }),
    /** Reset forzoso della password: innesca verso l'utente lo stesso ciclo
     * email di "password dimenticata" — l'admin non imposta una password,
     * solo avvia l'invio. Sempre 202, nessun corpo. */
    resetUserPassword: (token: string, userId: string) =>
      request<void>(`/api/v1/admin/users/${userId}/reset-password`, { method: "POST", token }),
    listBlogs: (token: string, q?: string) =>
      request<AdminBlog[]>(withQuery("/api/v1/admin/blogs", { q }), { token }),
    /** B5: `state` filtra per stato, `reported` = solo con segnalazioni aperte. */
    listBlogsFiltered: (token: string, filters: Partial<{ q: string; visibility: string; state: string }> = {}) =>
      request<AdminBlog[]>(withQuery("/api/v1/admin/blogs", filters), { token }),
    blogReports: (token: string, blogId: string) => request<BlogReports>(`/api/v1/admin/blogs/${blogId}/reports`, { token }),
    blogAction: (token: string, blogId: string, action: BlogAdminAction, note: string) =>
      request<AdminBlog>(`/api/v1/admin/blogs/${blogId}/action`, { method: "POST", token, body: { action, note } }),
    updateBlog: (token: string, blogId: string, payload: { is_suspended: boolean; note?: string }) =>
      request<AdminBlog>(`/api/v1/admin/blogs/${blogId}`, { method: "PATCH", token, body: payload }),
    listPosts: (token: string, q?: string) =>
      request<AdminPost[]>(withQuery("/api/v1/admin/posts", { q }), { token }),
    updatePost: (token: string, postId: string, payload: { is_hidden: boolean; note?: string }) =>
      request<AdminPost>(`/api/v1/admin/posts/${postId}`, { method: "PATCH", token, body: payload }),
    listAuditLog: (
      token: string,
      filters: Partial<{
        action: string;
        actor_id: string;
        target_id: string;
        blog_id: string;
        channel: AuditChannel;
        since: string;
        until: string;
        limit: string;
        offset: string;
      }> = {}
    ) => request<AuditLogEntry[]>(withQuery("/api/v1/admin/audit-log", filters), { token }),
    listComments: (token: string, filters: Partial<{ status: CommentStatus; q: string; reported: boolean }> = {}) =>
      request<AdminComment[]>(
        withQuery("/api/v1/admin/comments", { status: filters.status, q: filters.q, reported: filters.reported ? "true" : undefined }),
        { token }
      ),
  },

  tokens: {
    list: (token: string) => request<ApiToken[]>("/api/v1/tokens", { token }),
    create: (token: string, name: string) =>
      request<ApiTokenCreated>("/api/v1/tokens", { method: "POST", token, body: { name } }),
    revoke: (token: string, tokenId: string) =>
      request<void>(`/api/v1/tokens/${tokenId}`, { method: "DELETE", token }),
  },

  newsletter: {
    /** Pubblico, nessuna autenticazione: doppio opt-in. Risposta sempre
     * generica (202 `{status:"ok"}`), anche se l'indirizzo è già iscritto —
     * anti-enumerazione, vedi backend/app/api/v1/newsletter.py. */
    subscribe: (payload: { email: string; blog_slug?: string | null; locale?: string | null }) =>
      request<{ status: string }>("/api/v1/newsletter/subscribe", { method: "POST", body: payload }),
    confirm: (token: string) =>
      request<{ status: "confirmed" | "already_confirmed" | "invalid" }>(
        `/api/v1/newsletter/confirm?token=${encodeURIComponent(token)}`
      ),
    unsubscribe: (payload: { token: string; reason?: string | null }) =>
      request<{ status: string }>("/api/v1/newsletter/unsubscribe", { method: "POST", body: payload }),
    /** Cancellazione GDPR self-service (Art. 17), idempotente, senza login:
     * chi riceve l'email è già identificato dal token firmato del link. */
    unsubscribeAndDelete: (payload: { token: string }) =>
      request<{ status: string }>("/api/v1/newsletter/unsubscribe/delete", { method: "POST", body: payload }),
    blogStats: (token: string, slug: string) =>
      request<NewsletterStats>(`/api/v1/blogs/${slug}/newsletter/stats`, { token }),
    blogCampaigns: (token: string, slug: string) =>
      request<NewsletterCampaign[]>(`/api/v1/blogs/${slug}/newsletter/campaigns`, { token }),
    createBlogCampaign: (
      token: string,
      slug: string,
      payload: { subject: string; body_markdown: string; scheduled_at?: string | null }
    ) =>
      request<NewsletterCampaign>(`/api/v1/blogs/${slug}/newsletter/campaigns`, {
        method: "POST",
        token,
        body: payload,
      }),
    updateBlogSettings: (token: string, slug: string, payload: { newsletter_auto_notify_enabled: boolean }) =>
      request<{ newsletter_auto_notify_enabled: boolean }>(`/api/v1/blogs/${slug}/newsletter/settings`, {
        method: "PATCH",
        token,
        body: payload,
      }),
    adminStats: (token: string) => request<NewsletterStats>("/api/v1/admin/newsletter/stats", { token }),
    adminCampaigns: (token: string) => request<NewsletterCampaign[]>("/api/v1/admin/newsletter/campaigns", { token }),
    createAdminCampaign: (
      token: string,
      payload: { subject: string; body_markdown: string; scheduled_at?: string | null }
    ) => request<NewsletterCampaign>("/api/v1/admin/newsletter/campaigns", { method: "POST", token, body: payload }),
  },
};
