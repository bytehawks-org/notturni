import { FEED_HEADERS, buildAtom, postToFeedItem } from "@/lib/feed";
import { getPublicFeed } from "@/lib/server-api";
import { SITE_URL } from "@/lib/site";

export const dynamic = "force-dynamic";

const FEED_LIMIT = 50;

/** Atom 1.0 del feed cronologico di piattaforma — stessi dati di `feed.xml`
 * (RSS 2.0), entrambi i formati per compatibilità coi diversi feed reader. */
export async function GET() {
  const posts = await getPublicFeed({ limit: FEED_LIMIT }).catch(() => []);
  const xml = buildAtom(
    {
      title: "Notturni",
      description: "Ultimi post pubblicati su Notturni.",
      link: SITE_URL,
      feedUrl: `${SITE_URL}/atom.xml`,
      language: "it",
    },
    posts.map((post) => postToFeedItem(post))
  );
  return new Response(xml, { headers: FEED_HEADERS.atom });
}
