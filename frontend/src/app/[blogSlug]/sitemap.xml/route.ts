import { getSpaceSitemap } from "@/lib/server-api";
import { urlsetXml, xmlResponse } from "@/lib/sitemap-xml";
import { siteUrl } from "@/lib/site";

export const dynamic = "force-dynamic";

/** Sitemap di un solo Spazio, su `/{slug}/sitemap.xml` e (riscritta da `proxy.ts`) su `{slug}.{dominio}/sitemap.xml`.
 * Elenca gli indirizzi canonici del portale, perché sono quelli che i motori di ricerca indicizzano. */
export async function GET(_req: Request, { params }: { params: Promise<{ blogSlug: string }> }): Promise<Response> {
  const { blogSlug } = await params;
  const sitemap = await getSpaceSitemap(blogSlug);
  if (!sitemap) return new Response("Not found", { status: 404 });
  return xmlResponse(urlsetXml(sitemap.entries, siteUrl()));
}
