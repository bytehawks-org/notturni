import "server-only";

import { REVALIDATE_SECONDS, revalidateTags } from "./revalidate";
import type {
  BibliographyEntry,
  Blog,
  BlogConfig,
  Category,
  LinkBibliographyEntry,
  MediaBibliographyEntry,
  Page,
  Post,
  PostTranslationSummary,
  Profile,
  PublicBlog,
  Publication,
  PublicationDetail,
  TrendingTag,
} from "./types";

// Le pagine renderizzate lato server (Server Component) girano nel processo
// Node.js del container: a differenza del browser, non raggiungono il
// backend sulla porta pubblicata ma per nome servizio nella rete di compose
// (vedi NOCT_BACKEND_INTERNAL_URL in compose.yaml). NEXT_PUBLIC_API_URL resta
// per il codice lato client (lib/api.ts) — i due URL non sono la stessa cosa.
const BACKEND_INTERNAL_URL =
  process.env.NOCT_BACKEND_INTERNAL_URL || process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000";

/** Recupera un post pubblico dal suo permalink /{blogSlug}/{postSlug}.
 * Ritorna null se non trovato/non pubblicamente visibile (404 dal backend) —
 * qualsiasi altro errore viene propagato. */
export async function getPublicPostByPermalink(
  blogSlug: string,
  postSlug: string
): Promise<Post | null> {
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/blogs/${blogSlug}/posts/${postSlug}`, {
    // Cacheato con finestra a tempo + tag: il backend invalida `post:…` e
    // `blog:…` al publish/update (vedi lib/revalidate.ts). Senza il webhook,
    // il post torna coerente comunque entro REVALIDATE_SECONDS.
    next: {
      revalidate: REVALIDATE_SECONDS,
      tags: [
        revalidateTags.post(blogSlug, postSlug),
        revalidateTags.blog(blogSlug),
        revalidateTags.feed(),
      ],
    },
  });
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero del post.`);
  return (await res.json()) as Post;
}

/** Pagina statica pubblica di un blog, dal permalink /{blogSlug}/pagina/{pageSlug}
 * (CLAUDE.md #1, feature opt-in — vedi Blog.static_pages_enabled). `null` se
 * non trovata/non pubblicata (404). */
export async function getPublicPage(
  blogSlug: string,
  pageSlug: string,
  locale: string
): Promise<Page | null> {
  const res = await fetch(
    `${BACKEND_INTERNAL_URL}/api/v1/blogs/${blogSlug}/pages/${pageSlug}?locale=${locale}`,
    {
      next: {
        revalidate: REVALIDATE_SECONDS,
        tags: [revalidateTags.blogPage(blogSlug, pageSlug), revalidateTags.blog(blogSlug)],
      },
    }
  );
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero della pagina.`);
  return (await res.json()) as Page;
}

/** Pagina statica pubblica del sito principale, permalink dedicato
 * /p/{slug} (non legata a un blog — vedi backend/API.md). `null` se non
 * trovata/non pubblicata (404). */
export async function getPublicPlatformPage(slug: string, locale: string): Promise<Page | null> {
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/pages/${slug}?locale=${locale}`, {
    next: {
      revalidate: REVALIDATE_SECONDS,
      tags: [revalidateTags.platformPage(slug), revalidateTags.platformPages()],
    },
  });
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero della pagina.`);
  return (await res.json()) as Page;
}

/** Tutte le pagine statiche pubblicate del sito principale, lingua di
 * default — usato da `app/sitemap.ts`. Il routing i18n non è ancora
 * costruito lato frontend (CLAUDE.md #1), da cui la lingua fissa. */
export async function getPublicPlatformPages(): Promise<Page[]> {
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/pages?locale=it`, {
    next: { revalidate: REVALIDATE_SECONDS, tags: [revalidateTags.platformPages()] },
  });
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero delle pagine di piattaforma.`);
  return (await res.json()) as Page[];
}

/** Dettaglio pubblico di un blog. `null` se non trovato o non visibile (404). */
export async function getPublicBlog(slug: string): Promise<Blog | null> {
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/blogs/${slug}`, {
    next: { revalidate: REVALIDATE_SECONDS, tags: [revalidateTags.blog(slug)] },
  });
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero del blog.`);
  return (await res.json()) as Blog;
}

/** Post pubblicati di un blog, dal più recente — per la sua homepage
 * pubblica (`/{blogSlug}`). Come `getPublicBlog`, nessun header di sessione:
 * un blog `members`/`private` risulterà quindi vuoto/404 anche per un
 * visitatore autenticato, stesso limite già presente sulle altre pagine
 * pubbliche renderizzate server-side (nessun modo di inoltrare il JWT, che
 * vive in `localStorage`, a un Server Component). */
export async function getPublicBlogPosts(slug: string): Promise<Post[] | null> {
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/blogs/${slug}/posts`, {
    next: { revalidate: REVALIDATE_SECONDS, tags: [revalidateTags.blog(slug)] },
  });
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero dei post del blog.`);
  return (await res.json()) as Post[];
}

/** Bibliografia automatica del blog: tutte le note dei post pubblicati. */
export async function getBlogBibliography(slug: string): Promise<BibliographyEntry[] | null> {
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/blogs/${slug}/bibliography`, {
    next: { revalidate: REVALIDATE_SECONDS, tags: [revalidateTags.blog(slug)] },
  });
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero della bibliografia.`);
  return (await res.json()) as BibliographyEntry[];
}

/** CLAUDE.md #4: come sopra, per i media (immagini) citati nei post pubblicati. */
export async function getBlogMediaBibliography(slug: string): Promise<MediaBibliographyEntry[] | null> {
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/blogs/${slug}/media-bibliography`, {
    next: { revalidate: REVALIDATE_SECONDS, tags: [revalidateTags.blog(slug)] },
  });
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero della bibliografia dei media.`);
  return (await res.json()) as MediaBibliographyEntry[];
}

/** CLAUDE.md #4: come sopra, per i link citati nei post pubblicati. */
export async function getBlogLinksBibliography(slug: string): Promise<LinkBibliographyEntry[] | null> {
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/blogs/${slug}/links-bibliography`, {
    next: { revalidate: REVALIDATE_SECONDS, tags: [revalidateTags.blog(slug)] },
  });
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero della bibliografia dei link.`);
  return (await res.json()) as LinkBibliographyEntry[];
}

/** Feed multi-blog per la homepage: post pubblicati di tutti i blog, dal più recente. */
export async function getPublicFeed(
  options: { locale?: string; tag?: string; category?: string; limit?: number } = {}
): Promise<Post[]> {
  const params = new URLSearchParams();
  if (options.locale) params.set("locale", options.locale);
  if (options.tag) params.set("tag", options.tag);
  if (options.category) params.set("category", options.category);
  if (options.limit) params.set("limit", String(options.limit));
  const qs = params.toString();
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/feed/posts${qs ? `?${qs}` : ""}`, {
    next: { revalidate: REVALIDATE_SECONDS, tags: [revalidateTags.feed()] },
  });
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero del feed.`);
  return (await res.json()) as Post[];
}

/** Directory pubblica dei blog indicizzabili (`GET /api/v1/blogs`, mockup
 * 4a/4c): ricerca `q`, filtro `locale`, ordinamento `active|new|followers`,
 * con i conteggi di post e follower. Nessun tag di rivalidazione dedicato:
 * si affida alla finestra a tempo `REVALIDATE_SECONDS`. */
export async function getPublicBlogs(
  options: { limit?: number; offset?: number; q?: string; locale?: string; sort?: "active" | "new" | "followers" } = {}
): Promise<PublicBlog[]> {
  const params = new URLSearchParams();
  if (options.limit) params.set("limit", String(options.limit));
  if (options.offset) params.set("offset", String(options.offset));
  if (options.q) params.set("q", options.q);
  if (options.locale) params.set("locale", options.locale);
  if (options.sort) params.set("sort", options.sort);
  const qs = params.toString();
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/blogs${qs ? `?${qs}` : ""}`, {
    next: { revalidate: REVALIDATE_SECONDS },
  });
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero della directory dei blog.`);
  return (await res.json()) as PublicBlog[];
}

/** Tag più usati tra i post pubblicati di recente, per la sezione "di tendenza" della homepage. */
export async function getTrendingTags(
  options: { days?: number; limit?: number } = {}
): Promise<TrendingTag[]> {
  const params = new URLSearchParams();
  if (options.days) params.set("days", String(options.days));
  if (options.limit) params.set("limit", String(options.limit));
  const qs = params.toString();
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/feed/trending${qs ? `?${qs}` : ""}`, {
    next: { revalidate: REVALIDATE_SECONDS, tags: [revalidateTags.feed()] },
  });
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero delle tendenze.`);
  return (await res.json()) as TrendingTag[];
}

export interface CrawlDirectives {
  search_disallow: string[];
  ai_disallow: string[];
  ai_user_agents: string[];
}

/** Percorsi da escludere in robots.txt (`app/robots.ts`), calcolati dal
 * backend (backend/app/domain/seo.py) a partire dagli opt-in per crawler di
 * blog/post — vedi Blog.search_indexing_enabled/ai_crawling_enabled. */
export async function getCrawlDirectives(): Promise<CrawlDirectives> {
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/seo/crawl-directives`, {
    next: { revalidate: REVALIDATE_SECONDS },
  });
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero delle direttive crawler.`);
  return (await res.json()) as CrawlDirectives;
}

export interface SitemapEntries {
  blogs: { slug: string; updated_at: string }[];
  posts: { permalink: string; updated_at: string }[];
}

/** Voci per `sitemap.xml` (`app/sitemap.ts`): solo blog/post effettivamente
 * indicizzabili — stesso criterio di `getCrawlDirectives` sopra (vedi
 * backend/app/domain/seo.py::build_sitemap_entries). */
export async function getSitemapEntries(): Promise<SitemapEntries> {
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/seo/sitemap-entries`, {
    next: { revalidate: REVALIDATE_SECONDS },
  });
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero delle voci della sitemap.`);
  return (await res.json()) as SitemapEntries;
}

/** Traduzioni (famiglia `translation_group_id`) di un post pubblico — per i
 * link "Italiano · English" nella colonna laterale del post (mockup 1a). */
export async function getPublicPostTranslations(postId: string): Promise<PostTranslationSummary[]> {
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/posts/${postId}/translations`, {
    next: { revalidate: REVALIDATE_SECONDS, tags: [revalidateTags.feed()] },
  });
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero delle traduzioni del post.`);
  return (await res.json()) as PostTranslationSummary[];
}

/** Palette/tipografia del blog (`GET /blogs/{slug}/config`, pubblico per i
 * blog pubblici) — applicata alla root delle pagine del blog (mockup 3f). */
export async function getPublicBlogConfig(slug: string): Promise<BlogConfig | null> {
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/blogs/${slug}/config`, {
    next: { revalidate: REVALIDATE_SECONDS, tags: [revalidateTags.blog(slug)] },
  });
  if (!res.ok) return null;
  return (await res.json()) as BlogConfig;
}

/** Profilo pubblico (mockup 3e, `GET /users/{username}`, nessuna autenticazione
 * richiesta lato backend): usato solo per i `<meta>` della pagina
 * `/u/{username}` (Client Component, ne rifà la propria fetch autenticata per
 * i dati interattivi — follow/tab). Nessun tag di invalidazione: il backend
 * non notifica ancora le modifiche al profilo, resta la sola finestra a
 * tempo (come senza `NOCT_REVALIDATE_SECRET` configurato altrove). */
export async function getPublicUserProfile(username: string): Promise<Profile | null> {
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/users/${username}`, {
    next: { revalidate: REVALIDATE_SECONDS },
  });
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero del profilo.`);
  return (await res.json()) as Profile;
}

/** Categorie del blog per i filtri della sua home pubblica (mockup 3f). */
export async function getPublicBlogCategories(slug: string): Promise<Category[]> {
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/blogs/${slug}/categories`, {
    next: { revalidate: REVALIDATE_SECONDS, tags: [revalidateTags.blog(slug)] },
  });
  if (!res.ok) return [];
  return (await res.json()) as Category[];
}

/** B9: pubblicazioni pubbliche del blog (solo con capitoli pubblicati). */
export async function getPublicPublications(slug: string): Promise<Publication[]> {
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/blogs/${slug}/publications`, {
    next: { revalidate: REVALIDATE_SECONDS, tags: [revalidateTags.blog(slug)] },
  });
  if (!res.ok) return [];
  return (await res.json()) as Publication[];
}

/** B9: indice dei capitoli di una pubblicazione; `null` se non esiste o non ha capitoli pubblicati. */
export async function getPublicPublication(slug: string, name: string): Promise<PublicationDetail | null> {
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/blogs/${slug}/publications/${name}`, {
    next: { revalidate: REVALIDATE_SECONDS, tags: [revalidateTags.blog(slug)] },
  });
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero della pubblicazione.`);
  return (await res.json()) as PublicationDetail;
}
