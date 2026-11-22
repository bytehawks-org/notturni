import type { SitemapEntry } from "./types";

/** Voci per pagina di sitemap: il protocollo ne ammette 50.000 (e 50 MB); molto meno per tenere le risposte leggere. */
export const SITEMAP_CHUNK = 5000;

const escapeXml = (s: string): string =>
  s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&apos;");

/** `<urlset>` con le traduzioni come `xhtml:link rel="alternate" hreflang` (indicazione per Google e Bing). */
export function urlsetXml(entries: SitemapEntry[], siteUrl: string): string {
  const urls = entries.map((e) => {
    const alternates = e.alternates
      .map((a) => `    <xhtml:link rel="alternate" hreflang="${escapeXml(a.locale)}" href="${escapeXml(siteUrl + a.path)}"/>`)
      .join("\n");
    return [
      "  <url>",
      `    <loc>${escapeXml(siteUrl + e.path)}</loc>`,
      e.updated_at ? `    <lastmod>${escapeXml(e.updated_at)}</lastmod>` : "",
      alternates,
      "  </url>",
    ]
      .filter(Boolean)
      .join("\n");
  });
  return `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n${urls.join("\n")}\n</urlset>\n`;
}

/** `<sitemapindex>` che punta alle pagine `/sitemaps/{n}.xml`. */
export function sitemapIndexXml(pages: number, siteUrl: string): string {
  const items = Array.from({ length: pages }, (_, i) => `  <sitemap>\n    <loc>${escapeXml(`${siteUrl}/sitemaps/${i}.xml`)}</loc>\n  </sitemap>`);
  return `<?xml version="1.0" encoding="UTF-8"?>\n<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${items.join("\n")}\n</sitemapindex>\n`;
}

export function xmlResponse(body: string): Response {
  return new Response(body, {
    headers: { "Content-Type": "application/xml; charset=utf-8", "Cache-Control": "public, max-age=0, s-maxage=3600, stale-while-revalidate=86400" },
  });
}
