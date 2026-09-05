import "server-only";

import { REVALIDATE_SECONDS, revalidateTags } from "./revalidate";
import type {
  BibliographyEntry,
  Blog,
  LinkBibliographyEntry,
  MediaBibliographyEntry,
  Page,
  Post,
  TrendingTag,
} from "./types";

// Le pagine renderizzate lato server (Server Component) girano nel processo
// Node.js del container: a differenza del browser, non raggiungono il
// backend sulla porta pubblicata ma per nome servizio nella rete di compose
// (vedi NOCT_BACKEND_INTERNAL_URL in compose.yaml). NEXT_PUBLIC_API_URL resta
// per il codice lato client (lib/api.ts) — i due URL non sono la stessa cosa.
const BACKEND_INTERNAL_URL =
  process.env.NOCT_BACKEND_INTERNAL_URL || process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000";

/** Recupera un post pubblico dal suo permalink /{blogSlug}/{date}/{postSlug}.
 * Ritorna null se non trovato/non pubblicamente visibile (404 dal backend) —
 * qualsiasi altro errore viene propagato. */
export async function getPublicPostByPermalink(
  blogSlug: string,
  date: string,
  postSlug: string
): Promise<Post | null> {
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/blogs/${blogSlug}/posts/${date}/${postSlug}`, {
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
 * /pages/{slug} (non legata a un blog — vedi backend/API.md). `null` se non
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

/** Dettaglio pubblico di un blog. `null` se non trovato o non visibile (404). */
export async function getPublicBlog(slug: string): Promise<Blog | null> {
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/blogs/${slug}`, {
    next: { revalidate: REVALIDATE_SECONDS, tags: [revalidateTags.blog(slug)] },
  });
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero del blog.`);
  return (await res.json()) as Blog;
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
