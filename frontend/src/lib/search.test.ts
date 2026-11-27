import { describe, expect, it } from "vitest";

import { normalizeQuery, resultHref, resultsHref, type SearchScope } from "@/lib/search";

const PORTAL: SearchScope = { kind: "portal" };
const SUB_BLOG: SearchScope = { kind: "blog", slug: "foo", basePath: "/foo" };
const SOLO_PRIMARY: SearchScope = { kind: "blog", slug: "foo", basePath: "" };

describe("resultsHref", () => {
  it("portal: /search with the encoded query", () => {
    expect(resultsHref(PORTAL, "luna piena & mare")).toBe("/search?q=luna%20piena%20%26%20mare");
  });

  it("blog with a path prefix: prefixed search page", () => {
    expect(resultsHref(SUB_BLOG, "a")).toBe("/foo/search?q=a");
  });

  it("solo primary blog on the apex: search page at the root", () => {
    expect(resultsHref(SOLO_PRIMARY, "x")).toBe("/search?q=x");
  });
});

describe("resultHref", () => {
  it("portal keeps the permalink as returned by the API", () => {
    expect(resultHref(PORTAL, { permalink: "/foo/post-1" })).toBe("/foo/post-1");
  });

  it("blog with a path prefix keeps the permalink (already prefixed)", () => {
    expect(resultHref(SUB_BLOG, { permalink: "/foo/post-1" })).toBe("/foo/post-1");
  });

  it("solo primary blog strips the blog slug", () => {
    expect(resultHref(SOLO_PRIMARY, { permalink: "/foo/post-1" })).toBe("/post-1");
  });

  it("solo primary blog leaves /post/... permalinks untouched", () => {
    expect(resultHref(SOLO_PRIMARY, { permalink: "/post/abc" })).toBe("/post/abc");
  });

  it("solo primary blog leaves permalinks of other blogs untouched", () => {
    expect(resultHref(SOLO_PRIMARY, { permalink: "/bar/post-1" })).toBe("/bar/post-1");
  });
});

describe("normalizeQuery", () => {
  it("trims the query and caps its length", () => {
    expect(normalizeQuery("  ciao  ")).toBe("ciao");
    expect(normalizeQuery("a".repeat(150))).toHaveLength(100);
    expect(normalizeQuery("b".repeat(99) + " c")).toBe("b".repeat(99));
  });
});
