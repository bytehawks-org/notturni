import { NextRequest } from "next/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { needsPrimarySlug, soloRoute } from "@/lib/solo-routing";

const PAGES = ["/search", "/media", "/link", "/bibliografia", "/pub", "/pub/bar", "/pagina/bar"];
const FILES = ["/feed.xml", "/atom.xml", "/sitemap.xml", "/robots.txt"];

describe("soloRoute", () => {
  it.each(PAGES)("rewrites %s to the primary blog route", (path) => {
    expect(soloRoute(path, "foo")).toEqual({ type: "rewrite", pathname: `/foo${path}` });
    expect(needsPrimarySlug(path)).toBe(true);
  });

  it.each([...PAGES, ...FILES])("redirects /foo%s to the root", (path) => {
    expect(soloRoute(`/foo${path}`, "foo")).toEqual({ type: "redirect", pathname: path });
    expect(needsPrimarySlug(`/foo${path}`)).toBe(true);
  });

  it.each(FILES)("never rewrites the root %s", (path) => {
    expect(soloRoute(path, "foo")).toEqual({ type: "none" });
  });

  it.each([
    "/",
    "/foo",
    "/foo/a-post",
    "/post/a-post",
    "/bar/media",
    "/bar/feed.xml",
    "/searching",
    "/media/x",
    "/pagina",
    "/foobar/media",
    "/foo//example.org",
    "//example.org",
    "//example.org/x",
    "/foo/feed.xml/x",
    "/login",
  ])("leaves %s alone", (path) => {
    expect(soloRoute(path, "foo")).toEqual({ type: "none" });
  });

  it("does nothing without a primary slug", () => {
    for (const path of [...PAGES, "/foo/media"]) expect(soloRoute(path, null)).toEqual({ type: "none" });
  });

  it.each(["search", "media", "link", "bibliografia", "pub", "pagina"])(
    "cannot loop when the primary slug is %s",
    (primary) => {
      for (const start of [...PAGES, ...PAGES.map((p) => `/${primary}${p}`)]) {
        let path = start;
        let route = soloRoute(path, primary);
        while (route.type === "redirect") {
          expect(route.pathname.length).toBeLessThan(path.length);
          path = route.pathname;
          route = soloRoute(path, primary);
        }
      }
      expect(soloRoute(`/${primary}/media`, primary)).toEqual({ type: "redirect", pathname: "/media" });
    },
  );
});

describe("proxy in solo mode", () => {
  const config = () => new Response(JSON.stringify({ primary_blog_slug: "foo" }), { status: 200 });
  let fetchMock: ReturnType<typeof vi.fn<(url: string, init?: RequestInit) => Promise<Response>>>;

  /** A new module instance for each test: the slug cache starts empty. */
  async function call(path: string, host = "example.com") {
    const { proxy } = await import("@/proxy");
    return await proxy(new NextRequest(`http://${host}${path}`, { headers: { host } }));
  }

  beforeEach(() => {
    vi.resetModules();
    vi.stubEnv("NOCT_SITE_URL", "https://example.com");
    vi.stubEnv("NOCT_API_PUBLIC_URL", "https://example.com");
    vi.stubEnv("NOCT_BACKEND_INTERNAL_URL", "http://backend.example.com");
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "solo");
    vi.stubEnv("NOCT_SUBDOMAINS_ENABLED", "false");
    fetchMock = vi.fn(async () => config());
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
  });

  it("rewrites with the query string, the CSP and the nonce", async () => {
    const res = await call("/search?q=bar");
    expect(res.headers.get("x-middleware-rewrite")).toBe("http://example.com/foo/search?q=bar");
    const nonce = res.headers.get("x-middleware-request-x-nonce");
    expect(nonce).toBeTruthy();
    expect(res.headers.get("Content-Security-Policy")).toContain(`'nonce-${nonce}'`);
    expect(fetchMock.mock.calls[0][0]).toBe("http://backend.example.com/api/v1/config");
  });

  it("answers 308 on the same origin, with the query string and the CSP", async () => {
    const res = await call("/foo/media?category=bar");
    expect(res.status).toBe(308);
    expect(res.headers.get("location")).toBe("http://example.com/media?category=bar");
    expect(res.headers.get("Content-Security-Policy")).toContain("'nonce-");
  });

  it("does not call the backend for unrelated paths", async () => {
    await call("/post/bar");
    await call("/feed.xml");
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("caches the slug for one TTL", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    await call("/media");
    await call("/link");
    expect(fetchMock).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(31_000);
    await call("/media");
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it.each([
    ["network error", () => Promise.reject(new Error("down"))],
    ["HTTP error", async () => new Response("", { status: 503 })],
    ["no primary slug", async () => new Response(JSON.stringify({ primary_blog_slug: null }))],
  ])("passes through and does not cache: %s", async (_name, failure) => {
    fetchMock.mockImplementationOnce(failure);
    const first = await call("/media");
    expect(first.headers.get("x-middleware-next")).toBe("1");
    const second = await call("/media");
    expect(second.headers.get("x-middleware-rewrite")).toContain("/foo/media");
  });

  it.each(["192.168.1.10", "localhost", "www.example.com", "foo.example.com"])(
    "routes the root paths on %s when subdomains are disabled",
    async (host) => {
      expect((await call("/media", host)).headers.get("x-middleware-rewrite")).toBe(`http://${host}/foo/media`);
      const old = await call("/foo/media", host);
      expect(old.status).toBe(308);
      expect(old.headers.get("location")).toBe(`http://${host}/media`);
    },
  );

  it.each(["192.168.1.10", "localhost", "www.example.com"])(
    "routes only the apex when subdomains are enabled, not %s",
    async (host) => {
      vi.stubEnv("NOCT_SUBDOMAINS_ENABLED", "true");
      expect((await call("/media", host)).headers.get("x-middleware-next")).toBe("1");
      expect((await call("/foo/media", host)).headers.get("x-middleware-next")).toBe("1");
      expect(fetchMock).not.toHaveBeenCalled();
      expect((await call("/media")).headers.get("x-middleware-rewrite")).toContain("/foo/media");
    },
  );

  it("is inactive in platform mode", async () => {
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "platform");
    for (const path of ["/media", "/foo/media"]) {
      expect((await call(path)).headers.get("x-middleware-next")).toBe("1");
    }
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("keeps the subdomain rewrite, without redirects", async () => {
    vi.stubEnv("NOCT_SUBDOMAINS_ENABLED", "true");
    const root = await call("/media", "bar.example.com");
    expect(root.headers.get("x-middleware-rewrite")).toContain("/bar/media");
    const prefixed = await call("/foo/media", "foo.example.com");
    expect(prefixed.status).toBe(200);
    expect(prefixed.headers.get("x-middleware-rewrite")).toContain("/foo/foo/media");
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
