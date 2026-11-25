/**
 * Gallery fence helpers shared by the Markdown renderer and the editor.
 *
 * A gallery is a fenced block with info string `gallery`. Each non-empty line
 * is a standard Markdown image, with an optional title `sensitive` or
 * `sensitive:cat1,cat2`. The backend (`IMAGE_RE` in
 * `crates/noct-posts/src/rules.rs`) reads the same images for media tracking.
 */

export const GALLERY_MAX = 10;

export type GalleryItem = {
  url: string;
  alt: string;
  title?: string;
  sensitive: boolean;
  categories: string[];
};

export type GalleryModifier = "n1" | "n2" | "n3" | "n4";

export type GalleryLayout = {
  modifier: GalleryModifier;
  /** Tiles shown in the grid (max 4). */
  visible: number;
  /** Images hidden behind the "+N" tile (N > 4). */
  overflow: number;
};

// Same URL pattern as the backend: one level of balanced parentheses allowed.
const URL_PATTERN = String.raw`[^\s()]+(?:\([^\s()]*\)[^\s()]*)*`;

// alt: any char except an unescaped `]`; `\]` is accepted and unescaped on parse.
const IMAGE_LINE_RE = new RegExp(
  String.raw`^\s*!\[((?:\\\]|[^\]])*)\]\(\s*(${URL_PATTERN})(?:\s+"([^"]*)")?\s*\)\s*$`,
);

function parseCategories(title: string): string[] {
  const colon = title.indexOf(":");
  if (colon === -1) return [];
  return title
    .slice(colon + 1)
    .split(",")
    .map((c) => c.trim())
    .filter((c) => c.length > 0);
}

/** Parses the body of a gallery fence into items. Skips non-image lines and caps at GALLERY_MAX. */
export function parseGalleryBody(body: string): GalleryItem[] {
  const items: GalleryItem[] = [];
  for (const line of body.split(/\r?\n/)) {
    if (items.length >= GALLERY_MAX) break;
    const m = IMAGE_LINE_RE.exec(line);
    if (!m) continue;
    const title = m[3];
    const sensitive = title !== undefined && title.startsWith("sensitive");
    items.push({
      url: m[2],
      alt: m[1].replace(/\\\]/g, "]"),
      title,
      sensitive,
      categories: sensitive && title !== undefined ? parseCategories(title) : [],
    });
  }
  return items;
}

function imageLine(item: GalleryItem): string {
  // The backend image regex does not accept "]" in alt text: drop brackets.
  const alt = item.alt.replace(/[\[\]]/g, "");
  let title = "";
  if (item.sensitive) {
    const cats = item.categories.map((c) => c.trim()).filter((c) => c.length > 0);
    title = cats.length > 0 ? ` "sensitive:${cats.join(",")}"` : ` "sensitive"`;
  }
  return `![${alt}](${item.url}${title})`;
}

/**
 * Serializes items to a gallery block, fences included.
 * - 0 items: empty string.
 * - 1 item: a plain image line (no fence), per the E04 decision.
 * - Titles are emitted only for sensitive items.
 * Items beyond GALLERY_MAX are dropped.
 */
export function serializeGallery(items: GalleryItem[]): string {
  const kept = items.slice(0, GALLERY_MAX);
  if (kept.length === 0) return "";
  if (kept.length === 1) return imageLine(kept[0]);
  return ["```gallery", ...kept.map(imageLine), "```"].join("\n");
}

/** Layout for N images: n1 (one full-width image), n2, n3, n4+ (1 large + 3 small, "+N" on the last small tile). */
export function galleryLayout(n: number): GalleryLayout {
  const count = Math.max(0, Math.floor(n));
  const modifier: GalleryModifier =
    count <= 1 ? "n1" : count === 2 ? "n2" : count === 3 ? "n3" : "n4";
  return {
    modifier,
    visible: Math.min(count, 4),
    overflow: Math.max(0, count - 4),
  };
}
