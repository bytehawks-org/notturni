import { Node, mergeAttributes } from "@tiptap/core";
import type { MarkdownNodeSpec } from "tiptap-markdown";
import { GALLERY_MAX, parseGalleryBody, serializeGallery, type GalleryItem } from "@/lib/gallery";

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    gallery: {
      /** Inserts a gallery block with at most GALLERY_MAX items. */
      insertGallery: (items: GalleryItem[]) => ReturnType;
    };
  }
}

/** Keeps only well-formed items and clamps to GALLERY_MAX. */
export function clampGalleryItems(items: unknown): GalleryItem[] {
  if (!Array.isArray(items)) return [];
  const out: GalleryItem[] = [];
  for (const raw of items) {
    if (out.length >= GALLERY_MAX) break;
    if (!raw || typeof raw !== "object") continue;
    const r = raw as Partial<GalleryItem>;
    if (typeof r.url !== "string" || r.url === "") continue;
    out.push({
      url: r.url,
      alt: typeof r.alt === "string" ? r.alt : "",
      title: typeof r.title === "string" ? r.title : undefined,
      sensitive: r.sensitive === true,
      categories: Array.isArray(r.categories)
        ? r.categories.filter((c): c is string => typeof c === "string")
        : [],
    });
  }
  return out;
}

function encodeItems(items: GalleryItem[]): string {
  return JSON.stringify(items);
}

function decodeItems(value: string | null): GalleryItem[] {
  if (!value) return [];
  try {
    return clampGalleryItems(JSON.parse(value));
  } catch {
    return [];
  }
}

function escapeAttr(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

const PATCHED = Symbol.for("noct.gallery.fence");

type FenceRule = (tokens: unknown[], idx: number, ...rest: unknown[]) => string;
type MdLike = { renderer: { rules: { fence?: FenceRule } } };
type FenceToken = { info?: string; content: string };

/** Wraps markdown-it's fence rule once: a `gallery` fence becomes a div the node parses. */
function setupGalleryFence(md: MdLike) {
  const rules = md.renderer.rules as { fence?: FenceRule } & { [PATCHED]?: boolean };
  if (rules[PATCHED]) return;
  const original = rules.fence;
  rules.fence = (tokens, idx, ...rest) => {
    const token = tokens[idx] as FenceToken;
    if ((token.info ?? "").trim() === "gallery") {
      const items = parseGalleryBody(token.content);
      return `<div data-type="gallery" data-items="${escapeAttr(encodeItems(items))}"></div>`;
    }
    return original ? original(tokens, idx, ...rest) : "";
  };
  rules[PATCHED] = true;
}

const markdown: MarkdownNodeSpec = {
  serialize(state, node) {
    const items = clampGalleryItems(node.attrs.items);
    const text = serializeGallery(items);
    if (text === "") {
      // Empty gallery: emit nothing; avoid a dangling block separator.
      state.closeBlock(node);
      return;
    }
    // The stock image serializer does not close its block: a gallery right
    // after a block image would be glued to it. Insert the blank line here.
    const st = state as unknown as { out: string; closed: unknown };
    if (st.out !== "" && !st.closed) {
      state.ensureNewLine();
      state.write("\n");
    }
    state.write(text);
    state.closeBlock(node);
  },
  parse: {
    setup(md) {
      setupGalleryFence(md as unknown as MdLike);
    },
  },
};

export const GalleryExtension = Node.create({
  name: "gallery",
  group: "block",
  atom: true,
  selectable: true,
  draggable: true,

  addAttributes() {
    return {
      items: {
        default: [] as GalleryItem[],
        parseHTML: (el) => decodeItems(el.getAttribute("data-items")),
        renderHTML: (attrs) => ({ "data-items": encodeItems(clampGalleryItems(attrs.items)) }),
      },
    };
  },

  parseHTML() {
    return [{ tag: 'div[data-type="gallery"]' }];
  },

  renderHTML({ node, HTMLAttributes }) {
    const items = clampGalleryItems(node.attrs.items);
    return [
      "div",
      mergeAttributes(HTMLAttributes, { "data-type": "gallery" }),
      ...items.map((it) => ["img", { src: it.url, alt: it.alt }] as [string, Record<string, string>]),
    ];
  },

  addCommands() {
    return {
      insertGallery:
        (items) =>
        ({ commands }) => {
          const clamped = clampGalleryItems(items);
          if (clamped.length === 0) return false;
          return commands.insertContent({ type: this.name, attrs: { items: clamped } });
        },
    };
  },

  addStorage() {
    return { markdown };
  },
});
