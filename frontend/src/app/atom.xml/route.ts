import { FEED_HEADERS, buildAtom, postToFeedItem } from "@/lib/feed";
import { getPublicFeed } from "@/lib/server-api";
import { getPrimaryBlogFeed } from "@/lib/primary-feed";
import { siteName, siteUrl } from "@/lib/site";

export const dynamic = "force-dynamic";

const FEED_LIMIT = 50;

/** Atom 1.0 del feed cronologico di piattaforma — stessi dati di `feed.xml`
 * (RSS 2.0), entrambi i formati per compatibilità coi diversi feed reader. */
export async function GET() {
  const primary = await getPrimaryBlogFeed("/atom.xml");
  if (primary) return new Response(buildAtom(primary.meta, primary.items), { headers: FEED_HEADERS.atom });
  const posts = await getPublicFeed({ limit: FEED_LIMIT }).catch(() => []);
  const xml = buildAtom(
    {
      title: siteName(),
      description: `Ultimi post pubblicati su ${siteName()}.`,
      link: siteUrl(),
      feedUrl: `${siteUrl()}/atom.xml`,
      language: "it",
    },
    posts.map((post) => postToFeedItem(post))
  );
  return new Response(xml, { headers: FEED_HEADERS.atom });
}
