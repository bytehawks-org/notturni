import { fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { SiteConfigProvider } from "@/components/SiteConfigProvider";
import { SiteHeader } from "@/components/SiteHeader";
import type { SearchScope } from "@/lib/search";
import { renderWithIntl } from "@/test/render";

const seen = vi.hoisted(() => ({ scopes: [] as SearchScope[] }));

vi.mock("next/navigation", () => ({
  usePathname: () => "/",
  useRouter: () => ({ push: vi.fn() }),
}));
vi.mock("@/lib/auth-context", () => ({
  useAuth: () => ({ user: null, loading: false }),
}));
vi.mock("@/components/ThemeToggle", () => ({ ThemeToggle: () => null }));
vi.mock("@/components/shell/UiLanguagePicker", () => ({ UiLanguagePicker: () => null }));
// Il modal reale è coperto da SearchModal.test.tsx: qui si verifica solo l'ambito passato.
vi.mock("@/components/SearchModal", () => ({
  SearchModal: ({ open, scope }: { open: boolean; scope?: SearchScope }) => {
    if (!open) return null;
    seen.scopes.push(scope ?? { kind: "portal" });
    return <div data-testid="search-modal" />;
  },
}));

const BASE = { siteUrl: "https://example.com", apiUrl: "https://api.example.com", siteName: "Test" };
const PLATFORM = { ...BASE, deploymentMode: "platform" as const, subdomainsEnabled: true };
const SOLO = { ...BASE, deploymentMode: "solo" as const, subdomainsEnabled: false };

function renderHeader(config: typeof PLATFORM | typeof SOLO, primarySlug: string | null = null) {
  return renderWithIntl(
    <SiteConfigProvider config={config} primarySlug={primarySlug}>
      <SiteHeader />
    </SiteConfigProvider>,
  );
}

function jsonResponse(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });
}

function stubConfigFetch(response: () => Response) {
  const fetchMock = vi.fn(async () => response());
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

async function openSearch() {
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  return screen.findByTestId("search-modal");
}

beforeEach(() => {
  seen.scopes.length = 0;
});
afterEach(() => {
  vi.unstubAllGlobals();
});

describe("SiteHeader search scope", () => {
  it("platform: searches the whole portal and makes no config request", async () => {
    const fetchMock = stubConfigFetch(() => jsonResponse({}));
    renderHeader(PLATFORM);
    await openSearch();
    expect(seen.scopes).toEqual([{ kind: "portal" }]);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("solo: uses the primary blog from the provider, with empty basePath", async () => {
    const fetchMock = stubConfigFetch(() => jsonResponse({}));
    renderHeader(SOLO, "foo");
    await openSearch();
    expect(seen.scopes).toEqual([{ kind: "blog", slug: "foo", basePath: "" }]);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("solo: falls back to the portal scope when the primary slug cannot be fetched", async () => {
    // Primo test della file: la cache della richiesta condivisa non conserva i fallimenti.
    stubConfigFetch(() => jsonResponse({ detail: "boom" }, 500));
    renderHeader(SOLO, null);
    await openSearch();
    await waitFor(() => expect(seen.scopes.at(-1)).toEqual({ kind: "portal" }));
    expect(seen.scopes.every((s) => s.kind === "portal")).toBe(true);
  });

  it("solo: fetches the primary slug only when the modal opens", async () => {
    const fetchMock = stubConfigFetch(() => jsonResponse({ primary_blog_slug: "bar" }));
    renderHeader(SOLO, null);
    expect(fetchMock).not.toHaveBeenCalled();
    await openSearch();
    await waitFor(() => expect(seen.scopes.at(-1)).toEqual({ kind: "blog", slug: "bar", basePath: "" }));
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});
