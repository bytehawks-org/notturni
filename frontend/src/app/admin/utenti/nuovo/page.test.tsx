import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import NewUserPage from "@/app/admin/utenti/nuovo/page";
import { renderWithIntl } from "@/test/render";

const h = vi.hoisted(() => ({ createUser: vi.fn() }));

vi.mock("@/lib/auth-context", () => ({
  useAuth: () => ({
    user: { platform_role: "super_admin" },
    authFetch: (fn: (token: string) => unknown) => Promise.resolve(fn("tok")),
  }),
}));
vi.mock("@/lib/api", () => ({
  ApiClientError: class ApiClientError extends Error {
    status: number;
    constructor(status: number, message: string) {
      super(message);
      this.status = status;
    }
  },
  api: { admin: { createUser: h.createUser } },
}));

function fill(username: string, email: string, password = "") {
  fireEvent.change(screen.getByLabelText("Username"), { target: { value: username } });
  fireEvent.change(screen.getByLabelText("Email"), { target: { value: email } });
  if (password) fireEvent.change(screen.getByLabelText(/Password/), { target: { value: password } });
  fireEvent.click(screen.getByRole("button", { name: "Create account" }));
}

beforeEach(() => {
  h.createUser.mockReset();
  window.localStorage.clear();
});

describe("NewUserPage", () => {
  it("omits the password when empty and shows the generated one once, without storing it", async () => {
    h.createUser.mockResolvedValue({ id: "1", username: "foo", email: "foo@example.com", platform_role: "utente", is_active: true, generated_password: "Gen-Secret-123" });
    renderWithIntl(<NewUserPage />);
    fill("foo", "foo@example.com");
    expect(await screen.findByTestId("generated-password")).toHaveTextContent("Gen-Secret-123");
    expect(h.createUser).toHaveBeenCalledWith("tok", { username: "foo", email: "foo@example.com" });
    expect(window.localStorage.length).toBe(0);
    expect(window.sessionStorage.length).toBe(0);
    fireEvent.click(screen.getByRole("button", { name: "Create another" }));
    expect(screen.queryByText("Gen-Secret-123")).toBeNull();
  });

  it("sends a chosen password and does not display one", async () => {
    h.createUser.mockResolvedValue({ id: "1", username: "foo", email: "foo@example.com", platform_role: "utente", is_active: true, generated_password: null });
    renderWithIntl(<NewUserPage />);
    fill("foo", "foo@example.com", "Chosen-Pass-456");
    await screen.findByText("The account uses the password you entered.");
    expect(h.createUser).toHaveBeenCalledWith("tok", { username: "foo", email: "foo@example.com", password: "Chosen-Pass-456" });
    expect(screen.queryByTestId("generated-password")).toBeNull();
  });

  it("maps 409 to the duplicate message", async () => {
    const { ApiClientError } = await import("@/lib/api");
    h.createUser.mockRejectedValue(new ApiClientError(409, "Username o email già in uso."));
    renderWithIntl(<NewUserPage />);
    fill("foo", "foo@example.com");
    await waitFor(() => expect(screen.getByText("Username or email already in use.")).toBeInTheDocument());
  });
});
