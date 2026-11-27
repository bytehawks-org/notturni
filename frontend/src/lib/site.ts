/** Configurazione pubblica dell'istanza (nome, URL del sito, URL dell'API), risolta **a runtime**.
 *
 * `NEXT_PUBLIC_*` è inglobato nel bundle client in fase di `next build`: un'immagine buildata una volta
 * non potrebbe servire domini diversi (il browser chiamerebbe l'host sbagliato). Qui i valori si leggono
 * dalle env del container (`NOCT_*`, le stesse del ConfigMap) lato server, e il RootLayout li passa al
 * client con `SiteConfigProvider` → `setClientConfig`. Le `NEXT_PUBLIC_*` restano come fallback per lo
 * sviluppo locale (compose le passa come build-arg).
 *
 * Precedenza:
 *   - sito:  `NOCT_SITE_URL` → `https://{NOCT_INSTANCE_FQDN}` (solo con `NOCT_APP_ENV=production`) →
 *            `NEXT_PUBLIC_SITE_URL` → `http://localhost:3000`
 *   - API:   `NOCT_API_PUBLIC_URL` → `NEXT_PUBLIC_API_URL` → URL del sito (stessa origine: l'Ingress
 *            instrada già `/api` verso il backend)
 *   - nome:  `NOCT_SITE_NAME` → "Notturni" (il nome impostato in `/admin/seo` ha la precedenza, vedi layout).
 *   - modo:  `NOCT_DEPLOYMENT_MODE` = `solo` → "solo", qualsiasi altro valore (o assente) → "platform".
 *   - sottodomini: `NOCT_SUBDOMAINS_ENABLED`, default `true` se assente; `false|0|no|off` li disattiva
 *            (i blog restano raggiungibili solo path-based, `{siteUrl}/{slug}`). Il pacchetto solo lo
 *            imposta esplicitamente a `false`: qui `solo` da solo NON cambia il default. */
export interface SiteConfig {
  siteUrl: string;
  apiUrl: string;
  siteName: string;
  deploymentMode: "solo" | "platform";
  subdomainsEnabled: boolean;
}

export const DEFAULT_SITE_NAME = "Notturni";

const trimSlash = (v: string) => v.trim().replace(/\/+$/, "");

/** Valori dalle env del processo. Su server legge `process.env` a ogni chiamata (mai inlined: non sono
 * `NEXT_PUBLIC_*`); sul client `NOCT_*` è `undefined` e restano solo i fallback inlined al build. */
function configFromEnv(): SiteConfig {
  const env = process.env;
  const publicSiteUrl = process.env.NEXT_PUBLIC_SITE_URL;
  const publicApiUrl = process.env.NEXT_PUBLIC_API_URL;
  const fqdn = env.NOCT_INSTANCE_FQDN?.trim();
  const siteUrl = trimSlash(
    env.NOCT_SITE_URL ||
      (env.NOCT_APP_ENV === "production" && fqdn ? `https://${fqdn}` : "") ||
      publicSiteUrl ||
      "http://localhost:3000",
  );
  const apiUrl = trimSlash(env.NOCT_API_PUBLIC_URL || publicApiUrl || siteUrl);
  return {
    siteUrl,
    apiUrl,
    siteName: env.NOCT_SITE_NAME?.trim() || DEFAULT_SITE_NAME,
    deploymentMode: env.NOCT_DEPLOYMENT_MODE?.trim().toLowerCase() === "solo" ? "solo" : "platform",
    subdomainsEnabled: !["false", "0", "no", "off"].includes((env.NOCT_SUBDOMAINS_ENABLED ?? "").trim().toLowerCase()),
  };
}

let clientConfig: SiteConfig | null = null;

/** Chiamata dal `SiteConfigProvider` durante il render, prima dei figli (stesso valore a ogni richiesta). */
export function setClientConfig(config: SiteConfig) {
  clientConfig = config;
}

/** Su server sempre dalle env; nel browser dal valore passato dal layout. */
export function siteConfig(): SiteConfig {
  if (typeof window === "undefined") return configFromEnv();
  return clientConfig ?? configFromEnv();
}

/** URL pubblico del sito (SEO: metadataBase/canonical/sitemap.xml/robots.txt). */
export const siteUrl = () => siteConfig().siteUrl;

/** Solo host (senza schema/porta) per mostrare `{slug}.{host}` nelle directory. */
export const siteHost = () => new URL(siteConfig().siteUrl).hostname;

/** Base URL delle chiamate API dal browser. */
export const apiUrl = () => siteConfig().apiUrl;

/** Nome della piattaforma da env; il layout lo sostituisce con quello di `/admin/seo` se impostato. */
export const siteName = () => siteConfig().siteName;

/** `true` se i blog possono essere serviti su `{slug}.{host}` (default). */
export const subdomainsEnabled = () => siteConfig().subdomainsEnabled;

/** `true` con `NOCT_DEPLOYMENT_MODE=solo`. */
export const isSolo = () => siteConfig().deploymentMode === "solo";

/** `true` se `blogSlug` è il blog primario di un'istanza solo: i suoi post stanno su `/post/{postSlug}`. */
export const isPrimaryBlog = (blogSlug: string, primarySlug: string | null | undefined) =>
  isSolo() && !!primarySlug && primarySlug === blogSlug;

/** Href pubblico di una pagina del blog (`suffix` vuoto o che inizia con `/`), come `space_path_for` nel backend.
 * Blog primario in solo: `suffix`, oppure `/` se vuoto; altrimenti `/{blogSlug}{suffix}`. Usabile nei client component. */
export const spaceHref = (blogSlug: string, suffix: string, primarySlug: string | null | undefined) =>
  isPrimaryBlog(blogSlug, primarySlug) ? suffix || "/" : `/${blogSlug}${suffix}`;

/** Href pubblico di un post. Blog primario in solo: `/post/{postSlug}`; altrimenti `/{blogSlug}/{postSlug}`. */
export const postHref = (blogSlug: string, postSlug: string, primarySlug: string | null | undefined) =>
  isPrimaryBlog(blogSlug, primarySlug) ? `/post/${postSlug}` : `/${blogSlug}/${postSlug}`;

/** Indirizzo pubblico mostrato per un blog: `{slug}.{host}` con i sottodomini attivi, altrimenti `{siteUrl}/{slug}`. */
export const blogAddress = (slug: string) => (subdomainsEnabled() ? `${slug}.${siteHost()}` : `${siteUrl()}/${slug}`);
