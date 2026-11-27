import { SITEMAP_CHUNK, urlsetXml, xmlResponse } from "@/lib/sitemap-xml";
import { getRootSitemapPage } from "@/lib/primary-feed";
import { siteUrl } from "@/lib/site";

export const dynamic = "force-dynamic";

/** `/sitemaps/{n}.xml`: la n-esima pagina della sitemap root (globale più extra del primario in solo). */
export async function GET(_req: Request, { params }: { params: Promise<{ file: string }> }): Promise<Response> {
  const { file } = await params;
  const match = /^(\d{1,6})\.xml$/.exec(file);
  if (!match) return new Response("Not found", { status: 404 });
  const index = Number(match[1]);
  const page = await getRootSitemapPage(index * SITEMAP_CHUNK, SITEMAP_CHUNK);
  if (!page) return new Response("Sitemap non disponibile", { status: 503 });
  if (page.entries.length === 0) return new Response("Not found", { status: 404 });
  return xmlResponse(urlsetXml(page.entries, siteUrl()));
}
