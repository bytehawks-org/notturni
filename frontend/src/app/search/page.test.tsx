import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("server-only", () => ({}));
vi.mock("next-intl/server", () => ({
  getTranslations: () =>
    Promise.resolve((key: string, values?: Record<string, unknown>) =>
      values ? `${key}:${Object.values(values).join("|")}` : key
    ),
}));
vi.mock("@/components/SiteHeader", () => ({ SiteHeader: () => null }));
vi.mock("@/components/shell/SiteFooter", () => ({ SiteFooter: () => null }));
vi.mock("@/components/FeedPostCard", () => ({
  FeedPostCard: ({ post }: { post: { id: string } }) => <article>{post.id}</article>,
}));
vi.mock("@/lib/server-api", () => {
  class SearchRateLimitedError extends Error {}
  return { searchPosts: vi.fn(), SearchRateLimitedError };
});

import SearchPage from "@/app/search/page";
import { searchPosts, SearchRateLimitedError } from "@/lib/server-api";

const posts = (n: number) => Array.from({ length: n }, (_, i) => ({ id: `p${i}`, permalink: `/b/p${i}` }));

const render = async (searchParams: { q?: string; page?: string }) => {
  const el = await SearchPage({ searchParams: Promise.resolve(searchParams) });
  return renderToStaticMarkup(el);
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(searchPosts).mockResolvedValue([] as never);
});

describe("portal search page", () => {
  it("renders a visible submit button in the GET form", async () => {
    const html = await render({ q: "pippo" });
    expect(html).toMatch(/<form[^>]*method="GET"/i);
    expect(html).toMatch(/<button[^>]*type="submit"[^>]*>searchButton<\/button>/);
  });

  it("caps the input at the search max length", async () => {
    const html = await render({ q: "pippo" });
    expect(html).toContain('maxLength="100"');
  });

  it("requests 20 results per page, starting at offset 0 on page 1", async () => {
    vi.mocked(searchPosts).mockResolvedValue(posts(3) as never);
    await render({ q: "pippo" });
    expect(searchPosts).toHaveBeenCalledWith("pippo", { limit: 21, offset: 0 });
  });

  it("uses offset 20 on the second page", async () => {
    vi.mocked(searchPosts).mockResolvedValue(posts(3) as never);
    await render({ q: "pippo", page: "2" });
    expect(searchPosts).toHaveBeenCalledWith("pippo", { limit: 21, offset: 20 });
  });

  it.each(["", "0", "-4", "abc"])("treats page %j as page 1", async (page) => {
    vi.mocked(searchPosts).mockResolvedValue(posts(1) as never);
    await render({ q: "pippo", page });
    expect(searchPosts).toHaveBeenCalledWith("pippo", { limit: 21, offset: 0 });
  });

  it("shows only 20 results and a next link when one more result exists", async () => {
    vi.mocked(searchPosts).mockResolvedValue(posts(21) as never);
    const html = await render({ q: "pippo" });
    expect(html).toContain('href="/search?q=pippo&amp;page=2"');
    expect(html).toContain(">p19<");
    expect(html).not.toContain(">p20<");
  });

  it("has no next link when the page is full but there is no further result", async () => {
    vi.mocked(searchPosts).mockResolvedValue(posts(20) as never);
    const html = await render({ q: "pippo" });
    expect(html).not.toContain("page=2");
  });

  it("links back to the previous page on page 2 and keeps the query", async () => {
    vi.mocked(searchPosts).mockResolvedValue(posts(2) as never);
    const html = await render({ q: "a b&c", page: "2" });
    expect(html).toContain('href="/search?q=a%20b%26c"');
  });

  it("counts only the results shown on the current page", async () => {
    vi.mocked(searchPosts).mockResolvedValue(posts(5) as never);
    const html = await render({ q: "pippo", page: "2" });
    expect(html).toContain('resultsRange:21|25|pippo');
  });

  it("shows the rate limit message instead of an error page", async () => {
    vi.mocked(searchPosts).mockRejectedValue(new SearchRateLimitedError());
    const html = await render({ q: "pippo" });
    expect(html).toContain("rateLimited");
  });

  it("does not query when there is no search term", async () => {
    const html = await render({});
    expect(searchPosts).not.toHaveBeenCalled();
    expect(html).toContain("noQuery");
  });
});
