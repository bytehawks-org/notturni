import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("server-only", () => ({}));
vi.mock("next-intl/server", () => ({ getTranslations: () => Promise.resolve((key: string) => key) }));
// Client and layout children are out of scope: only the address under the blog title is checked.
vi.mock("@/components/blog/BlogPageShell", () => ({
  BlogPageShell: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  blogLayout: () => "list",
}));
vi.mock("@/components/blog/PostHeaderActions", () => ({ BlogHeaderActions: () => null }));
vi.mock("@/components/shell/BlogHeader", () => ({ BlogHeader: () => null }));
vi.mock("@/components/NewsletterSignup", () => ({ NewsletterSignup: () => null }));
vi.mock("@/lib/server-api", () => ({
  getPrimaryBlogSlug: vi.fn(async () => "notturni"),
  getPublicBlog: vi.fn(),
  getPublicBlogPosts: vi.fn(async () => []),
  getPublicBlogCategories: vi.fn(async () => []),
  getPublicBlogConfig: vi.fn(async () => ({})),
  getPublicPublications: vi.fn(async () => []),
  getPublicOutline: vi.fn(async () => []),
}));

import { BlogHome } from "@/components/blog/BlogHome";
import { getPublicBlog } from "@/lib/server-api";

const render = async (slug: string) => {
  vi.mocked(getPublicBlog).mockResolvedValue({ slug, title: "Diario", kind: "blog", display_mode: "feed", default_locale: "it" } as never);
  return renderToStaticMarkup(await BlogHome({ blogSlug: slug }));
};

beforeEach(() => {
  vi.stubEnv("NOCT_SITE_URL", "https://example.com");
  vi.stubEnv("NOCT_DEPLOYMENT_MODE", "solo");
  vi.stubEnv("NOCT_SUBDOMAINS_ENABLED", "false");
});

afterEach(() => {
  vi.unstubAllEnvs();
});

describe("BlogHome shown address", () => {
  it("solo primary blog: shows the site URL without the slug", async () => {
    const html = await render("notturni");
    expect(html).toContain(">https://example.com</span>");
    expect(html).not.toContain("https://example.com/notturni");
  });

  it("non-primary blog in solo: keeps the slug", async () => {
    expect(await render("altro")).toContain(">https://example.com/altro</span>");
  });
});
