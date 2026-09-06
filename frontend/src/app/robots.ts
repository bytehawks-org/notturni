import type { MetadataRoute } from "next";

import { getCrawlDirectives } from "@/lib/server-api";
import { SITE_URL } from "@/lib/site";

// Senza questo, Next prova a pre-renderizzare /robots.txt in fase di build
// (route "cacheata di default" per le route speciali, vedi doc Next.js) e la
// build fallisce se il backend non è raggiungibile in quel momento — a
// runtime resta comunque cacheato dalla finestra REVALIDATE_SECONDS del
// fetch in getCrawlDirectives, non da qui.
export const dynamic = "force-dynamic";

/** Robots.txt generato dinamicamente (punto 6: opt-in per crawler a livello
 * di blog/post — vedi backend/app/domain/seo.py). Un gruppo user-agent
 * specifico sostituisce interamente `User-agent: *` per quel bot, non lo
 * integra: `ai_disallow` è quindi già un superset di `search_disallow` (vedi
 * commento in build_crawl_directives), non serve unirli qui. */
export default async function robots(): Promise<MetadataRoute.Robots> {
  const { search_disallow, ai_disallow, ai_user_agents } = await getCrawlDirectives();

  const rules: MetadataRoute.Robots["rules"] = [
    { userAgent: "*", allow: "/", disallow: search_disallow.length > 0 ? search_disallow : undefined },
  ];

  if (ai_disallow.length > 0) {
    for (const userAgent of ai_user_agents) {
      rules.push({ userAgent, allow: "/", disallow: ai_disallow });
    }
  }

  return { rules, sitemap: `${SITE_URL}/sitemap.xml` };
}
