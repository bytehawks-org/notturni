import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("server-only", () => ({}));
vi.mock("@/lib/server-api", () => ({
  getPrimaryBlogSlug: vi.fn(async () => null),
  getPublicBlog: vi.fn(async () => null),
  getPublicBlogPosts: vi.fn(async () => []),
}));

import { getPrimaryBlogFeed } from "@/lib/primary-feed";
import { getPrimaryBlogSlug, getPublicBlog, getPublicBlogPosts } from "@/lib/server-api";

const SOLO = { NOCT_SITE_URL: "https://example.com", NOCT_DEPLOYMENT_MODE: "solo", NOCT_SUBDOMAINS_ENABLED: "" };

beforeEach(() => {
  for (const [k, v] of Object.entries(SOLO)) vi.stubEnv(k, v);
});

afterEach(() => {
  vi.unstubAllEnvs();
  vi.clearAllMocks();
});

describe("getPrimaryBlogFeed", () => {
  it("solo with a primary blog: feed metadata points at the root feed URL", async () => {
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue("foo");
    vi.mocked(getPublicBlog).mockResolvedValue({ slug: "foo", title: "Foo", description: null, subtitle: "Sotto", default_locale: "it" } as never);
    const feed = await getPrimaryBlogFeed("/atom.xml");
    expect(feed?.meta).toEqual({
      title: "Foo",
      description: "Sotto",
      link: "https://example.com",
      feedUrl: "https://example.com/atom.xml",
      language: "it",
    });
    expect(feed?.items).toEqual([]);
    expect(getPublicBlogPosts).toHaveBeenCalledWith("foo");
  });

  it("platform: null, no backend call for the blog", async () => {
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "platform");
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue(null);
    expect(await getPrimaryBlogFeed("/feed.xml")).toBeNull();
    expect(getPublicBlog).not.toHaveBeenCalled();
  });

  it("solo, primary blog not visible: null (platform feed fallback)", async () => {
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue("foo");
    vi.mocked(getPublicBlog).mockResolvedValue(null);
    expect(await getPrimaryBlogFeed("/feed.xml")).toBeNull();
  });
});
