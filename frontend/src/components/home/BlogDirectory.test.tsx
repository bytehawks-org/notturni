import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { BlogDirectoryGrid, BlogDirectoryList } from "@/components/home/BlogDirectory";
import type { PublicBlog } from "@/lib/types";
import { renderWithIntl } from "@/test/render";

const h = vi.hoisted(() => ({ primary: null as string | null }));

vi.mock("next-intl/server", () => ({ getTranslations: async () => (key: string) => key }));
vi.mock("server-only", () => ({}));
vi.mock("@/lib/server-api", () => ({ getPrimaryBlogSlug: async () => h.primary }));

// Synthetic blogs: only the fields the directory reads.
const blogs = [
  { slug: "notturni", title: "Blog primario", favicon_url: null, default_locale: "it", post_count: 3, subtitle: null, description: null },
  { slug: "altro", title: "Altro blog", favicon_url: null, default_locale: "it", post_count: 1, subtitle: null, description: null },
] as unknown as PublicBlog[];

beforeEach(() => {
  h.primary = "notturni";
  vi.stubEnv("NOCT_DEPLOYMENT_MODE", "solo");
  vi.stubEnv("NOCT_SITE_URL", "https://example.com");
  vi.stubEnv("NOCT_SUBDOMAINS_ENABLED", "false");
});

describe("BlogDirectory links in solo mode", () => {
  it("grid: the primary blog links to the root, others keep the slug", async () => {
    renderWithIntl(await BlogDirectoryGrid({ blogs }));
    const titles = screen.getAllByRole("link", { name: "Blog primario" });
    expect(titles[0]).toHaveAttribute("href", "/");
    expect(screen.getAllByRole("link", { name: "Altro blog" })[0]).toHaveAttribute("href", "/altro");
  });

  it("list: the primary blog links to the root, address shows the domain only", async () => {
    renderWithIntl((await BlogDirectoryList({ blogs, total: 2 })) ?? <div />);
    expect(screen.getByRole("link", { name: "Blog primario" })).toHaveAttribute("href", "/");
    expect(screen.getByText(/^https:\/\/example\.com · /)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Altro blog" })).toHaveAttribute("href", "/altro");
  });

  it("platform: the primary blog keeps its slug", async () => {
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "platform");
    renderWithIntl(await BlogDirectoryGrid({ blogs }));
    expect(screen.getAllByRole("link", { name: "Blog primario" })[0]).toHaveAttribute("href", "/notturni");
  });
});
