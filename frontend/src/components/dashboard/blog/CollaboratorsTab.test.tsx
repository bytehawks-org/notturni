import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { CollaboratorsTab } from "@/components/dashboard/blog/CollaboratorsTab";
import { renderWithIntl } from "@/test/render";

const flags = vi.hoisted(() => ({ isSolo: false, subdomainsEnabled: true }));

vi.mock("@/components/SiteConfigProvider", () => ({ useSiteFlags: () => flags }));
vi.mock("@/lib/auth-context", () => ({
  useAuth: () => ({ authFetch: (fn: (token: string) => unknown) => Promise.resolve(fn("tok")) }),
}));
vi.mock("@/lib/api", () => ({
  ApiClientError: class extends Error {},
  api: {
    blogs: {
      members: vi.fn(async () => [
        { user_id: "u1", username: "owner", avatar_url: null, role: "autore", author_display_name: null, created_at: "2026-01-01" },
        { user_id: "u2", username: "helper", avatar_url: null, role: "co_autore", author_display_name: null, created_at: "2026-01-02" },
      ]),
      listInvitations: vi.fn(async () => [
        { id: "i1", invited_username: "invitee", role: "mediatore", status: "pending" },
      ]),
      createInvitation: vi.fn(),
      revokeInvitation: vi.fn(),
      removeMember: vi.fn(),
      updateMemberRole: vi.fn(),
    },
  },
}));

const rolePills = () => screen.queryAllByText(/^(co-author|author|mediator)$/, { selector: "span" });

async function renderTab() {
  renderWithIntl(<CollaboratorsTab blogSlug="foo" />);
  await screen.findByText("@helper");
}

beforeEach(() => {
  flags.isSolo = false;
});

describe("CollaboratorsTab role labels", () => {
  it("platform: shows a role pill per member and the role next to pending invitations", async () => {
    await renderTab();
    expect(rolePills().length).toBe(2);
    await waitFor(() => expect(screen.getByText(/@invitee — mediator/)).toBeInTheDocument());
  });

  it("solo: hides role pills and the invitation role label", async () => {
    flags.isSolo = true;
    await renderTab();
    expect(rolePills()).toHaveLength(0);
    await waitFor(() => expect(screen.getByText(/@invitee/)).toBeInTheDocument());
    expect(screen.queryByText(/@invitee — /)).toBeNull();
  });

  it.each([false, true])("offers both roles in the invite form and the member selector (solo=%s)", async (solo) => {
    flags.isSolo = solo;
    await renderTab();
    const selects = screen.getAllByRole("combobox");
    // one role-change selector per member + the invite form selector
    expect(selects).toHaveLength(3);
    for (const select of selects) {
      const options = Array.from(select.querySelectorAll("option")).map((o) => o.value);
      expect(options).toEqual(["co_autore", "mediatore"]);
    }
  });
});
