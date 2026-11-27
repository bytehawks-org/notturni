/** Routing of the primary blog at the domain root (solo mode). Pure functions, used by `src/proxy.ts`. */

export type SoloRoute = { type: "rewrite" | "redirect"; pathname: string } | { type: "none" };

/** Subpages of the primary blog served at the root: exact paths. */
const EXACT_PAGES = new Set(["/search", "/media", "/link", "/bibliografia", "/pub"]);
/** Subpages of the primary blog served at the root: path prefixes. */
const PAGE_PREFIXES = ["/pub/", "/pagina/"];
/** Files with their own root route: old URLs redirect to them, the root path is never rewritten. */
const ROOT_FILES = new Set(["/feed.xml", "/atom.xml", "/sitemap.xml", "/robots.txt"]);

const isRootPage = (path: string) => EXACT_PAGES.has(path) || PAGE_PREFIXES.some((p) => path.startsWith(p));
const isMovedPath = (path: string) => isRootPage(path) || ROOT_FILES.has(path);
/** `/a/b/c` gives `/b/c`; a path with one segment gives an empty string. */
const afterFirstSegment = (pathname: string) => {
  const slash = pathname.indexOf("/", 1);
  return slash === -1 ? "" : pathname.slice(slash);
};

/** `false` if no primary slug can change the result: the proxy then skips the backend call. */
export const needsPrimarySlug = (pathname: string) => isRootPage(pathname) || isMovedPath(afterFirstSegment(pathname));

/** Decides what to do with `pathname` on the apex host of a solo instance.
 *
 * - `redirect` (308): old `/{primary}<path>` URL, to `<path>`. `<path>` is always in the fixed sets above, so the
 *   target cannot start with `//` (no open redirect).
 * - `rewrite`: root subpage, served by the `/{primary}<path>` route. A rewrite does not run the proxy again.
 *
 * The redirect check comes first. Each redirect makes the path shorter, so redirects cannot loop, also when the
 * primary slug equals a subpage name. The exact home `/{primary}` and the posts are redirected by the pages. */
export function soloRoute(pathname: string, primary: string | null): SoloRoute {
  if (!primary) return { type: "none" };
  const prefix = `/${primary}`;
  if (pathname.startsWith(`${prefix}/`)) {
    const rest = pathname.slice(prefix.length);
    if (isMovedPath(rest)) return { type: "redirect", pathname: rest };
  }
  if (isRootPage(pathname)) return { type: "rewrite", pathname: `${prefix}${pathname}` };
  return { type: "none" };
}
