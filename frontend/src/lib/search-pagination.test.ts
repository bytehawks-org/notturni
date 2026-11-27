import { describe, expect, it } from "vitest";

import { SEARCH_PAGE_SIZE, parseSearchPage, searchPageHref } from "@/lib/search-pagination";

describe("parseSearchPage", () => {
  it.each([
    [undefined, 1],
    ["", 1],
    ["0", 1],
    ["-3", 1],
    ["abc", 1],
    ["1.9", 1],
    ["1", 1],
    ["4", 4],
    ["999999", 1000],
    ["1e300", 1000],
  ])("maps %j to %i", (raw, expected) => {
    expect(parseSearchPage(raw)).toBe(expected);
  });
});

describe("searchPageHref", () => {
  it("omits page for page 1", () => {
    expect(searchPageHref("", "pippo", 1)).toBe("/search?q=pippo");
  });

  it("adds page and encodes the query", () => {
    expect(searchPageHref("/diario", "a b&c", 3)).toBe("/diario/search?q=a%20b%26c&page=3");
  });

  it("uses 20 results per page", () => {
    expect(SEARCH_PAGE_SIZE).toBe(20);
  });
});
