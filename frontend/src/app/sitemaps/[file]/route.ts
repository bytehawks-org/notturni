import { getSitemapPage } from "@/lib/server-api";
import { SITEMAP_CHUNK, urlsetXml, xmlResponse } from "@/lib/sitemap-xml";
import { SITE_URL } from "@/lib/site";

export const dynamic = "force-dynamic";

/** `/sitemaps/{n}.xml`: la n-esima pagina della sitemap globale quando non sta in un solo file. */
export async function GET(_req: Request, { params }: { params: Promise<{ file: string }> }): Promise<Response> {
  const { file } = await params;
  const match = /^(\d{1,6})\.xml$/.exec(file);
  if (!match) return new Response("Not found", { status: 404 });
  const index = Number(match[1]);
  const page = await getSitemapPage(index * SITEMAP_CHUNK, SITEMAP_CHUNK);
  if (!page) return new Response("Sitemap non disponibile", { status: 503 });
  if (page.entries.length === 0) return new Response("Not found", { status: 404 });
  return xmlResponse(urlsetXml(page.entries, SITE_URL));
}
