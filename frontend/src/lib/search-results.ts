import type { Post } from "./types";
import { SEARCH_PAGE_SIZE } from "./search-pagination";
import { SearchRateLimitedError } from "./server-api";

export interface SearchResultsPage {
  /** Al più `SEARCH_PAGE_SIZE` risultati della pagina richiesta. */
  items: Post[];
  /** Esiste almeno un risultato oltre questa pagina. */
  hasNext: boolean;
  /** La API ha risposto 429: la pagina mostra un messaggio, non un errore. */
  rateLimited: boolean;
}

/** Carica una pagina di risultati chiedendone una in più, per sapere se esiste la successiva.
 * Ogni altro errore viene propagato. */
export async function fetchSearchResultsPage(
  load: (options: { limit: number; offset: number }) => Promise<Post[] | null>,
  page: number
): Promise<SearchResultsPage> {
  try {
    const fetched =
      (await load({ limit: SEARCH_PAGE_SIZE + 1, offset: (page - 1) * SEARCH_PAGE_SIZE })) ?? [];
    return {
      items: fetched.slice(0, SEARCH_PAGE_SIZE),
      hasNext: fetched.length > SEARCH_PAGE_SIZE,
      rateLimited: false,
    };
  } catch (error) {
    if (error instanceof SearchRateLimitedError) return { items: [], hasNext: false, rateLimited: true };
    throw error;
  }
}
