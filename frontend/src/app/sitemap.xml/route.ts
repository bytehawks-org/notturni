import { getSitemapPage } from "@/lib/server-api";
import { SITEMAP_CHUNK, sitemapIndexXml, urlsetXml, xmlResponse } from "@/lib/sitemap-xml";
import { SITE_URL } from "@/lib/site";

// Come robots.txt: senza questo Next prova a pre-renderizzare la route in fase di build e fallisce se il backend
// non è raggiungibile; a runtime resta comunque cacheata dalla finestra del fetch.
export const dynamic = "force-dynamic";

/** Sitemap globale: un solo `<urlset>` finché sta in una pagina, poi un indice che punta a `/sitemaps/{n}.xml`.
 * Il portale chiuso agli indici (`noindex`) risponde con un elenco vuoto. */
export async function GET(): Promise<Response> {
  const page = await getSitemapPage(0, SITEMAP_CHUNK);
  if (!page) return new Response("Sitemap non disponibile", { status: 503 });
  if (page.total > SITEMAP_CHUNK) return xmlResponse(sitemapIndexXml(Math.ceil(page.total / SITEMAP_CHUNK), SITE_URL));
  return xmlResponse(urlsetXml(page.entries, SITE_URL));
}
