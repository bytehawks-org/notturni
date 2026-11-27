import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { FeedPostCard } from "@/components/FeedPostCard";
import type { Post } from "@/lib/types";
import { renderWithIntl } from "@/test/render";

const h = vi.hoisted(() => ({ primary: null as string | null }));

vi.mock("next-intl/server", () => ({
  getTranslations: async () => (key: string) => key,
  getLocale: async () => "en",
}));
vi.mock("server-only", () => ({}));
vi.mock("@/lib/server-api", () => ({ getPrimaryBlogSlug: async () => h.primary }));

// Synthetic post: only the fields FeedPostCard reads.
const post = {
  id: "00000000-0000-0000-0000-000000000001",
  title: "Titolo di prova",
  slug: "prova",
  permalink: "/notturni/prova",
  blog_slug: "notturni",
  author_display_name: "Autore Test",
  author_avatar_url: null,
  published_at: null,
  content: "Testo di prova.",
  category: null,
  notes: [],
  tags: [],
  cover_image_url: null,
  cover_image_is_sensitive: false,
} as unknown as Post;

beforeEach(() => {
  h.primary = null;
  vi.unstubAllEnvs();
});

async function renderCard(blogSlug: string) {
  const element = await FeedPostCard({ post: { ...post, blog_slug: blogSlug }, blogTitle: "Blog di prova" });
  return renderWithIntl(element);
}

describe("FeedPostCard blog link", () => {
  it("solo: the primary blog links to the root without its slug", async () => {
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "solo");
    h.primary = "notturni";
    await renderCard("notturni");
    expect(screen.getByRole("link", { name: "Blog di prova" })).toHaveAttribute("href", "/");
  });

  it("solo: another blog keeps its slug", async () => {
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "solo");
    h.primary = "notturni";
    await renderCard("altro");
    expect(screen.getByRole("link", { name: "Blog di prova" })).toHaveAttribute("href", "/altro");
  });

  it("platform: the slug is kept even when a primary slug is returned", async () => {
    h.primary = "notturni";
    await renderCard("notturni");
    expect(screen.getByRole("link", { name: "Blog di prova" })).toHaveAttribute("href", "/notturni");
  });
});
