import { beforeEach, describe, expect, it, vi } from "vitest";

import PostPage from "@/app/[blogSlug]/[postSlug]/page";

const h = vi.hoisted(() => ({ solo: true, primary: "mia-pagina" as string | null }));

vi.mock("next/navigation", () => ({
  permanentRedirect: (url: string) => {
    throw new Error(`NEXT_REDIRECT:permanent:${url}`);
  },
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
  PublicPostView: () => <div data-testid="post" />,
  generatePostMetadata: () => Promise.resolve({ title: "x" }),
}));

beforeEach(() => {
  h.solo = true;
  h.primary = "mia-pagina";
});

describe("/[blogSlug]/[postSlug] redirect", () => {
  it("solo, primary blog: permanently redirects to /post/<post>", async () => {
    await expect(PostPage({ params: Promise.resolve({ blogSlug: "mia-pagina", postSlug: "ciao" }) })).rejects.toThrow(
      "NEXT_REDIRECT:permanent:/post/ciao"
    );
  });

  it("solo, other blog: no redirect", async () => {
    const element = await PostPage({ params: Promise.resolve({ blogSlug: "altro", postSlug: "ciao" }) });
    expect(element).toBeTruthy();
  });

  it("platform: no redirect even if a slug matches", async () => {
    h.solo = false;
    const element = await PostPage({ params: Promise.resolve({ blogSlug: "mia-pagina", postSlug: "ciao" }) });
    expect(element).toBeTruthy();
  });
});
