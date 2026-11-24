import { fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { InfrastructurePanel } from "@/components/admin/InfrastructurePanel";
import { SetupBanner } from "@/components/admin/SetupBanner";
import { renderWithIntl } from "@/test/render";
import type { PlatformInfrastructure } from "@/lib/types";

const flags = vi.hoisted(() => ({ isSolo: true, subdomainsEnabled: false }));
vi.mock("@/components/SiteConfigProvider", () => ({ useSiteFlags: () => flags }));

const infra: PlatformInfrastructure = {
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
  local_storage_public_url: "https://example.com/storage",
  modules: ["pages", "comments"],
  bus: "memory",
  cache: "memory",
};

beforeEach(() => {
  flags.isSolo = true;
  window.localStorage.clear();
});

describe("InfrastructurePanel", () => {
  it("renders the new fields read-only", () => {
    renderWithIntl(<InfrastructurePanel infrastructure={infra} />);
    expect(screen.getByText("Active modules")).toBeInTheDocument();
    expect(screen.getByText("pages, comments")).toBeInTheDocument();
    expect(screen.getByText("Message bus")).toBeInTheDocument();
    expect(screen.getByText("https://example.com/storage")).toBeInTheDocument();
    expect(screen.queryByRole("textbox")).toBeNull();
  });
});

describe("SetupBanner", () => {
  it("shows in solo and can be dismissed", () => {
    renderWithIntl(<SetupBanner />);
    expect(screen.getByText("Open the wizard")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Dismiss"));
    expect(screen.queryByText("Open the wizard")).toBeNull();
    expect(window.localStorage.getItem("noct.setupWizardDismissed")).toBe("1");
  });

  it("is hidden in platform mode", () => {
    flags.isSolo = false;
    renderWithIntl(<SetupBanner />);
    expect(screen.queryByText("Open the wizard")).toBeNull();
  });
});
