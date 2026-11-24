import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { createElement } from "react";
import { afterEach, vi } from "vitest";

import { mockHeaders } from "@/test/next-mocks";

afterEach(() => {
  cleanup();
  mockHeaders.clear();
});

// `next/headers`: request headers/cookies are controlled by tests through `mockHeaders` (src/test/next-mocks.ts).
vi.mock("next/headers", () => ({
  headers: async () => new Headers(mockHeaders.entries()),
  cookies: async () => ({ get: () => undefined, getAll: () => [], has: () => false }),
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: vi.fn(), replace: vi.fn(), back: vi.fn(), refresh: vi.fn(), prefetch: vi.fn() }),
  usePathname: () => "/",
  useSearchParams: () => new URLSearchParams(),
  useParams: () => ({}),
  redirect: vi.fn(),
  notFound: vi.fn(),
}));

vi.mock("next/link", () => ({
  default: ({ href, children }: { href: unknown; children?: unknown }) =>
    createElement("a", { href: typeof href === "string" ? href : String((href as { pathname?: string }).pathname ?? "") }, children as never),
}));
