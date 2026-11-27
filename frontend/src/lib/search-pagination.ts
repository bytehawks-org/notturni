/** Risultati per pagina nelle pagine di ricerca. */
export const SEARCH_PAGE_SIZE = 20;

/** Ultima pagina richiedibile: un valore enorme produrrebbe un `offset` non intero per il backend. */
const MAX_SEARCH_PAGE = 1000;

/** Numero di pagina dal query string: assente, non numerico o < 1 diventa 1; il massimo è `MAX_SEARCH_PAGE`. */
export function parseSearchPage(raw: string | undefined): number {
  const page = Math.floor(Number(raw));
  return Number.isFinite(page) && page >= 1 ? Math.min(page, MAX_SEARCH_PAGE) : 1;
}

/** Link a una pagina dei risultati. Mantiene `q` (URL encoded); la pagina 1 non porta `page`. */
export function searchPageHref(basePath: string, q: string, page: number): string {
  const pageParam = page > 1 ? `&page=${page}` : "";
  return `${basePath}/search?q=${encodeURIComponent(q)}${pageParam}`;
}
