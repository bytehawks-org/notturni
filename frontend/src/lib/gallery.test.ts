import { describe, expect, it } from "vitest";

import {
  GALLERY_MAX,
  galleryLayout,
  parseGalleryBody,
  serializeGallery,
  type GalleryItem,
} from "@/lib/gallery";

const plain: GalleryItem = {
  url: "https://example.com/a.jpg",
  alt: "one",
  sensitive: false,
  categories: [],
};

const sensitive: GalleryItem = {
  url: "https://example.com/b.jpg",
  alt: "two",
  title: "sensitive:nudity",
  sensitive: true,
  categories: ["nudity"],
};

describe("parseGalleryBody", () => {
  it("returns an empty list for an empty body", () => {
    expect(parseGalleryBody("")).toEqual([]);
    expect(parseGalleryBody("\n  \n")).toEqual([]);
  });

  it("parses plain and sensitive image lines", () => {
    const body = `![one](https://example.com/a.jpg)\n![two](https://example.com/b.jpg "sensitive:nudity")`;
    expect(parseGalleryBody(body)).toEqual([
      { url: plain.url, alt: "one", title: undefined, sensitive: false, categories: [] },
      { url: sensitive.url, alt: "two", title: "sensitive:nudity", sensitive: true, categories: ["nudity"] },
    ]);
  });

  it("reads sensitive with several categories and without categories", () => {
    const body = `![a](https://example.com/a.jpg "sensitive:nudity, gore")\n![b](https://example.com/b.jpg "sensitive")`;
    const [a, b] = parseGalleryBody(body);
    expect(a.sensitive).toBe(true);
    expect(a.categories).toEqual(["nudity", "gore"]);
    expect(b.sensitive).toBe(true);
    expect(b.categories).toEqual([]);
  });

  it("ignores garbage and non-image lines", () => {
    const body = [
      "Testo libero",
      "![ok](https://example.com/a.jpg)",
      "[link](https://example.com)",
      "![rotto](https://example.com/x.jpg",
      "",
      "   ",
      "![due](https://example.com/b.jpg)",
    ].join("\n");
    expect(parseGalleryBody(body).map((i) => i.alt)).toEqual(["ok", "due"]);
  });

  it("caps the result at GALLERY_MAX items", () => {
    expect(GALLERY_MAX).toBe(10);
    const body = Array.from({ length: 12 }, (_, i) => `![img${i}](https://example.com/${i}.jpg)`).join("\n");
    const items = parseGalleryBody(body);
    expect(items).toHaveLength(10);
    expect(items[9].alt).toBe("img9");
  });

  it("accepts an escaped bracket in alt text", () => {
    const [item] = parseGalleryBody(String.raw`![a \] b](https://example.com/a.jpg)`);
    expect(item.alt).toBe("a ] b");
  });
});

describe("serializeGallery", () => {
  it("emits a fenced block for two or more items", () => {
    expect(serializeGallery([plain, sensitive])).toBe(
      [
        "```gallery",
        "![one](https://example.com/a.jpg)",
        '![two](https://example.com/b.jpg "sensitive:nudity")',
        "```",
      ].join("\n"),
    );
  });

  it("emits a plain image line for a single item", () => {
    expect(serializeGallery([plain])).toBe("![one](https://example.com/a.jpg)");
  });

  it("emits an empty string for no items", () => {
    expect(serializeGallery([])).toBe("");
  });

  it("does not emit a title for non-sensitive items", () => {
    const out = serializeGallery([plain, plain]);
    expect(out).not.toContain('"');
  });

  it("emits bare sensitive title when there are no categories", () => {
    const s: GalleryItem = { ...plain, sensitive: true, categories: [] };
    expect(serializeGallery([plain, s])).toContain('![one](https://example.com/a.jpg "sensitive")');
  });

  it("drops brackets from alt text (backend regex does not accept them)", () => {
    const item: GalleryItem = { ...plain, alt: "a ] b [c" };
    expect(serializeGallery([item, plain])).toContain("![a  b c](https://example.com/a.jpg)");
  });

  it("drops items beyond GALLERY_MAX", () => {
    const many = Array.from({ length: 12 }, (_, i) => ({ ...plain, alt: `img${i}` }));
    const lines = serializeGallery(many).split("\n");
    expect(lines).toHaveLength(12); // fence open, 10 images, fence close
  });

  it("round-trips through parseGalleryBody", () => {
    const items: GalleryItem[] = [
      plain,
      { ...sensitive, alt: "x y", categories: ["nudity", "gore"], title: "sensitive:nudity,gore" },
      { ...plain, url: "https://example.com/Esempio_(disambigua)", alt: "c" },
    ];
    const block = serializeGallery(items);
    const body = block.replace(/^```gallery\n/, "").replace(/\n```$/, "");
    expect(parseGalleryBody(body)).toEqual(
      items.map((i) => ({
        url: i.url,
        alt: i.alt,
        title: i.sensitive ? i.title : undefined,
        sensitive: i.sensitive,
        categories: i.categories,
      })),
    );
  });
});

describe("galleryLayout", () => {
  it.each([
    [1, "n1", 1, 0],
    [2, "n2", 2, 0],
    [3, "n3", 3, 0],
    [4, "n4", 4, 0],
    [5, "n4", 4, 1],
    [10, "n4", 4, 6],
  ] as const)("n=%i -> modifier %s, visible %i, overflow %i", (n, modifier, visible, overflow) => {
    expect(galleryLayout(n)).toEqual({ modifier, visible, overflow });
  });

  it("treats zero images as an empty n1 layout", () => {
    expect(galleryLayout(0)).toEqual({ modifier: "n1", visible: 0, overflow: 0 });
  });
});
