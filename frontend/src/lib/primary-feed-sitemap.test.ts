import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("server-only", () => ({}));
vi.mock("@/lib/server-api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/server-api")>()),
  getPrimaryBlogSlug: vi.fn(async () => "foo"),
}));

import { getPrimarySitemapExtras } from "@/lib/primary-feed";

const SOLO = { NOCT_SITE_URL: "https://example.com", NOCT_DEPLOYMENT_MODE: "solo", NOCT_SUBDOMAINS_ENABLED: "" };

// Synthetic sitemap: one global-looking path and two primary-only paths.
const SITEMAP = {
  entries: [
    { path: "/pagina/about", lastmod: null },
    { path: "/pub/book", lastmod: null },
    { path: "/post/post-1", lastmod: null },
  ],
};

beforeEach(() => {
  for (const [k, v] of Object.entries(SOLO)) vi.stubEnv(k, v);
});

afterEach(() => {
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
});

describe("getPrimarySitemapExtras", () => {
  it("reads the primary sitemap without a time-based cache", async () => {
    const fetchMock = vi.fn<(url: string, init?: RequestInit & { next?: unknown }) => Promise<Response>>(
      async () => new Response(JSON.stringify(SITEMAP), { status: 200 }),
    );
    vi.stubGlobal("fetch", fetchMock);
    await getPrimarySitemapExtras();
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const init = fetchMock.mock.calls[0]?.[1] ?? {};
    expect(init.cache).toBe("no-store");
    expect(init.next).toBeUndefined();
  });

  it("returns only the /pagina and /pub entries", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify(SITEMAP), { status: 200 })));
    const extras = await getPrimarySitemapExtras();
    expect(extras.map((e) => e.path)).toEqual(["/pagina/about", "/pub/book"]);
  });

  it("after the primary stops being indexable (404) no stale entries are served", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response(JSON.stringify(SITEMAP), { status: 200 }))
      .mockResolvedValueOnce(new Response("", { status: 404 }));
    vi.stubGlobal("fetch", fetchMock);
    expect((await getPrimarySitemapExtras()).length).toBe(2);
    expect(await getPrimarySitemapExtras()).toEqual([]);
  });
});
