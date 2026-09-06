/** URL pubblico del sito (SEO: metadataBase/canonical/sitemap.xml/robots.txt).
 * `NEXT_PUBLIC_*` è inglobato nel bundle in fase di build (vedi Dockerfile),
 * non letto a runtime — stesso schema di `NEXT_PUBLIC_API_URL`, coerente
 * anche lato Server Component: entrambi finiscono nello stesso bundle unico
 * (standalone Next.js), non serve una controparte "interna" come per l'URL
 * del backend. */
export const SITE_URL = (process.env.NEXT_PUBLIC_SITE_URL || "http://localhost:3000").replace(/\/$/, "");
