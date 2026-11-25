import { headers } from "next/headers";
import { isPrimaryBlog, siteHost, subdomainsEnabled } from "@/lib/site";

export interface BlogLinks {
  /** Prefisso da anteporre ai link interni *costruiti a mano* (es.
   * `${basePath}/media`): `/{blogSlug}` quando la pagina è raggiunta
   * path-based (`notturni.eu/{blogSlug}/...`, sempre vero finché il DNS
   * wildcard non è configurato), stringa vuota quando è raggiunta dal
   * proprio sottodominio (`{blogSlug}.notturni.eu/...`, `src/proxy.ts`
   * ha già riscritto internamente il path aggiungendo lo slug — ripeterlo
   * nell'href genererebbe un doppio segmento). Il blog primario in solo
   * mantiene `/{blogSlug}` anche sul dominio principale: solo la sua home
   * sta su `/` (vedi `homeHref`). */
  basePath: string;
  /** Href della home del blog. `/` per il blog primario in solo (anche
   * path-based) e sul sottodominio; altrimenti `basePath || "/"`. */
  homeHref: string;
  /** Converte un permalink assoluto già prefissato con lo slug (es.
   * `Post.permalink`, `/{blogSlug}/{postSlug}`, calcolato dal backend — vedi
   * crates/noct-posts) nell'href corretto per il contesto
   * corrente: invariato path-based, con lo slug tolto in testa sul
   * sottodominio. Va sempre usato per gli href che arrivano già formati
   * dall'API, mai concatenato manualmente come `${basePath}${permalink}`. */
  fromPermalink: (permalink: string) => string;
}

/** `true` se la richiesta arriva dal sottodominio del blog (`{blogSlug}.{host}`). */
export async function isOnBlogSubdomain(blogSlug: string): Promise<boolean> {
  const host = (await headers()).get("host") ?? "";
  const hostname = host.split(":")[0];
  return subdomainsEnabled() && hostname === `${blogSlug}.${siteHost()}`;
}

/** `primarySlug`: slug del blog primario (`getPrimaryBlogSlug()`), o `null`/omesso. Per il blog primario in solo
 * `basePath` resta `/{slug}` (sottopagine sul dominio principale) e `homeHref` è `/`; i permalink `/post/...`
 * restano invariati. Le pagine usano `currentBlogLinks` (`@/lib/blog-links`), che legge il primario dal backend. */
export async function blogLinks(blogSlug: string, primarySlug?: string | null): Promise<BlogLinks> {
  const onSubdomain = await isOnBlogSubdomain(blogSlug);
  const primary = isPrimaryBlog(blogSlug, primarySlug);
  const prefix = `/${blogSlug}`;
  return {
    basePath: onSubdomain ? "" : prefix,
    homeHref: primary || onSubdomain ? "/" : prefix,
    fromPermalink: (permalink) =>
      onSubdomain && !permalink.startsWith("/post/") && permalink.startsWith(`${prefix}/`)
        ? permalink.slice(prefix.length)
        : permalink,
  };
}

/** Solo il prefisso (pagine senza permalink da riscrivere) — scorciatoia su
 * `blogLinks` per i chiamanti che non ne hanno bisogno. */
export async function blogBasePath(blogSlug: string): Promise<string> {
  return (await blogLinks(blogSlug)).basePath;
}
