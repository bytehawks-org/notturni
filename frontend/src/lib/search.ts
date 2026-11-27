import type { Post } from "./types";

/** Ambito della ricerca: la piattaforma intera o un singolo blog (Space). */
export type SearchScope =
  | { kind: "portal" }
  | {
      kind: "blog";
      slug: string;
      /** Prefisso dei link interni, come `BlogLinks.basePath`: `/{slug}`, oppure `""` per il blog primario in solo. */
      basePath: string;
      title?: string;
    };

/** Risultati mostrati nel suggeritore. */
export const SEARCH_SUGGESTION_LIMIT = 8;
/** Lunghezza minima della query prima di interrogare il backend. */
export const SEARCH_MIN_LENGTH = 2;
/** Lunghezza massima della query inviata al backend. */
export const SEARCH_MAX_LENGTH = 100;
/** Attesa dopo l'ultimo tasto prima della richiesta, in millisecondi. */
export const SEARCH_DEBOUNCE_MS = 300;

/** Pagina dei risultati completi. Il percorso dipende dall'ambito. */
export function resultsHref(scope: SearchScope, q: string): string {
  const base = scope.kind === "blog" ? `${scope.basePath}/search` : "/search";
  return `${base}?q=${encodeURIComponent(q)}`;
}

/** Href di un singolo risultato. `post.permalink` arriva dall'API come `/{blogSlug}/{postSlug}`
 * oppure `/post/...`. Sul blog primario in solo (`basePath` vuoto) lo slug del blog viene tolto
 * in testa, come fa `BlogLinks.fromPermalink`. Con `basePath` valorizzato il permalink resta invariato. */
export function resultHref(scope: SearchScope, post: Pick<Post, "permalink">): string {
  const { permalink } = post;
  if (scope.kind !== "blog" || scope.basePath !== "") return permalink;
  const prefix = `/${scope.slug}`;
  return !permalink.startsWith("/post/") && permalink.startsWith(`${prefix}/`) ? permalink.slice(prefix.length) : permalink;
}

/** Normalizza la query: spazi ai bordi rimossi, lunghezza massima applicata. */
export function normalizeQuery(raw: string): string {
  return raw.trim().slice(0, SEARCH_MAX_LENGTH).trimEnd();
}
