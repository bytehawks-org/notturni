import { describe, expect, it, vi } from "vitest";

vi.mock("server-only", () => ({}));
vi.mock("./shiki-highlighter", () => ({
  ensureLanguagesLoaded: async () => ({}),
  highlightCode: (_h: unknown, code: string) => `<pre><code>${code}</code></pre>`,
}));

import { renderMarkdown } from "./markdown";

const lines = (n: number, extra: (i: number) => string = () => "") =>
  Array.from({ length: n }, (_, i) => `![a${i}](/m/${i}.jpg${extra(i)})`).join("\n");
const gal = (n: number, extra?: (i: number) => string) => "```gallery\n" + lines(n, extra) + "\n```";
const parse = async (md: string, opts = {}) => {
  const d = document.implementation.createHTMLDocument("");
  d.body.innerHTML = await renderMarkdown(md, opts);
  return d;
};

describe("gallery fence", () => {
  it.each([
    [1, "n1", 1, 0],
    [2, "n2", 2, 0],
    [3, "n3", 3, 0],
    [4, "n4", 4, 0],
    [5, "n4", 5, 1],
  ])("N=%i", async (n, mod, tiles, hidden) => {
    const d = await parse(gal(n));
    const g = d.querySelector(".post-gallery")!;
    expect(g.classList.contains(`post-gallery--${mod}`)).toBe(true);
    expect(g.getAttribute("role")).toBe("group");
    expect(g.getAttribute("aria-label")).toBe("Gallery");
    expect(g.querySelectorAll(".post-gallery__tile")).toHaveLength(tiles);
    expect(g.querySelectorAll(".post-gallery__tile--hidden")).toHaveLength(hidden);
    expect(g.querySelectorAll(".post-gallery__wrapper, .sensitive-image-wrapper")).toHaveLength(0);
    const img = g.querySelector("img")!;
    expect(img.getAttribute("data-lightbox")).toBe("1");
    expect(img.getAttribute("loading")).toBe("lazy");
    expect(img.getAttribute("data-gallery-index")).toBe("0");
    if (n === 5) {
      expect(g.querySelector(".post-gallery__more .post-gallery__count")!.textContent).toBe("+1");
      expect(g.querySelector(".post-gallery__count")!.getAttribute("aria-hidden")).toBe("true");
      expect(g.querySelectorAll(".post-gallery__tile")[3].classList.contains("post-gallery__more")).toBe(true);
    } else {
      expect(g.querySelector(".post-gallery__count")).toBeNull();
    }
  });

  it("12 lines render 10 tiles with +6", async () => {
    const d = await parse(gal(12));
    expect(d.querySelectorAll(".post-gallery__tile")).toHaveLength(10);
    expect(d.querySelector(".post-gallery__count")!.textContent).toBe("+6");
  });

  it("only the sensitive tile gets the wrapper", async () => {
    const d = await parse(gal(3, (i) => (i === 1 ? ' "sensitive:nudity"' : "")));
    const tiles = d.querySelectorAll(".post-gallery__tile");
    expect(tiles[1].querySelector(".sensitive-image-wrapper img")).not.toBeNull();
    expect(tiles[1].querySelector("img")!.hasAttribute("data-lightbox")).toBe(false);
    expect(tiles[1].querySelector(".lightbox-expand-btn")!.getAttribute("data-gallery-index")).toBe("1");
    expect(tiles[0].querySelector(".sensitive-image-wrapper")).toBeNull();
    expect(tiles[2].querySelector(".sensitive-image-wrapper")).toBeNull();
    expect(d.querySelectorAll(".sensitive-image-wrapper")).toHaveLength(1);
  });

  it("two galleries get different ids; custom label", async () => {
    const d = await parse(gal(2) + "\n\n" + gal(3), { galleryLabel: "Foto" });
    const gs = d.querySelectorAll(".post-gallery");
    expect(gs[0].getAttribute("data-gallery")).not.toBe(gs[1].getAttribute("data-gallery"));
    expect(gs[0].getAttribute("aria-label")).toBe("Foto");
  });

  it("sanitizes XSS in alt, url and title", async () => {
    const md =
      "```gallery\n" +
      '![x" onerror="alert(1)](/a.jpg "sensitive\\" onerror=\\"alert(2)")\n' +
      "![b](javascript:alert(3))\n" +
      "![c](/c.jpg)\n```";
    const html = await renderMarkdown(md);
    const d = await parse(md);
    expect(html).not.toMatch(/javascript:/i);
    d.querySelectorAll("*").forEach((el) => {
      expect(el.getAttributeNames().some((a) => a.startsWith("on"))).toBe(false);
    });
  });

  it("leaves plain images and other fences unchanged", async () => {
    const d = await parse("![p](/p.jpg)");
    expect(d.querySelector(".post-gallery")).toBeNull();
    expect(d.querySelector("img")!.getAttribute("data-lightbox")).toBe("1");
    expect(d.querySelector(".sensitive-image-wrapper")).toBeNull();
    const c = await parse("```js\nlet a = 1;\n```");
    expect(c.querySelector("pre")).not.toBeNull();
    expect(c.querySelector(".post-gallery")).toBeNull();
  });
});
