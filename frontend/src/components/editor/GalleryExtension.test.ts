import { describe, expect, it } from "vitest";
import { Editor } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import ImageExtension from "@tiptap/extension-image";
import { Markdown, type MarkdownStorage } from "tiptap-markdown";
import { GalleryExtension, clampGalleryItems, insertGalleryBlock } from "./GalleryExtension";
import type { GalleryItem } from "@/lib/gallery";

function roundTrip(md: string): string {
  const editor = new Editor({
    extensions: [
      StarterKit,
      ImageExtension,
      GalleryExtension,
      Markdown.configure({ html: false, linkify: true, tightLists: true }),
    ],
    content: md,
  });
  const out = (editor.storage as unknown as { markdown: MarkdownStorage }).markdown.getMarkdown();
  editor.destroy();
  return out;
}

const img = (n: number, extra = "") => `![a${n}](/m/${n}.png${extra})`;
const gal = (lines: string[]) => ["```gallery", ...lines, "```"].join("\n");

describe("GalleryExtension markdown round-trip", () => {
  it("2 images", () => {
    const md = gal([img(1), img(2)]);
    expect(roundTrip(md)).toBe(md);
  });
  it("10 images", () => {
    const md = gal(Array.from({ length: 10 }, (_, i) => img(i)));
    expect(roundTrip(md)).toBe(md);
  });
  it("sensitive items with categories", () => {
    const md = gal([
      img(1, ' "sensitive"'),
      img(2, ' "sensitive:nsfw,violence"'),
      img(3),
    ]);
    expect(roundTrip(md)).toBe(md);
  });
  it("alt text with special characters", () => {
    const md = gal(['![a "q" & <b> it\'s](/m/1.png)', img(2)]);
    expect(roundTrip(md)).toBe(md);
  });
  it("brackets in alt are dropped, not corrupting", () => {
    const out = roundTrip(gal(["![a\\]b](/m/1.png)", img(2)]));
    expect(out).toBe(gal(["![ab](/m/1.png)", img(2)]));
  });
  it("gallery between paragraphs", () => {
    const md = `before\n\n${gal([img(1), img(2)])}\n\nafter`;
    expect(roundTrip(md)).toBe(md);
  });
  it("two galleries in a row", () => {
    const md = `${gal([img(1), img(2)])}\n\n${gal([img(3), img(4), img(5)])}`;
    expect(roundTrip(md)).toBe(md);
  });
  it("ordinary image next to a gallery stays an image", () => {
    const md = `${img(9)}\n\n${gal([img(1), img(2)])}\n\n${img(8)}`;
    expect(roundTrip(md)).toBe(md);
  });
  it("normal code fence stays code", () => {
    const md = "```js\nconst a = 1;\n```";
    expect(roundTrip(md)).toBe(md);
  });
  it("unlabeled fence stays code", () => {
    const md = "```\nplain\n```";
    expect(roundTrip(md)).toBe(md);
  });
  it("gallery fence with a single image serializes as a plain image", () => {
    expect(roundTrip(gal([img(1)]))).toBe(img(1));
  });
  it("parsing repeatedly does not stack fence wrappers", () => {
    const md = gal([img(1), img(2)]);
    expect(roundTrip(md)).toBe(md);
    expect(roundTrip(md)).toBe(md);
  });
});

describe("insertGallery and clamp", () => {
  const item = (n: number): GalleryItem => ({
    url: `/m/${n}.png`,
    alt: `a${n}`,
    sensitive: false,
    categories: [],
  });
  it("clamps to 10", () => {
    expect(clampGalleryItems(Array.from({ length: 15 }, (_, i) => item(i)))).toHaveLength(10);
  });
  it("insertGallery inserts a clamped node", () => {
    const editor = new Editor({
      extensions: [StarterKit, ImageExtension, GalleryExtension, Markdown],
      content: "",
    });
    editor.commands.insertGallery(Array.from({ length: 12 }, (_, i) => item(i)));
    const md = (editor.storage as unknown as { markdown: MarkdownStorage }).markdown.getMarkdown();
    editor.destroy();
    expect(md).toBe(gal(Array.from({ length: 10 }, (_, i) => img(i))));
  });
});

describe("insertGalleryBlock", () => {
  const items: GalleryItem[] = [1, 2].map((n) => ({ url: `/m/${n}.png`, alt: "", sensitive: false, categories: [] }));

  it.each([
    ["empty document", ""],
    ["after text", "before"],
  ])("keeps the gallery and adds a paragraph after it (%s)", (_name, content) => {
    const editor = new Editor({ extensions: [StarterKit, GalleryExtension, Markdown], content });
    editor.commands.setTextSelection(editor.state.doc.content.size);

    insertGalleryBlock(editor, items);

    const types = editor.getJSON().content?.map((n) => n.type) ?? [];
    expect(types.filter((t) => t === "gallery")).toHaveLength(1);
    expect(types[types.indexOf("gallery") + 1]).toBe("paragraph");
    editor.destroy();
  });
});
