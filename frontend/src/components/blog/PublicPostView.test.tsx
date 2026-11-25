import { describe, expect, it, vi } from "vitest";

import { PublicPostView } from "@/components/blog/PublicPostView";

vi.mock("server-only", () => ({}));
vi.mock("next/navigation", () => ({
  notFound: () => {
    throw new Error("NEXT_NOT_FOUND");
  },
}));
vi.mock("next-intl/server", () => ({
  getTranslations: () => Promise.resolve((key: string) => key),
  getLocale: () => Promise.resolve("it"),
}));
vi.mock("@/lib/blog-path", () => ({
  blogLinks: () => Promise.resolve({ basePath: "", fromPermalink: (p: string) => p }),
}));
vi.mock("@/lib/server-api", () => ({
  getPrimaryBlogSlug: () => Promise.resolve("mia-pagina"),
  getPublicPostByPermalink: () => Promise.resolve(null),
  getPublicBlog: () => Promise.resolve(null),
  getPublicBlogConfig: () => Promise.resolve({}),
  getPublicPostTranslations: () => Promise.resolve([]),
  getPublicPublication: () => Promise.resolve(null),
  getPublicPublications: () => Promise.resolve([]),
  getPublicBacklinks: () => Promise.resolve([]),
  getPublicOutline: () => Promise.resolve([]),
}));

describe("PublicPostView", () => {
  it("unknown post: returns notFound", async () => {
    await expect(
      PublicPostView({ params: Promise.resolve({ blogSlug: "mia-pagina", postSlug: "non-esiste" }) })
    ).rejects.toThrow("NEXT_NOT_FOUND");
  });
});
