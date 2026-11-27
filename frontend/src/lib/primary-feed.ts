import { getPublicBlog, getPublicBlogPosts, getPrimaryBlogSlug, getSitemapPage, getSpaceSitemap } from "@/lib/server-api";
import type { SitemapEntry } from "@/lib/types";
import { postToFeedItem, type FeedItem, type FeedMeta } from "@/lib/feed";
import { siteUrl } from "@/lib/site";

/** Feed del blog primario per i percorsi di piattaforma `/feed.xml` e `/atom.xml` in solo: stesso contenuto della
 * home `/`. `null` se non è solo, non c'è un blog primario o il blog non è visibile; la route ricade sul feed di
 * piattaforma. */
export async function getPrimaryBlogFeed(
  feedPath: "/feed.xml" | "/atom.xml",
): Promise<{ meta: FeedMeta; items: FeedItem[] } | null> {
  const slug = await getPrimaryBlogSlug();
  if (!slug) return null;
  const [blog, posts] = await Promise.all([getPublicBlog(slug), getPublicBlogPosts(slug).catch(() => null)]);
  if (!blog || !posts) return null;
  return {
    meta: {
      title: blog.title,
      description: blog.description ?? blog.subtitle ?? `Post pubblicati su ${blog.title}.`,
      link: siteUrl(),
      feedUrl: `${siteUrl()}${feedPath}`,
      language: blog.default_locale,
    },
    items: posts.map((post) => postToFeedItem(post)),
  };
}

/** Percorsi del blog primario che la sitemap globale non contiene: pagine e pubblicazioni, già in root. */
const PRIMARY_SITEMAP_PREFIXES = ["/pagina/", "/pub/"];

/** Voci `/pagina/..` e `/pub/..` del blog primario per la sitemap root (solo). Vuoto se non è solo, non c'è un
 * primario o la sua sitemap non è disponibile (404/null). Queste voci non sono nel `total` della sitemap globale. */
export async function getPrimarySitemapExtras(): Promise<SitemapEntry[]> {
  const slug = await getPrimaryBlogSlug();
  if (!slug) return [];
  const space = await getSpaceSitemap(slug);
  return (space?.entries ?? []).filter((entry) => PRIMARY_SITEMAP_PREFIXES.some((prefix) => entry.path.startsWith(prefix)));
}

/** Pagina `[offset, offset + limit)` della sitemap root: prima le voci globali, poi gli extra del primario.
 * `count` è il totale virtuale (globale + extra), da usare per il chunking. `null` se il backend non risponde.
 * Oltre la fine globale il backend risponde `total: 0`: il totale globale si legge quindi da `offset 0`. */
export async function getRootSitemapPage(
  offset: number,
  limit: number,
): Promise<{ count: number; entries: SitemapEntry[] } | null> {
  const [page, extras] = await Promise.all([getSitemapPage(offset, limit), getPrimarySitemapExtras()]);
  if (!page) return null;
  let globalTotal = page.total;
  if (offset > 0 && extras.length > 0) {
    const head = await getSitemapPage(0, 1);
    if (!head) return null;
    globalTotal = head.total;
  }
  const extrasSlice = extras.slice(Math.max(0, offset - globalTotal), Math.max(0, offset + limit - globalTotal));
  return { count: globalTotal + extras.length, entries: [...page.entries, ...extrasSlice] };
}
