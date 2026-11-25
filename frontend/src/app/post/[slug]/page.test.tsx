import { beforeEach, describe, expect, it, vi } from "vitest";

import PostBySlugPage from "@/app/post/[slug]/page";

const h = vi.hoisted(() => ({ solo: true, primary: "mia-pagina" as string | null }));

vi.mock("next/navigation", () => ({
  notFound: () => {
    throw new Error("NEXT_NOT_FOUND");
  },
}));
vi.mock("@/lib/site", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/site")>()),
  isSolo: () => h.solo,
}));
vi.mock("@/lib/server-api", () => ({
  getPrimaryBlogSlug: () => Promise.resolve(h.primary),
}));
vi.mock("@/components/blog/PublicPostView", () => ({
  PublicPostView: ({ params }: { params: Promise<{ blogSlug: string; postSlug: string }> }) => (
    <div data-testid="post">{JSON.stringify(params)}</div>
  ),
  generatePostMetadata: () => Promise.resolve({ title: "x" }),
}));

beforeEach(() => {
  h.solo = true;
  h.primary = "mia-pagina";
});

describe("/post/[slug]", () => {
  it("platform: returns notFound", async () => {
    h.solo = false;
    await expect(PostBySlugPage({ params: Promise.resolve({ slug: "ciao" }) })).rejects.toThrow("NEXT_NOT_FOUND");
  });

  it("solo without primary blog: returns notFound", async () => {
    h.primary = null;
    await expect(PostBySlugPage({ params: Promise.resolve({ slug: "ciao" }) })).rejects.toThrow("NEXT_NOT_FOUND");
  });

  it("solo with primary blog: renders the post of the primary blog", async () => {
    const element = await PostBySlugPage({ params: Promise.resolve({ slug: "ciao" }) });
    const { params } = element.props as { params: Promise<{ blogSlug: string; postSlug: string }> };
    await expect(params).resolves.toEqual({ blogSlug: "mia-pagina", postSlug: "ciao" });
  });
});
