import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("server-only", () => ({}));
vi.mock("@/lib/server-api", () => ({
  getSitemapPage: vi.fn(),
  getPrimaryBlogSlug: vi.fn(async () => null),
  getSpaceSitemap: vi.fn(async () => null),
}));

import { GET } from "@/app/sitemap.xml/route";
import { GET as GET_CHUNK } from "@/app/sitemaps/[file]/route";
import { getPrimaryBlogSlug, getSitemapPage, getSpaceSitemap } from "@/lib/server-api";
import type { SitemapEntry } from "@/lib/types";

const entry = (path: string): SitemapEntry => ({ path, updated_at: null, alternates: [] });

/** Fake backend: `GET /api/v1/seo/sitemap` over a global list, with the real semantics (`total: 0` past the end). */
function fakeGlobal(paths: string[]) {
  return async (offset: number, limit: number) => {
    const slice = paths.slice(offset, offset + limit).map(entry);
    return { total: offset >= paths.length ? 0 : paths.length, noindex: false, entries: slice };
  };
}

const locs = (xml: string): string[] => [...xml.matchAll(/<loc>([^<]*)<\/loc>/g)].map((m) => m[1]);

beforeEach(() => {
  vi.clearAllMocks();
  vi.stubEnv("NOCT_SITE_URL", "https://example.com");
});

describe("root sitemap, solo primary blog", () => {
  it("adds the primary /pagina/.. and /pub/.. at root URLs, never /<primary>/", async () => {
    vi.mocked(getSitemapPage).mockImplementation(fakeGlobal(["/", "/p/x", "/altro", "/post/ciao"]));
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue("foo");
    vi.mocked(getSpaceSitemap).mockResolvedValue({
      slug: "foo",
      entries: [entry("/"), entry("/post/ciao"), entry("/pagina/about"), entry("/pub/libro")],
    });

    const xml = await (await GET()).text();
    const urls = locs(xml);

    expect(getSpaceSitemap).toHaveBeenCalledWith("foo");
    expect(urls).toContain("https://example.com/pagina/about");
    expect(urls).toContain("https://example.com/pub/libro");
    expect(urls.some((u) => u.includes("/foo/") || u.endsWith("/foo"))).toBe(false);
    expect(new Set(urls).size).toBe(urls.length);
  });

  it("platform (no primary slug): output is the global list only", async () => {
    vi.mocked(getSitemapPage).mockImplementation(fakeGlobal(["/", "/altro"]));
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue(null);

    const urls = locs(await (await GET()).text());

    expect(getSpaceSitemap).not.toHaveBeenCalled();
    expect(urls).toEqual(["https://example.com/", "https://example.com/altro"]);
  });

  it("primary not indexable (space sitemap null): adds nothing", async () => {
    vi.mocked(getSitemapPage).mockImplementation(fakeGlobal(["/"]));
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue("foo");
    vi.mocked(getSpaceSitemap).mockResolvedValue(null);

    const urls = locs(await (await GET()).text());

    expect(urls).toEqual(["https://example.com/"]);
  });

  it("counts the extras in the chunking: index when global + extras exceed one page", async () => {
    const global = Array.from({ length: 4999 }, (_, i) => `/post/p${i}`);
    vi.mocked(getSitemapPage).mockImplementation(fakeGlobal(global));
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue("foo");
    vi.mocked(getSpaceSitemap).mockResolvedValue({
      slug: "foo",
      entries: [entry("/pagina/a"), entry("/pagina/b"), entry("/pub/c")],
    });

    const xml = await (await GET()).text();
    expect(xml).toContain("<sitemapindex");
    expect(locs(xml)).toEqual(["https://example.com/sitemaps/0.xml", "https://example.com/sitemaps/1.xml"]);

    // Page 1 holds the global tail after the 4999 global entries: the extras from index 1 on.
    const page1 = await (await GET_CHUNK(new Request("https://example.com/sitemaps/1.xml"), { params: Promise.resolve({ file: "1.xml" }) })).text();
    expect(locs(page1)).toEqual(["https://example.com/pagina/b", "https://example.com/pub/c"]);
  });

  it("chunk past the whole list (global and extras): 404", async () => {
    vi.mocked(getSitemapPage).mockImplementation(fakeGlobal(["/"]));
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue("foo");
    vi.mocked(getSpaceSitemap).mockResolvedValue({ slug: "foo", entries: [entry("/pagina/a")] });

    const res = await GET_CHUNK(new Request("https://example.com/sitemaps/9.xml"), { params: Promise.resolve({ file: "9.xml" }) });
    expect(res.status).toBe(404);
  });
});
