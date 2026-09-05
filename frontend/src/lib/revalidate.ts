/** Rivalidazione della cache dei Server Component pubblici.
 *
 * I fetch pubblici in `server-api.ts` (feed, post, blog, bibliografie, pagine)
 * sono cacheati con una finestra a tempo — `REVALIDATE_SECONDS` — e con un
 * insieme di tag, così il backend può invalidarli in modo mirato dopo una
 * modifica (publish/update di un post, config o impostazioni di un blog,
 * pagine statiche): vedi la route handler `app/api/revalidate`.
 *
 * Le stringhe dei tag devono restare identiche a quelle usate dal backend in
 * `backend/app/core/revalidation.py` — cambiarle in un solo posto rompe
 * l'invalidazione senza errori visibili.
 */

/** Finestra di rivalidazione a tempo (secondi). Anche senza il webhook del
 * backend, un contenuto pubblico modificato torna coerente entro questo
 * intervallo. */
export const REVALIDATE_SECONDS = 60;

export const revalidateTags = {
  /** Homepage: feed cronologico + tendenze. */
  feed: (): string => "feed",
  /** Tutte le pagine pubbliche di un blog (post, bibliografie, dettaglio blog). */
  blog: (slug: string): string => `blog:${slug}`,
  /** Pagina pubblica di un singolo post (permalink). */
  post: (blogSlug: string, postSlug: string): string => `post:${blogSlug}:${postSlug}`,
  /** Elenco delle pagine statiche di piattaforma. */
  platformPages: (): string => "platform-pages",
  /** Una singola pagina statica di piattaforma (`/pages/{slug}`). */
  platformPage: (slug: string): string => `platform-page:${slug}`,
  /** Una singola pagina statica di un blog (`/{blog}/pagina/{slug}`). */
  blogPage: (blogSlug: string, pageSlug: string): string => `blog-page:${blogSlug}:${pageSlug}`,
} as const;
