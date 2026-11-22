import type { Blog, SpaceDisplayMode } from "./types";

/** Modalità di visualizzazione effettiva di uno Spazio. Un blog è sempre `feed`; wiki e docs usano la
 * modalità salvata se è una delle loro (`wiki`/`docs`), altrimenti il predefinito del tipo (coerente
 * con `rules::resolve_display_mode` del backend). */
export function effectiveDisplayMode(blog: Pick<Blog, "kind" | "display_mode">): SpaceDisplayMode {
  if (blog.kind === "blog") return "feed";
  if (blog.display_mode === "wiki" || blog.display_mode === "docs") return blog.display_mode;
  return blog.kind;
}
