import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("server-only", () => ({}));
vi.mock("next-intl/server", () => ({
  getTranslations: () => Promise.resolve((key: string) => key),
  getLocale: () => Promise.resolve("it"),
}));
vi.mock("@/lib/markdown", () => ({
  excerpt: () => "",
  renderPost: vi.fn(async () => ({ html: "", headings: [] })),
}));
vi.mock("@/components/blog/JsonLd", () => ({ JsonLd: vi.fn(() => null) }));
vi.mock("@/lib/server-api", () => ({
  getPrimaryBlogSlug: vi.fn(async () => null),
  getPublicPostByPermalink: vi.fn(),
  getPublicBlog: vi.fn(),
  getPublicBlogConfig: vi.fn(async () => ({})),
  getPublicPostTranslations: vi.fn(async () => []),
  getPublicPublication: vi.fn(async () => null),
  getPublicPublications: vi.fn(async () => []),
  getPublicBacklinks: vi.fn(async () => []),
  getPublicOutline: vi.fn(async () => []),
}));

import { PublicPostView } from "@/components/blog/PublicPostView";
import { JsonLd } from "@/components/blog/JsonLd";
import { renderPost } from "@/lib/markdown";
import { getPrimaryBlogSlug, getPublicBlog, getPublicPostByPermalink } from "@/lib/server-api";

const SOLO = { NOCT_SITE_URL: "https://example.com", NOCT_DEPLOYMENT_MODE: "solo", NOCT_SUBDOMAINS_ENABLED: "false" };

const post = {
  id: "p1",
  slug: "voce",
  permalink: "/mia-pagina/voce",
  title: "Voce",
  content: "",
  locale: "it",
  author_display_name: "Autore",
  published_at: null,
  cover_image_url: null,
  cover_image_is_sensitive: false,
  effective_search_indexing_enabled: true,
  mentions_enabled: false,
  notes: [],
  tags: [],
  publication: null,
};

/** Props of the last `renderPost` call (wiki link resolver is passed there). */
const lastRenderOptions = () => vi.mocked(renderPost).mock.calls.at(-1)?.[1] as { wikiLinks?: { hrefFor: (s: string) => string } } | undefined;
/** Finds the `<JsonLd data={...}>` element in the tree returned by the async server component. */
function findJsonLd(node: unknown): Record<string, unknown> | undefined {
  if (Array.isArray(node)) {
    for (const child of node) {
      const found = findJsonLd(child);
      if (found) return found;
    }
    return undefined;
  }
  if (!node || typeof node !== "object") return undefined;
  const el = node as { type?: unknown; props?: Record<string, unknown> };
  if (el.type === JsonLd) return el.props?.data as Record<string, unknown>;
  return findJsonLd(el.props?.children);
}

const render = async (blogSlug: string) => {
  const tree = await PublicPostView({ params: Promise.resolve({ blogSlug, postSlug: "voce" }) });
  return findJsonLd(tree);
};

beforeEach(() => {
  for (const [k, v] of Object.entries(SOLO)) vi.stubEnv(k, v);
  vi.mocked(getPublicPostByPermalink).mockResolvedValue(post as never);
  vi.mocked(getPublicBlog).mockResolvedValue({ title: "Diario", kind: "wiki" } as never);
});

afterEach(() => {
  vi.unstubAllEnvs();
  vi.clearAllMocks();
});

describe("PublicPostView public URLs", () => {
  it("solo primary blog: JSON-LD isPartOf and wiki links have no slug", async () => {
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue("mia-pagina");
    const jsonLd = await render("mia-pagina");
    expect(jsonLd?.isPartOf).toMatchObject({ url: "https://example.com/" });
    expect(lastRenderOptions()?.wikiLinks?.hrefFor("altra-voce")).toBe("/altra-voce");
  });

  it("non-primary blog in solo: JSON-LD isPartOf and wiki links keep the slug", async () => {
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue("mia-pagina");
    const jsonLd = await render("altro-blog");
    expect(jsonLd?.isPartOf).toMatchObject({ url: "https://example.com/altro-blog" });
    expect(lastRenderOptions()?.wikiLinks?.hrefFor("altra-voce")).toBe("/altro-blog/altra-voce");
  });
});
