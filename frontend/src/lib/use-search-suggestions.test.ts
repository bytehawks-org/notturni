import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { SEARCH_DEBOUNCE_MS, type SearchScope } from "@/lib/search";
import { useSearchSuggestions } from "@/lib/use-search-suggestions";

const PORTAL: SearchScope = { kind: "portal" };
const SOLO_PRIMARY: SearchScope = { kind: "blog", slug: "foo", basePath: "" };

function post(id: string, permalink: string) {
  return { id, title: `Titolo ${id}`, slug: id, permalink } as const;
}

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });
}

interface Call {
  url: string;
  signal: AbortSignal | undefined;
  resolve: (res: Response) => void;
  reject: (err: unknown) => void;
}

/** Stub di fetch con risposte controllate dal test. Ignora il segnale, come farebbe un server lento. */
function stubFetch() {
  const calls: Call[] = [];
  const fetchMock = vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
    return new Promise<Response>((resolve, reject) => {
      calls.push({ url: String(input), signal: init?.signal ?? undefined, resolve, reject });
    });
  });
  vi.stubGlobal("fetch", fetchMock);
  return { fetchMock, calls };
}

/** Lascia scorrere il debounce e le promise pendenti. */
async function advance(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("useSearchSuggestions", () => {
  it("is idle with an empty query and never calls the backend", async () => {
    const { fetchMock } = stubFetch();
    const { result } = renderHook(() => useSearchSuggestions(PORTAL, "   "));
    await advance(SEARCH_DEBOUNCE_MS * 2);
    expect(result.current.status).toBe("idle");
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("is short under 2 characters and never calls the backend", async () => {
    const { fetchMock } = stubFetch();
    const { result } = renderHook(() => useSearchSuggestions(PORTAL, "a"));
    await advance(SEARCH_DEBOUNCE_MS * 2);
    expect(result.current.status).toBe("short");
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("issues a single request after the debounce and maps the results", async () => {
    const { fetchMock, calls } = stubFetch();
    const { result } = renderHook(() => useSearchSuggestions(PORTAL, "lu"));
    expect(result.current.status).toBe("loading");

    await advance(SEARCH_DEBOUNCE_MS - 1);
    expect(fetchMock).not.toHaveBeenCalled();

    await advance(1);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(calls[0].url).toContain("/api/v1/search/posts?q=lu&limit=8");

    await act(async () => {
      calls[0].resolve(jsonResponse(200, [post("p1", "/foo/p1")]));
    });
    expect(result.current.status).toBe("done");
    expect(result.current.suggestions).toEqual([{ post: post("p1", "/foo/p1"), href: "/foo/p1" }]);
  });

  it("uses the blog endpoint with an encoded slug for a blog scope", async () => {
    const { calls } = stubFetch();
    renderHook(() => useSearchSuggestions({ kind: "blog", slug: "a b", basePath: "/a%20b" }, "luna"));
    await advance(SEARCH_DEBOUNCE_MS);
    expect(calls[0].url).toContain("/api/v1/blogs/a%20b/search?q=luna&limit=8");
  });

  it("aborts the previous request when the query changes", async () => {
    const { fetchMock, calls } = stubFetch();
    const { rerender } = renderHook(({ q }) => useSearchSuggestions(PORTAL, q), {
      initialProps: { q: "lun" },
    });
    await advance(SEARCH_DEBOUNCE_MS);
    expect(fetchMock).toHaveBeenCalledTimes(1);

    rerender({ q: "luna" });
    expect(calls[0].signal?.aborted).toBe(true);

    await advance(SEARCH_DEBOUNCE_MS);
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(calls[1].signal?.aborted).toBe(false);
  });

  it("cancels the pending debounce when the query goes under the minimum", async () => {
    const { fetchMock } = stubFetch();
    const { result, rerender } = renderHook(({ q }) => useSearchSuggestions(PORTAL, q), {
      initialProps: { q: "lun" },
    });
    rerender({ q: "l" });
    await advance(SEARCH_DEBOUNCE_MS * 2);
    expect(fetchMock).not.toHaveBeenCalled();
    expect(result.current.status).toBe("short");
  });

  it("ignores an out-of-order answer to an older query", async () => {
    const { calls } = stubFetch();
    const { result, rerender } = renderHook(({ q }) => useSearchSuggestions(PORTAL, q), {
      initialProps: { q: "ab" },
    });
    await advance(SEARCH_DEBOUNCE_MS);
    rerender({ q: "abc" });
    await advance(SEARCH_DEBOUNCE_MS);
    expect(calls).toHaveLength(2);

    await act(async () => {
      calls[1].resolve(jsonResponse(200, [post("new", "/foo/new")]));
    });
    await act(async () => {
      calls[0].resolve(jsonResponse(200, [post("old", "/foo/old")]));
    });
    expect(result.current.suggestions.map((s) => s.post.id)).toEqual(["new"]);
  });

  it("shows no stale suggestions while the new query is loading", async () => {
    const { calls } = stubFetch();
    const { result, rerender } = renderHook(({ q }) => useSearchSuggestions(PORTAL, q), {
      initialProps: { q: "ab" },
    });
    await advance(SEARCH_DEBOUNCE_MS);
    await act(async () => {
      calls[0].resolve(jsonResponse(200, [post("old", "/foo/old")]));
    });
    expect(result.current.status).toBe("done");

    rerender({ q: "abc" });
    expect(result.current.status).toBe("loading");
    expect(result.current.suggestions).toEqual([]);
  });

  it("reports rate_limited on HTTP 429", async () => {
    const { calls } = stubFetch();
    const { result } = renderHook(() => useSearchSuggestions(PORTAL, "lu"));
    await advance(SEARCH_DEBOUNCE_MS);
    await act(async () => {
      calls[0].resolve(jsonResponse(429, { detail: "troppe richieste" }));
    });
    expect(result.current.status).toBe("rate_limited");
    expect(result.current.suggestions).toEqual([]);
  });

  it("reports error on other failures", async () => {
    const { calls } = stubFetch();
    const { result } = renderHook(() => useSearchSuggestions(PORTAL, "lu"));
    await advance(SEARCH_DEBOUNCE_MS);
    await act(async () => {
      calls[0].resolve(jsonResponse(500, { detail: "boom" }));
    });
    expect(result.current.status).toBe("error");
  });

  it("aborts on unmount and sends nothing afterwards", async () => {
    const { fetchMock, calls } = stubFetch();
    const { unmount } = renderHook(() => useSearchSuggestions(PORTAL, "lu"));
    await advance(SEARCH_DEBOUNCE_MS);
    unmount();
    expect(calls[0].signal?.aborted).toBe(true);
    await advance(SEARCH_DEBOUNCE_MS * 2);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("is idle and silent when disabled (modal closed)", async () => {
    const { fetchMock, calls } = stubFetch();
    const { result, rerender } = renderHook(({ open }) => useSearchSuggestions(PORTAL, "lu", open), {
      initialProps: { open: true },
    });
    await advance(SEARCH_DEBOUNCE_MS);
    expect(calls).toHaveLength(1);
    rerender({ open: false });
    expect(calls[0].signal?.aborted).toBe(true);
    await advance(SEARCH_DEBOUNCE_MS * 2);
    expect(result.current.status).toBe("idle");
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("builds the solo primary blog hrefs without the blog slug", async () => {
    const { calls } = stubFetch();
    const { result } = renderHook(() => useSearchSuggestions(SOLO_PRIMARY, "lu"));
    await advance(SEARCH_DEBOUNCE_MS);
    await act(async () => {
      calls[0].resolve(jsonResponse(200, [post("p1", "/foo/p1"), post("p2", "/post/p2")]));
    });
    expect(result.current.suggestions.map((s) => s.href)).toEqual(["/p1", "/post/p2"]);
  });
});
