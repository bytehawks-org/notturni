import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { VerificationBadge } from "@/components/ui/VerificationBadge";

afterEach(() => vi.unstubAllEnvs());

describe("VerificationBadge", () => {
  it("renders the seal in platform mode", () => {
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "platform");
    render(<VerificationBadge tier="gold" label="Verified" />);
    expect(screen.getByRole("img", { name: "Verified" })).toBeInTheDocument();
  });

  it("renders nothing in solo mode", () => {
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "solo");
    const { container } = render(<VerificationBadge tier="gold" label="Verified" />);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders nothing for tier none in platform mode", () => {
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "platform");
    const { container } = render(<VerificationBadge tier="none" />);
    expect(container).toBeEmptyDOMElement();
  });
});
