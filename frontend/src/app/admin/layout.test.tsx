import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import AdminLayout from "@/app/admin/layout";
import { renderWithIntl } from "@/test/render";

const h = vi.hoisted(() => ({ mode: "solo" as "solo" | "platform" }));

vi.mock("next/navigation", () => ({ useRouter: () => ({ replace: vi.fn(), push: vi.fn() }), usePathname: () => "/admin" }));
vi.mock("@/components/shell/DashboardShell", () => ({
  DashboardShell: ({ items, children }: { items: { href: string; label: string }[]; children: React.ReactNode }) => (
    <div>
      <ul>{items.map((i) => <li key={i.href}><a href={i.href}>{i.label}</a></li>)}</ul>
      {children}
    </div>
  ),
}));
vi.mock("@/components/ThemeToggle", () => ({ ThemeToggle: () => null }));
vi.mock("@/lib/auth-context", () => ({
  useAuth: () => ({ user: { platform_role: "super_admin", username: "foo" }, loading: false, logout: vi.fn() }),
}));
vi.mock("@/lib/api", () => ({ api: { config: { get: () => Promise.resolve({ deployment_mode: h.mode }) } } }));

beforeEach(() => {
  h.mode = "solo";
});

describe("admin nav", () => {
  it("solo: shows Collaborators and hides Users", async () => {
    renderWithIntl(<AdminLayout>x</AdminLayout>);
    await waitFor(() => expect(screen.getByRole("link", { name: "Collaborators" })).toHaveAttribute("href", "/admin/utenti/nuovo"));
    expect(screen.queryByRole("link", { name: "Users" })).toBeNull();
  });

  it("platform: shows Users and hides Collaborators", async () => {
    h.mode = "platform";
    renderWithIntl(<AdminLayout>x</AdminLayout>);
    await waitFor(() => expect(screen.getByRole("link", { name: "Users" })).toBeInTheDocument());
    expect(screen.queryByRole("link", { name: "Collaborators" })).toBeNull();
  });
});
