import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("server-only", () => ({}));
vi.mock("next-intl/server", () => ({
  getTranslations: async () => (key: string) => key,
}));
vi.mock("@/lib/server-api", () => ({
  getPrimaryBlogSlug: vi.fn(async () => null),
  getPublicFeed: vi.fn(async () => []),
  getTrendingTags: vi.fn(async () => []),
  getPublicBlogs: vi.fn(async () => []),
  getFeedLocales: vi.fn(async () => []),
  getPublicBlog: vi.fn(async () => null),
}));
vi.mock("@/components/blog/BlogHome", () => ({
  BlogHome: function BlogHome() {
    return null;
  },
  blogHomeMetadata: vi.fn(async () => ({ title: "Blog primario", alternates: { canonical: "/" } })),
}));

import Home, { generateMetadata } from "@/app/page";
import { BlogHome, blogHomeMetadata } from "@/components/blog/BlogHome";
import { getPrimaryBlogSlug } from "@/lib/server-api";

const SOLO = { NOCT_SITE_URL: "https://example.com", NOCT_DEPLOYMENT_MODE: "solo", NOCT_SUBDOMAINS_ENABLED: "" };
const searchParams = Promise.resolve({});

beforeEach(() => {
  for (const [k, v] of Object.entries(SOLO)) vi.stubEnv(k, v);
});

afterEach(() => {
  vi.unstubAllEnvs();
  vi.clearAllMocks();
});

describe("platform home (/)", () => {
  it("solo with a primary blog: / renders the primary blog home", async () => {
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue("foo");
    const element = (await Home({ searchParams })) as { type: unknown; props: Record<string, unknown> };
    expect(element.type).toBe(BlogHome);
    expect(element.props).toEqual({ blogSlug: "foo", category: undefined });
  });

  it("solo with a primary blog: metadata is the blog one, canonical /", async () => {
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue("foo");
    const metadata = await generateMetadata({}, {} as never);
    expect(blogHomeMetadata).toHaveBeenCalledWith({ blogSlug: "foo", root: true }, expect.anything());
    expect(metadata.alternates?.canonical).toBe("/");
  });

  it("platform: the platform home is rendered, the blog home is not", async () => {
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "platform");
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue(null); // the real helper returns null in platform
    const element = (await Home({ searchParams })) as { type: unknown };
    expect(element.type).not.toBe(BlogHome);
    expect(blogHomeMetadata).not.toHaveBeenCalled();
  });

  it("solo without a primary blog: falls back to the platform home", async () => {
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue(null);
    const element = (await Home({ searchParams })) as { type: unknown };
    expect(element.type).not.toBe(BlogHome);
    expect(blogHomeMetadata).not.toHaveBeenCalled();
  });
});
