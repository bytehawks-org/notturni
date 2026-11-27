import { renderToStaticMarkup } from "react-dom/server";
import { NextIntlClientProvider } from "next-intl";
import { describe, expect, it, vi, beforeEach } from "vitest";

import { BlogDirectoryGridClient } from "@/components/home/BlogDirectoryClient";
import { SiteConfigProvider } from "@/components/SiteConfigProvider";
import en from "@/messages/en.json";
import type { Blog } from "@/lib/types";

const apiGet = vi.hoisted(() => vi.fn(async () => ({ primary_blog_slug: "notturni" })));
vi.mock("@/lib/api", () => ({ api: { config: { get: apiGet } } }));

// Synthetic blogs: only the fields the directory client reads.
const blogs = [
  { slug: "notturni", title: "Blog primario", favicon_url: null, subtitle: null, description: null },
  { slug: "altro", title: "Altro blog", favicon_url: null, subtitle: null, description: null },
] as unknown as Blog[];

const SOLO_CONFIG = {
  siteUrl: "https://example.com",
  apiUrl: "https://example.com",
  siteName: "Notturni",
  deploymentMode: "solo" as const,
  subdomainsEnabled: false,
};

function renderGrid(config: typeof SOLO_CONFIG, primarySlug: string | null) {
  return renderToStaticMarkup(
    <NextIntlClientProvider locale="en" messages={en} timeZone="Europe/Rome">
      <SiteConfigProvider config={config} primarySlug={primarySlug}>
        <BlogDirectoryGridClient blogs={blogs} />
      </SiteConfigProvider>
    </NextIntlClientProvider>,
  );
}

beforeEach(() => {
  apiGet.mockClear();
});

describe("BlogDirectoryGridClient server render in solo mode", () => {
  it("links the primary blog to the root, without its slug, when the provider gives the slug", () => {
    const html = renderGrid(SOLO_CONFIG, "notturni");
    expect(html).toContain('href="/"');
    expect(html).not.toContain('href="/notturni"');
    expect(html).toContain('href="/altro"');
  });

  it("shows the primary address as the site URL, as the server directory does", () => {
    const html = renderGrid(SOLO_CONFIG, "notturni");
    expect(html).toContain(">https://example.com</span>");
  });
});

describe("usePrimaryBlogSlug fetch", () => {
  it("does not fetch the config on the client when the provider already gives the slug", async () => {
    const { render, waitFor } = await import("@testing-library/react");
    const { usePrimaryBlogSlug } = await import("@/lib/primary-blog-client");
    function Probe() {
      return <span data-testid="slug">{usePrimaryBlogSlug() ?? "none"}</span>;
    }
    const { getByTestId } = render(
      <SiteConfigProvider config={SOLO_CONFIG} primarySlug="notturni">
        <Probe />
      </SiteConfigProvider>,
    );
    expect(getByTestId("slug").textContent).toBe("notturni");
    await waitFor(() => expect(apiGet).not.toHaveBeenCalled());
  });
});
