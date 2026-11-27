import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import DashboardLayout from "@/app/dashboard/layout";
import { renderWithIntl } from "@/test/render";

const h = vi.hoisted(() => ({ primary: null as string | null, pathname: "/dashboard" }));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ replace: vi.fn(), push: vi.fn() }),
  usePathname: () => h.pathname,
}));
vi.mock("@/components/shell/DashboardShell", () => ({
  DashboardShell: ({ blogLink }: { blogLink?: { href: string; label: string } }) =>
    blogLink ? <a href={blogLink.href}>{blogLink.label}</a> : null,
}));
vi.mock("@/components/shell/UiLanguagePicker", () => ({ UiLanguagePicker: () => null }));
vi.mock("@/components/ThemeToggle", () => ({ ThemeToggle: () => null }));
vi.mock("@/lib/auth-context", () => ({
  useAuth: () => ({ user: { platform_role: "user", username: "foo" }, loading: false, logout: vi.fn() }),
}));
vi.mock("@/lib/api", () => ({ api: { config: { get: () => Promise.resolve({ primary_blog_slug: h.primary }) } } }));

beforeEach(() => {
  h.primary = "notturni";
  vi.stubEnv("NOCT_DEPLOYMENT_MODE", "solo");
});

describe("dashboard blog link", () => {
  it("solo: the primary blog's 'view blog' link goes to the root", async () => {
    h.pathname = "/dashboard/blogs/notturni/posts";
    renderWithIntl(<DashboardLayout>x</DashboardLayout>);
    await waitFor(() => expect(screen.getByRole("link", { name: /notturni/ })).toHaveAttribute("href", "/"));
  });

  it("solo: another blog keeps its slug", async () => {
    h.pathname = "/dashboard/blogs/altro/posts";
    renderWithIntl(<DashboardLayout>x</DashboardLayout>);
    await waitFor(() => expect(screen.getByRole("link", { name: /altro/ })).toHaveAttribute("href", "/altro"));
  });
});
