import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { MediaTab } from "@/components/dashboard/blog/MediaTab";
import { isMediaSensitive } from "@/lib/content-media";
import { renderWithIntl } from "@/test/render";

const flags = vi.hoisted(() => ({ isSolo: false, subdomainsEnabled: true }));

vi.mock("@/components/SiteConfigProvider", () => ({ useSiteFlags: () => flags }));
vi.mock("@/components/ui/Toast", () => ({ useToast: () => vi.fn() }));
vi.mock("@/lib/auth-context", () => ({
  useAuth: () => ({ authFetch: (fn: (token: string) => unknown) => Promise.resolve(fn("tok")) }),
}));

const item = (id: string, over: Record<string, unknown>) => ({
  id,
  url: `https://example.com/${id}.png`,
  content_type: "image/png",
  size_bytes: 2048,
  alt_text: `alt-${id}`,
  caption: null,
  categories: [],
  is_sensitive: false,
  uploader_username: "foo",
  created_at: "2026-01-01T00:00:00Z",
  used_in: [],
  ...over,
});

vi.mock("@/lib/api", () => ({
  ApiClientError: class extends Error {},
  api: {
    blogs: {
      mediaLibrary: vi.fn(async () => ({
        total_bytes: 0,
        items: [
          item("clean", {}),
          item("auto", { is_sensitive: true }),
          item("manual", { categories: ["nudity"] }),
        ],
      })),
    },
  },
}));

beforeEach(() => {
  flags.isSolo = false;
});

describe("isMediaSensitive", () => {
  it("is true for the auto flag or any category, false otherwise", () => {
    expect(isMediaSensitive({ is_sensitive: false, categories: [] })).toBe(false);
    expect(isMediaSensitive({ is_sensitive: true, categories: [] })).toBe(true);
    expect(isMediaSensitive({ is_sensitive: false, categories: ["other"] })).toBe(true);
  });
});

describe("MediaTab", () => {
  it("blurs only flagged images (auto flag or manual category)", async () => {
    renderWithIntl(<MediaTab blogSlug="foo" canWrite />);
    await screen.findByAltText("alt-clean");
    expect(screen.getByAltText("alt-clean").className).not.toContain("blur-xl");
    expect(screen.getByAltText("alt-auto").className).toContain("blur-xl");
    expect(screen.getByAltText("alt-manual").className).toContain("blur-xl");
  });

  it("platform: shows the auto-moderation line", async () => {
    renderWithIntl(<MediaTab blogSlug="foo" canWrite />);
    await userEvent.click(await screen.findByAltText("alt-auto"));
    expect(screen.getByText(/Auto-moderation: flagged/)).toBeInTheDocument();
    expect(screen.queryByText(/marked as sensitive manually/)).toBeNull();
  });

  it("solo: shows the manual marking message instead", async () => {
    flags.isSolo = true;
    renderWithIntl(<MediaTab blogSlug="foo" canWrite />);
    await userEvent.click(await screen.findByAltText("alt-auto"));
    expect(screen.getByText("Images are marked as sensitive manually by the author.")).toBeInTheDocument();
    expect(screen.queryByText(/Auto-moderation/)).toBeNull();
  });
});
