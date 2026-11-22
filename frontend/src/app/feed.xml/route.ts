import { FEED_HEADERS, buildRss, postToFeedItem } from "@/lib/feed";
import { getPublicFeed } from "@/lib/server-api";
import { siteName, siteUrl } from "@/lib/site";

// Come sitemap.ts/robots.ts: senza questo Next prova a pre-renderizzare la
// route in fase di build e fallisce se il backend non è raggiungibile.
export const dynamic = "force-dynamic";

const FEED_LIMIT = 50;

/** RSS 2.0 del feed cronologico di piattaforma (richiesta esplicita) — stessi
 * post di `GET /api/v1/feed/posts` mostrati sulla home, già filtrati per
 * visibilità pubblica. Vedi anche `atom.xml` per la variante Atom. */
export async function GET() {
  const posts = await getPublicFeed({ limit: FEED_LIMIT }).catch(() => []);
  const xml = buildRss(
    {
      title: siteName(),
      description: `Ultimi post pubblicati su ${siteName()}.`,
      link: siteUrl(),
      feedUrl: `${siteUrl()}/feed.xml`,
      language: "it",
    },
    posts.map((post) => postToFeedItem(post))
  );
  return new Response(xml, { headers: FEED_HEADERS.rss });
}
