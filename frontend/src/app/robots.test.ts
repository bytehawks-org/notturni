import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("server-only", () => ({}));
vi.mock("@/lib/server-api", () => ({
  getCrawlDirectives: vi.fn(),
  getSiteSeo: vi.fn(async () => null),
}));

import robots from "@/app/robots";
import { getCrawlDirectives } from "@/lib/server-api";

beforeEach(() => {
  vi.stubEnv("NOCT_SITE_URL", "https://example.com");
});

describe("robots", () => {
  it("emits the primary-blog disallow strings verbatim, without trimming $ or ?", async () => {
    const disallow = ["/$", "/?", "/post/", "/pagina/", "/pub/", "/feed.xml$", "/pub$", "/pub?", "/media-lab"];
    vi.mocked(getCrawlDirectives).mockResolvedValue({
      search_disallow: disallow,
      ai_disallow: [],
      ai_user_agents: [],
    } as never);

    const result = await robots();
    const star = result.rules;
    expect(Array.isArray(star) ? star[0] : star).toMatchObject({ userAgent: "*", allow: "/", disallow });
    expect(result.sitemap).toBe("https://example.com/sitemap.xml");
  });
});
