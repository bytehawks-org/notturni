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
 *   - nome:  `NOCT_SITE_NAME` → "Notturni" (il nome impostato in `/admin/seo` ha la precedenza, vedi layout). */
export interface SiteConfig {
  siteUrl: string;
  apiUrl: string;
  siteName: string;
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
  return { siteUrl, apiUrl, siteName: env.NOCT_SITE_NAME?.trim() || DEFAULT_SITE_NAME };
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
