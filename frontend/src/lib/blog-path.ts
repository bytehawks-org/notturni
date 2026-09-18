import { headers } from "next/headers";
import { SITE_HOST } from "@/lib/site";

export interface BlogLinks {
  /** Prefisso da anteporre ai link interni *costruiti a mano* (es.
   * `${basePath}/media`): `/{blogSlug}` quando la pagina è raggiunta
   * path-based (`notturni.eu/{blogSlug}/...`, sempre vero finché il DNS
   * wildcard non è configurato), stringa vuota quando è raggiunta dal
   * proprio sottodominio (`{blogSlug}.notturni.eu/...`, `src/proxy.ts`
   * ha già riscritto internamente il path aggiungendo lo slug — ripeterlo
   * nell'href genererebbe un doppio segmento). */
  basePath: string;
  /** Converte un permalink assoluto già prefissato con lo slug (es.
   * `Post.permalink`, `/{blogSlug}/{postSlug}`, calcolato dal backend — vedi
   * backend/app/domain/permalinks.py) nell'href corretto per il contesto
   * corrente: invariato path-based, con lo slug tolto in testa sul
   * sottodominio. Va sempre usato per gli href che arrivano già formati
   * dall'API, mai concatenato manualmente come `${basePath}${permalink}`. */
  fromPermalink: (permalink: string) => string;
}

export async function blogLinks(blogSlug: string): Promise<BlogLinks> {
  const host = (await headers()).get("host") ?? "";
  const hostname = host.split(":")[0];
  const onSubdomain = hostname === `${blogSlug}.${SITE_HOST}`;
  const prefix = `/${blogSlug}`;
  return {
    basePath: onSubdomain ? "" : prefix,
    fromPermalink: (permalink) =>
      onSubdomain && permalink.startsWith(`${prefix}/`) ? permalink.slice(prefix.length) : permalink,
  };
}

/** Solo il prefisso (pagine senza permalink da riscrivere) — scorciatoia su
 * `blogLinks` per i chiamanti che non ne hanno bisogno. */
export async function blogBasePath(blogSlug: string): Promise<string> {
  return (await blogLinks(blogSlug)).basePath;
}
