import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("server-only", () => ({}));

import { SearchRateLimitedError, searchBlogPosts, searchPosts } from "@/lib/server-api";
import { mockHeaders } from "@/test/next-mocks";

const fetchMock = vi.fn<typeof fetch>();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

function respond(status: number) {
  fetchMock.mockResolvedValue(new Response("[]", { status }));
}

function sentHeaders() {
  return new Headers(fetchMock.mock.calls[0][1]?.headers);
}

describe.each([
  ["searchPosts", () => searchPosts("foo")],
  ["searchBlogPosts", () => searchBlogPosts("bar", "foo")],
])("%s", (_name, search) => {
  it("forwards the incoming x-forwarded-for chain unchanged", async () => {
    mockHeaders.set("x-forwarded-for", "198.51.100.7, 192.168.0.2");
    mockHeaders.set("cookie", "foo=bar");
    respond(200);

    await expect(search()).resolves.toEqual([]);

    expect([...sentHeaders()]).toEqual([["x-forwarded-for", "198.51.100.7, 192.168.0.2"]]);
    expect(fetchMock.mock.calls[0][1]?.cache).toBe("no-store");
  });

  it("sends no header when the request has none", async () => {
    respond(200);

    await search();

    expect([...sentHeaders()]).toEqual([]);
  });

  it("reports 429 as a distinct error", async () => {
    respond(429);

    await expect(search()).rejects.toBeInstanceOf(SearchRateLimitedError);
  });

  it("reports other failures as a generic error", async () => {
    respond(500);

    const error = await search().catch((e: unknown) => e);

    expect(error).toBeInstanceOf(Error);
    expect(error).not.toBeInstanceOf(SearchRateLimitedError);
  });
});

describe("searchBlogPosts", () => {
  it("returns null on 404", async () => {
    respond(404);

    await expect(searchBlogPosts("bar", "foo")).resolves.toBeNull();
  });
});
