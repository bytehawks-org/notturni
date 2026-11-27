import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("server-only", () => ({}));

import { blogLinks } from "@/lib/blog-path";
import { getPrimaryBlogSlug } from "@/lib/server-api";
import { mockHeaders } from "@/test/next-mocks";

const SOLO = { NOCT_SITE_URL: "https://example.com", NOCT_DEPLOYMENT_MODE: "solo", NOCT_SUBDOMAINS_ENABLED: "" };

beforeEach(() => {
  for (const [k, v] of Object.entries(SOLO)) vi.stubEnv(k, v);
});

afterEach(() => {
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
});

describe("blogLinks with a primary blog", () => {
  it("solo, primary blog on the apex: sub-pages are at the root, home is / and /post/... permalinks are untouched", async () => {
    mockHeaders.set("host", "example.com");
    const links = await blogLinks("foo", "foo");
    expect(links.basePath).toBe("");
    expect(links.homeHref).toBe("/");
    expect(links.fromPermalink("/post/post-1")).toBe("/post/post-1");
  });

  it("solo, primary blog on its subdomain: /post/... permalinks are untouched", async () => {
    mockHeaders.set("host", "foo.example.com");
    const links = await blogLinks("foo", "foo");
    expect(links.basePath).toBe("");
    expect(links.homeHref).toBe("/");
    expect(links.fromPermalink("/post/post-1")).toBe("/post/post-1");
  });

  it("solo, non-primary blog keeps the path-based base path", async () => {
    mockHeaders.set("host", "example.com");
    const links = await blogLinks("bar", "foo");
    expect(links.basePath).toBe("/bar");
    expect(links.homeHref).toBe("/bar");
    expect(links.fromPermalink("/bar/post-1")).toBe("/bar/post-1");
  });

  it("solo, primary slug null keeps the old outputs", async () => {
    mockHeaders.set("host", "example.com");
    const links = await blogLinks("foo", null);
    expect(links.basePath).toBe("/foo");
    expect(links.fromPermalink("/foo/post-1")).toBe("/foo/post-1");
  });

  it("platform, primary slug ignored: outputs are the old ones", async () => {
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "platform");
    mockHeaders.set("host", "example.com");
    const links = await blogLinks("foo", "foo");
    expect(links.basePath).toBe("/foo");
    expect(links.fromPermalink("/foo/post-1")).toBe("/foo/post-1");
  });
});

describe("getPrimaryBlogSlug", () => {
  it("platform: returns null without calling the backend", async () => {
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "platform");
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    expect(await getPrimaryBlogSlug()).toBeNull();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("solo: returns primary_blog_slug from GET /api/v1/config", async () => {
    const fetchMock = vi.fn(async (_url: string) => new Response(JSON.stringify({ primary_blog_slug: "foo" }), { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);
    expect(await getPrimaryBlogSlug()).toBe("foo");
    expect(String(fetchMock.mock.calls[0]?.[0])).toMatch(/\/api\/v1\/config$/);
  });

  it("solo: null when the backend reports no primary blog", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({ primary_blog_slug: null }), { status: 200 })));
    expect(await getPrimaryBlogSlug()).toBeNull();
  });

  it("solo: null when the request fails or returns an error status", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => Promise.reject(new Error("network"))));
    expect(await getPrimaryBlogSlug()).toBeNull();
    vi.stubGlobal("fetch", vi.fn(async () => new Response("err", { status: 500 })));
    expect(await getPrimaryBlogSlug()).toBeNull();
  });

  it("solo: revalidates within a few seconds, so a null slug is not cached for long", async () => {
    const fetchMock = vi.fn(async (...args: unknown[]) => (args.length >= 0 ? new Response(JSON.stringify({ primary_blog_slug: null }), { status: 200 }) : undefined));
    vi.stubGlobal("fetch", fetchMock);
    await getPrimaryBlogSlug();
    const init = fetchMock.mock.calls[0]?.[1] as { next?: { revalidate?: number } };
    expect(init.next?.revalidate).toBeLessThanOrEqual(5);
  });
});
