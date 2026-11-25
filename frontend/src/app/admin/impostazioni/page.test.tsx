import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import PlatformSettingsPage from "@/app/admin/impostazioni/page";
import { renderWithIntl } from "@/test/render";
import type { PlatformConfig } from "@/lib/types";

const flags = vi.hoisted(() => ({ isSolo: true, subdomainsEnabled: false }));
vi.mock("@/components/SiteConfigProvider", () => ({ useSiteFlags: () => flags }));
vi.mock("@/components/ui/Toast", () => ({ useToast: () => vi.fn() }));
vi.mock("@/lib/auth-context", () => ({
  useAuth: () => ({
    user: { platform_role: "super_admin" },
    authFetch: (fn: (token: string) => unknown) => Promise.resolve(fn("tok")),
  }),
}));

const api = vi.hoisted(() => ({
  getConfig: vi.fn(),
  updateConfig: vi.fn(),
}));
vi.mock("@/lib/api", () => ({
  ApiClientError: class extends Error {},
  api: { admin: api },
}));

function makeConfig(): PlatformConfig {
  return {
    default_locale: "it",
    registration_mode: "open",
    sso_providers: [],
    sso_configured: [],
    mfa_required_for_admins: false,
    reserved_blog_names: ["staff"],
    reserved_builtin: ["admin"],
    moderation_threshold: 0.8,
    max_blogs_per_user: 3,
    anonymous_comments_allowed: false,
    audit_retention_days: 90,
    footer_column1_markdown: "",
    footer_column2_markdown: "",
    footer_column3_markdown: "",
    footer_bottom_bar_markdown: "",
    interests: [{ key: "music", translations: { en: "Music" } }],
    max_blog_storage_mb: 0,
    verification_gold_identifiers: [],
    verification_silver_identifiers: [],
    verification_blue_domains: [],
    updated_at: null,
    infrastructure: {
      deployment_mode: "solo",
      instance_fqdn: "example.com",
      storage_backend: "localstorage",
      s3_endpoint_url: null,
      moderation_service: false,
      smtp: true,
      nats: false,
      redis: false,
      subdomains_enabled: false,
      worker_inline: true,
      local_storage_public_url: null,
      modules: ["pages"],
      bus: "memory",
      cache: "memory",
    },
  } as unknown as PlatformConfig;
}

/** Changes a footer field so the Save button becomes enabled, then clicks it. */
async function editFooterAndSave() {
  fireEvent.change(screen.getByLabelText(/Column 1/), { target: { value: "Nuovo testo" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(api.updateConfig).toHaveBeenCalledTimes(1));
  return api.updateConfig.mock.calls[0][1] as Record<string, unknown>;
}

beforeEach(() => {
  flags.isSolo = true;
  api.getConfig.mockReset().mockImplementation(async () => makeConfig());
  api.updateConfig.mockReset().mockImplementation(async () => makeConfig());
});

describe("PlatformSettingsPage in solo", () => {
  it("hides the reserved-names editor and the interests editor", async () => {
    renderWithIntl(<PlatformSettingsPage />);
    await screen.findByText("Footer");
    expect(screen.queryByText("Reserved space names")).toBeNull();
    expect(screen.queryByPlaceholderText("name")).toBeNull();
    expect(screen.queryByText("User interests")).toBeNull();
    expect(screen.queryByText("staff")).toBeNull();
  });

  it("keeps the footer fields", async () => {
    renderWithIntl(<PlatformSettingsPage />);
    await screen.findByText("Footer");
    expect(screen.getByLabelText(/Column 1/)).toBeInTheDocument();
    expect(screen.getByLabelText(/Column 2/)).toBeInTheDocument();
    expect(screen.getByLabelText(/Column 3/)).toBeInTheDocument();
  });

  it("omits reserved_blog_names from the PATCH body", async () => {
    renderWithIntl(<PlatformSettingsPage />);
    await screen.findByText("Footer");
    const body = await editFooterAndSave();
    expect(body).not.toHaveProperty("reserved_blog_names");
    expect(body.footer_column1_markdown).toBe("Nuovo testo");
  });
});

describe("PlatformSettingsPage in platform", () => {
  beforeEach(() => {
    flags.isSolo = false;
  });

  it("shows the reserved-names editor and the interests editor", async () => {
    renderWithIntl(<PlatformSettingsPage />);
    await screen.findByText("Footer");
    expect(screen.getByText("Reserved space names")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("name")).toBeInTheDocument();
    expect(screen.getByText("User interests")).toBeInTheDocument();
    expect(screen.getByText("Footer")).toBeInTheDocument();
  });

  it("sends reserved_blog_names in the PATCH body", async () => {
    renderWithIntl(<PlatformSettingsPage />);
    await screen.findByText("Footer");
    const body = await editFooterAndSave();
    expect(body.reserved_blog_names).toEqual(["staff"]);
  });
});
