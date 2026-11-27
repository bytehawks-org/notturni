import type { ResolvingMetadata } from "next";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("server-only", () => ({}));
vi.mock("next-intl/server", () => ({
  getTranslations: () => Promise.resolve((key: string) => key),
  getLocale: () => Promise.resolve("it"),
}));
vi.mock("@/lib/server-api", () => ({
  getPrimaryBlogSlug: vi.fn(async () => null),
  getPublicBlog: vi.fn(async () => ({ title: "Diario", search_indexing_enabled: true })),
  getPublicBlogConfig: vi.fn(async () => ({})),
  getPublicPublications: vi.fn(async () => []),
  getBlogMediaBibliography: vi.fn(async () => []),
}));

import { generateMetadata } from "@/app/[blogSlug]/media/page";
import { getPrimaryBlogSlug } from "@/lib/server-api";

const SOLO = { NOCT_SITE_URL: "https://example.com", NOCT_DEPLOYMENT_MODE: "solo", NOCT_SUBDOMAINS_ENABLED: "false" };
const parent = Promise.resolve({}) as unknown as ResolvingMetadata;

const metadataFor = (blogSlug: string) => generateMetadata({ params: Promise.resolve({ blogSlug }) }, parent);

beforeEach(() => {
  for (const [k, v] of Object.entries(SOLO)) vi.stubEnv(k, v);
});

afterEach(() => {
  vi.unstubAllEnvs();
  vi.clearAllMocks();
});

describe("media page metadata URLs", () => {
  it("solo primary blog: canonical and og:url have no slug", async () => {
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue("mia-pagina");
    const meta = await metadataFor("mia-pagina");
    expect(meta.alternates?.canonical).toBe("/media");
    expect(meta.openGraph).toMatchObject({ url: "/media" });
  });

  it("non-primary blog in solo: canonical and og:url keep the slug", async () => {
    vi.mocked(getPrimaryBlogSlug).mockResolvedValue("mia-pagina");
    const meta = await metadataFor("altro-blog");
    expect(meta.alternates?.canonical).toBe("/altro-blog/media");
    expect(meta.openGraph).toMatchObject({ url: "/altro-blog/media" });
  });
});
