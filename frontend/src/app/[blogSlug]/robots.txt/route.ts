import { getCrawlDirectives, getSpaceSitemap } from "@/lib/server-api";
import { siteUrl } from "@/lib/site";

export const dynamic = "force-dynamic";

/** Riscrive i percorsi del portale (`/{slug}/post`) per il sottodominio dello Spazio, dove il prefisso non c'è. */
function forSubdomain(paths: string[], slug: string): string[] {
  const prefix = `/${slug}`;
  const out = paths
    .filter((p) => p === prefix || p.startsWith(`${prefix}/`))
    .map((p) => (p === prefix ? "/" : p.slice(prefix.length)));
  return Array.from(new Set(out));
}

/** `robots.txt` di `{slug}.{dominio}`: stesse esclusioni del portale per quello Spazio (motori di ricerca e crawler
 * di IA) e rimando alla sua sitemap. Su un percorso `/{slug}/robots.txt` risponde allo stesso modo. */
export async function GET(_req: Request, { params }: { params: Promise<{ blogSlug: string }> }): Promise<Response> {
  const { blogSlug } = await params;
  const [directives, sitemap] = await Promise.all([getCrawlDirectives(), getSpaceSitemap(blogSlug)]);
  const lines: string[] = [];
  const group = (agent: string, disallow: string[]) => {
    lines.push(`User-agent: ${agent}`, "Allow: /", ...disallow.map((d) => `Disallow: ${d}`), "");
  };
  // Spazio non indicizzabile (privato, escluso, in pausa…): nessuna sitemap e nulla da scansionare
  const search = sitemap ? forSubdomain(directives.search_disallow, blogSlug) : ["/"];
  group("*", search);
  const ai = sitemap ? forSubdomain(directives.ai_disallow, blogSlug) : ["/"];
  if (ai.length > 0) for (const agent of directives.ai_user_agents) group(agent, ai);
  if (sitemap) lines.push(`Sitemap: ${siteUrl()}/${blogSlug}/sitemap.xml`);
  return new Response(`${lines.join("\n").trim()}\n`, {
    headers: { "Content-Type": "text/plain; charset=utf-8", "Cache-Control": "public, max-age=0, s-maxage=3600" },
  });
}
