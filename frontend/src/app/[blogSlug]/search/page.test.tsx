import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("server-only", () => ({}));
vi.mock("next-intl/server", () => ({
  getTranslations: () =>
    Promise.resolve((key: string, values?: Record<string, unknown>) =>
      values ? `${key}:${Object.values(values).join("|")}` : key
    ),
}));
vi.mock("next/navigation", () => ({ notFound: vi.fn() }));
vi.mock("@/components/blog/BlogPageShell", () => ({
  BlogPageShell: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}));
vi.mock("@/components/blog/BlogStateNotice", () => ({
  BlogStateNotice: () => null,
  blogIsOffline: () => false,
}));
vi.mock("@/components/FeedPostCard", () => ({
  FeedPostCard: ({ post }: { post: { id: string } }) => <article>{post.id}</article>,
}));
vi.mock("@/components/shell/BlogHeader", () => ({ BlogHeader: () => null }));
vi.mock("@/lib/blog-links", () => ({
  currentBlogLinks: vi.fn(async () => ({ basePath: "", homeHref: "/", fromPermalink: (p: string) => p })),
}));
vi.mock("@/lib/server-api", () => {
  class SearchRateLimitedError extends Error {}
  return {
    getPublicBlog: vi.fn(async () => ({ title: "Diario", status: "public" })),
    getPublicBlogConfig: vi.fn(async () => ({})),
    getPublicPublications: vi.fn(async () => []),
    searchBlogPosts: vi.fn(),
    SearchRateLimitedError,
  };
});

import BlogSearchPage from "@/app/[blogSlug]/search/page";
import { currentBlogLinks } from "@/lib/blog-links";
import { searchBlogPosts, SearchRateLimitedError } from "@/lib/server-api";

const posts = (n: number) => Array.from({ length: n }, (_, i) => ({ id: `p${i}`, permalink: `/diario/p${i}` }));

const render = async (searchParams: { q?: string; page?: string }, blogSlug = "diario") => {
  const el = await BlogSearchPage({
    params: Promise.resolve({ blogSlug }),
    searchParams: Promise.resolve(searchParams),
  });
  return renderToStaticMarkup(el);
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(searchBlogPosts).mockResolvedValue([] as never);
});

describe("blog search page", () => {
  it("renders a visible submit button and a capped input", async () => {
    const html = await render({ q: "pippo" });
    expect(html).toMatch(/<button[^>]*type="submit"[^>]*>searchButton<\/button>/);
    expect(html).toContain('maxLength="100"');
  });

  it("requests 20 results per page and uses offset 20 on page 2", async () => {
    vi.mocked(searchBlogPosts).mockResolvedValue(posts(2) as never);
    await render({ q: "pippo", page: "2" });
    expect(searchBlogPosts).toHaveBeenCalledWith("diario", "pippo", { limit: 21, offset: 20 });
  });

  it.each(["", "0", "-1", "x"])("treats page %j as page 1", async (page) => {
    vi.mocked(searchBlogPosts).mockResolvedValue(posts(1) as never);
    await render({ q: "pippo", page });
    expect(searchBlogPosts).toHaveBeenCalledWith("diario", "pippo", { limit: 21, offset: 0 });
  });

  it("links to the next page only when one more result exists", async () => {
    vi.mocked(searchBlogPosts).mockResolvedValue(posts(21) as never);
    const withMore = await render({ q: "pippo" });
    expect(withMore).toContain('href="/search?q=pippo&amp;page=2"');
    vi.mocked(searchBlogPosts).mockResolvedValue(posts(20) as never);
    const full = await render({ q: "pippo" });
    expect(full).not.toContain("page=2");
  });

  it("builds pagination links from the blog basePath", async () => {
    vi.mocked(currentBlogLinks).mockResolvedValueOnce({
      basePath: "/diario",
      homeHref: "/diario",
      fromPermalink: (p: string) => p,
    } as never);
    vi.mocked(searchBlogPosts).mockResolvedValue(posts(21) as never);
    const html = await render({ q: "a b", page: "1" });
    expect(html).toContain('href="/diario/search?q=a%20b&amp;page=2"');
  });

  it("keeps the query in the previous link on page 2", async () => {
    vi.mocked(searchBlogPosts).mockResolvedValue(posts(2) as never);
    const html = await render({ q: "a b&c", page: "2" });
    expect(html).toContain('href="/search?q=a%20b%26c"');
  });

  it("counts only the results shown on the current page", async () => {
    vi.mocked(searchBlogPosts).mockResolvedValue(posts(5) as never);
    const html = await render({ q: "pippo", page: "2" });
    expect(html).toContain('resultsRange:21|25|pippo');
  });

  it("shows the rate limit message instead of an error page", async () => {
    vi.mocked(searchBlogPosts).mockRejectedValue(new SearchRateLimitedError());
    const html = await render({ q: "pippo" });
    expect(html).toContain("rateLimited");
  });

  it("does not query when there is no search term", async () => {
    const html = await render({});
    expect(searchBlogPosts).not.toHaveBeenCalled();
    expect(html).toContain("noQuery");
  });
});
