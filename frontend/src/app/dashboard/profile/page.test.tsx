import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi, type Mock } from "vitest";

import ProfilePage from "@/app/dashboard/profile/page";
import { renderWithIntl } from "@/test/render";

const flags = vi.hoisted(() => ({ isSolo: true, subdomainsEnabled: false }));
vi.mock("@/components/SiteConfigProvider", () => ({ useSiteFlags: () => flags }));
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace: vi.fn(), refresh: vi.fn() }) }));
vi.mock("@/lib/auth-context", () => ({
  useAuth: () => ({
    user: { id: "u1", username: "alice" },
    authFetch: (fn: (token: string) => unknown) => Promise.resolve(fn("tok")),
    refreshUser: vi.fn(() => Promise.resolve()),
    logout: vi.fn(),
  }),
}));

/** Endpoints touched by the page; anything else resolves to undefined. */
const api = vi.hoisted(() => {
  const ns = (base: Record<string, unknown>) =>
    new Proxy(base, {
      get: (target, key) => (key in target ? target[key as string] : vi.fn(() => Promise.resolve(undefined))),
    });
  return {
    users: ns({ me: vi.fn(), updateMe: vi.fn(), followStats: vi.fn() }),
    interests: ns({ list: vi.fn() }),
  } as unknown as {
    users: Record<"me" | "updateMe" | "followStats", Mock>;
    interests: Record<"list", Mock>;
  };
});
vi.mock("@/lib/api", () => ({
  ApiClientError: class extends Error {},
  api,
}));

function makeMe() {
  return {
    id: "u1",
    username: "alice",
    email: "alice@example.com",
    bio: "",
    first_name: "",
    last_name: "",
    display_name: "",
    post_author_name_style: "username",
    country: "",
    native_language: null,
    fallback_languages: [],
    interests: ["music"],
    directory_listed: true,
    verification_tier: "none",
    pending_email_change: null,
    domain_pending_verification: null,
    domain_verification_instructions: null,
    social_links: [],
  };
}

beforeEach(() => {
  flags.isSolo = true;
  api.users.me.mockReset().mockImplementation(async () => makeMe());
  api.users.updateMe.mockReset().mockImplementation(async () => makeMe());
  api.users.followStats.mockReset().mockImplementation(async () => ({ user_followers: 0, blogs: [], total_followers: 0 }));
  api.interests.list.mockReset().mockImplementation(async () => [{ key: "music", translations: { en: "Music" } }]);
});

describe("ProfilePage in solo", () => {
  it("does not call GET /interests", async () => {
    renderWithIntl(<ProfilePage />);
    await waitFor(() => expect(api.users.me).toHaveBeenCalled());
    expect(api.interests.list).not.toHaveBeenCalled();
  });

  it("hides the interests selector", async () => {
    renderWithIntl(<ProfilePage />);
    await waitFor(() => expect(api.users.me).toHaveBeenCalled());
    expect(screen.queryByText(/interests? of 5$/)).toBeNull();
    expect(screen.queryByRole("button", { name: "Music" })).toBeNull();
  });

  it("omits interests from the profile PATCH body", async () => {
    renderWithIntl(<ProfilePage />);
    await waitFor(() => expect(api.users.me).toHaveBeenCalled());
    const bio = await screen.findByLabelText(/bio/i);
    fireEvent.change(bio, { target: { value: "Ciao" } });
    fireEvent.click(screen.getAllByRole("button", { name: /save/i })[0]);
    await waitFor(() => expect(api.users.updateMe).toHaveBeenCalled());
    const body = api.users.updateMe.mock.calls[0][1] as Record<string, unknown>;
    expect(body).toHaveProperty("bio", "Ciao");
    expect(body).not.toHaveProperty("interests");
  });
});

describe("ProfilePage in platform", () => {
  beforeEach(() => {
    flags.isSolo = false;
  });

  it("loads GET /interests and shows the selector", async () => {
    renderWithIntl(<ProfilePage />);
    expect(await screen.findByRole("button", { name: "Music" })).toBeInTheDocument();
    expect(screen.getByText(/interests? of 5$/)).toBeInTheDocument();
    expect(api.interests.list).toHaveBeenCalled();
  });

  it("sends interests in the profile PATCH body", async () => {
    renderWithIntl(<ProfilePage />);
    await screen.findByRole("button", { name: "Music" });
    const bio = screen.getByLabelText(/bio/i);
    fireEvent.change(bio, { target: { value: "Ciao" } });
    fireEvent.click(screen.getAllByRole("button", { name: /save/i })[0]);
    await waitFor(() => expect(api.users.updateMe).toHaveBeenCalled());
    const body = api.users.updateMe.mock.calls[0][1] as Record<string, unknown>;
    expect(body).toHaveProperty("interests", ["music"]);
  });
});
