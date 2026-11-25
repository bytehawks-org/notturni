import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("server-only", () => ({}));
vi.mock("next/navigation", () => ({
  permanentRedirect: vi.fn((url: string) => {
    throw new Error(`REDIRECT:${url}`);
  }),
  notFound: vi.fn(),
}));
vi.mock("@/components/blog/BlogHome", () => ({
  BlogHome: function BlogHome() {
    return null;
  },
  blogHomeMetadata: vi.fn(async () => ({})),
}));
vi.mock("@/lib/server-api", () => ({
  getPrimaryBlogSlug: vi.fn(async () => null),
}));

import BlogHomePage from "@/app/[blogSlug]/page";
import { BlogHome } from "@/components/blog/BlogHome";
import { getPrimaryBlogSlug } from "@/lib/server-api";
import { mockHeaders } from "@/test/next-mocks";

const SOLO = { NOCT_SITE_URL: "https://example.com", NOCT_DEPLOYMENT_MODE: "solo", NOCT_SUBDOMAINS_ENABLED: "" };

const render = (blogSlug: string, query: Record<string, string> = {}) =>
  BlogHomePage({ params: Promise.resolve({ blogSlug }), searchParams: Promise.resolve(query) });

beforeEach(() => {
  for (const [k, v] of Object.entries(SOLO)) vi.stubEnv(k, v);
  mockHeaders.set("host", "example.com");
});

afterEach(() => {
  vi.unstubAllEnvs();
  mockHeaders.clear();
  vi.clearAllMocks();
});

describe("/{blogSlug} home", () => {
  it("solo, primary blog on the apex: /{primary} redirects permanently to /", async () => {
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue("foo");
    await expect(render("foo")).rejects.toThrow("REDIRECT:/");
  });

  it("solo, primary blog: the category filter is kept in the redirect", async () => {
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue("foo");
    await expect(render("foo", { category: "racconti" })).rejects.toThrow("REDIRECT:/?category=racconti");
  });

  it("solo, primary blog on its subdomain: no redirect (the proxy rewrites / to /{slug}, a redirect would loop)", async () => {
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue("foo");
    mockHeaders.set("host", "foo.example.com");
    const element = (await render("foo")) as { type: unknown };
    expect(element.type).toBe(BlogHome);
  });

  it("solo, non-primary blog: rendered, no redirect", async () => {
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue("foo");
    const element = (await render("bar")) as { type: unknown };
    expect(element.type).toBe(BlogHome);
  });

  it("platform: no redirect even if the slug matches", async () => {
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "platform");
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue(null);
    const element = (await render("foo")) as { type: unknown };
    expect(element.type).toBe(BlogHome);
  });
});
