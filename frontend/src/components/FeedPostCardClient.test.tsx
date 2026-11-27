import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { FeedPostCardClient } from "@/components/FeedPostCardClient";
import type { Post } from "@/lib/types";
import { renderWithIntl } from "@/test/render";

const h = vi.hoisted(() => ({ primary: null as string | null }));

vi.mock("@/lib/api", () => ({
  api: { config: { get: () => Promise.resolve({ primary_blog_slug: h.primary }) } },
}));

// Synthetic post: only the fields the card reads.
const post = {
  id: "00000000-0000-0000-0000-000000000002",
  title: "Titolo client",
  slug: "prova-client",
  permalink: "/notturni/prova-client",
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
  vi.stubEnv("NOCT_DEPLOYMENT_MODE", "solo");
});

describe("FeedPostCardClient links in solo mode", () => {
  it("the primary blog links to the root and the post to /post/{slug}", async () => {
    h.primary = "notturni";
    renderWithIntl(<FeedPostCardClient post={post} blogTitle="Blog di prova" />);
    await waitFor(() => expect(screen.getByRole("link", { name: "Blog di prova" })).toHaveAttribute("href", "/"));
    expect(screen.getByRole("link", { name: "Titolo client" })).toHaveAttribute("href", "/post/prova-client");
  });
});
