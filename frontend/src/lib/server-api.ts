import "server-only";

import type { BibliographyEntry, Blog, Post, TrendingTag } from "./types";

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
    // niente cache tra una request e l'altra: un post appena pubblicato deve
    // essere visibile subito, non serve una strategia di revalidate qui.
    cache: "no-store",
  });
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero del post.`);
  return (await res.json()) as Post;
}

/** Dettaglio pubblico di un blog. `null` se non trovato o non visibile (404). */
export async function getPublicBlog(slug: string): Promise<Blog | null> {
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/blogs/${slug}`, { cache: "no-store" });
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero del blog.`);
  return (await res.json()) as Blog;
}

/** Bibliografia automatica del blog: tutte le note dei post pubblicati. */
export async function getBlogBibliography(slug: string): Promise<BibliographyEntry[] | null> {
  const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/blogs/${slug}/bibliography`, {
    cache: "no-store",
  });
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero della bibliografia.`);
  return (await res.json()) as BibliographyEntry[];
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
    cache: "no-store",
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
    cache: "no-store",
  });
  if (!res.ok) throw new Error(`Errore ${res.status} nel recupero delle tendenze.`);
  return (await res.json()) as TrendingTag[];
}
