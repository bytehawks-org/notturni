import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import DashboardHomePage from "@/app/dashboard/page";
import { renderWithIntl } from "@/test/render";
import { siteUrl } from "@/lib/site";

const flags = vi.hoisted(() => ({ isSolo: true, subdomainsEnabled: false }));
vi.mock("@/components/SiteConfigProvider", () => ({ useSiteFlags: () => flags }));
vi.mock("@/lib/auth-context", () => ({
  useAuth: () => ({
    user: { username: "anna" },
    authFetch: (fn: (token: string) => unknown) => Promise.resolve(fn("tok")),
  }),
}));

const api = vi.hoisted(() => ({
  blogs: {
    listMine: vi.fn(),
    memberOf: vi.fn(),
    receivedInvitations: vi.fn(),
    create: vi.fn(),
  },
  posts: { list: vi.fn() },
  comments: { listForBlog: vi.fn() },
  users: { followStats: vi.fn() },
  config: { get: vi.fn() },
}));
vi.mock("@/lib/api", () => {
  class ApiClientError extends Error {
    status: number;
    constructor(status: number, message: string) {
      super(message);
      this.status = status;
    }
  }
  return { ApiClientError, api };
});

async function openCreateForm() {
  fireEvent.click(await screen.findByRole("button", { name: /Nuovo spazio|New space/ }));
  return screen.findByLabelText(/^Slug/) as Promise<HTMLInputElement>;
}

describe("dashboard create-space form", () => {
  beforeEach(() => {
    flags.isSolo = true;
    api.blogs.listMine.mockResolvedValue([]);
    api.blogs.memberOf.mockResolvedValue([]);
    api.blogs.receivedInvitations.mockResolvedValue([]);
    api.blogs.create.mockReset();
    api.posts.list.mockResolvedValue([]);
    api.comments.listForBlog.mockResolvedValue([]);
    api.users.followStats.mockResolvedValue({ total_followers: 0 });
    api.config.get.mockResolvedValue({ experimental_features: [] });
  });

  it("solo accepts a 2-char slug on the client and shows the path address", async () => {
    flags.isSolo = true;
    renderWithIntl(<DashboardHomePage />);
    const slug = await openCreateForm();

    expect(slug).not.toHaveAttribute("minLength");
    expect(slug).toHaveAttribute("maxLength", "63");
    fireEvent.change(slug, { target: { value: "ab" } });
    expect(slug.validity.tooShort).toBe(false);
    expect(slug.validity.valid).toBe(true);
    expect(screen.getByText(`${siteUrl()}/ab`)).toBeInTheDocument();
  });

  it("solo still rejects a slug that breaks the regex", async () => {
    renderWithIntl(<DashboardHomePage />);
    const slug = await openCreateForm();

    fireEvent.change(slug, { target: { value: "Ab_" } });
    expect(slug.validity.patternMismatch).toBe(true);
  });

  it("solo shows the backend collision error next to the slug field", async () => {
    const { ApiClientError } = await import("@/lib/api");
    api.blogs.create.mockRejectedValue(new ApiClientError(400, "'admin' collides with a site page."));
    renderWithIntl(<DashboardHomePage />);
    const slug = await openCreateForm();

    fireEvent.change(slug, { target: { value: "admin" } });
    fireEvent.submit(slug.closest("form") as HTMLFormElement);

    const alert = await waitFor(() => screen.getByText("'admin' collides with a site page."));
    expect(slug).toHaveAttribute("aria-describedby", expect.stringContaining(alert.id));
  });

  it("platform still requires 4 chars and shows the min-4 hint", async () => {
    flags.isSolo = false;
    renderWithIntl(<DashboardHomePage />);
    const slug = await openCreateForm();

    expect(slug).toHaveAttribute("minLength", "4");
    expect(slug).not.toHaveAttribute("maxLength");
    expect(screen.getByText(/min 4 chars/)).toBeInTheDocument();
    expect(screen.queryByText(`${siteUrl()}/ab`)).not.toBeInTheDocument();
  });
});
