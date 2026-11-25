import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { UserDirectoryGrid } from "@/components/home/UserDirectory";
import { renderWithIntl } from "@/test/render";
import type { Interest, PublicUser } from "@/lib/types";

const flags = vi.hoisted(() => ({ isSolo: true }));
vi.mock("@/lib/site", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/site")>()),
  isSolo: () => flags.isSolo,
}));
vi.mock("next-intl/server", () => ({
  getTranslations: async () => (key: string) => key,
}));

const interests: Interest[] = [{ key: "music", translations: { en: "Music" } }];

function makeUser(): PublicUser {
  return {
    username: "alice",
    display_name: "Alice",
    custom_domain: null,
    avatar_url: null,
    bio: null,
    verification_tier: "none",
    follower_count: 0,
    interests: ["music"],
  } as unknown as PublicUser;
}

/** Async Server Component: call it and render the returned tree. */
async function renderGrid(props: { interests?: Interest[] } = {}) {
  const element = await UserDirectoryGrid({ users: [makeUser()], locale: "en", ...props });
  return renderWithIntl(element);
}

beforeEach(() => {
  flags.isSolo = true;
});

describe("UserDirectoryGrid in solo", () => {
  it("does not render interest filter chips on the cards", async () => {
    await renderGrid({ interests });
    expect(screen.getByText("Alice")).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Music" })).toBeNull();
    expect(document.querySelector('a[href^="/users?interest="]')).toBeNull();
  });
});

describe("UserDirectoryGrid in platform", () => {
  beforeEach(() => {
    flags.isSolo = false;
  });

  it("renders interest filter chips linking to /users?interest=", async () => {
    await renderGrid({ interests });
    expect(screen.getByRole("link", { name: "Music" })).toHaveAttribute("href", "/users?interest=music");
  });
});
