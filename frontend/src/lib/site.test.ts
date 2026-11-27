import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { blogLinks } from "@/lib/blog-path";
import { blogAddress, isSolo, postHref, siteConfig, spaceHref, subdomainsEnabled } from "@/lib/site";
import { mockHeaders } from "@/test/next-mocks";

beforeEach(() => {
  vi.stubEnv("NOCT_SITE_URL", "https://example.com");
  vi.stubEnv("NOCT_SUBDOMAINS_ENABLED", "");
  vi.stubEnv("NOCT_DEPLOYMENT_MODE", "");
});

afterEach(() => vi.unstubAllEnvs());

describe("NOCT_SUBDOMAINS_ENABLED parsing", () => {
  it("defaults to true when unset or empty", () => {
    expect(subdomainsEnabled()).toBe(true);
    vi.stubEnv("NOCT_SUBDOMAINS_ENABLED", undefined as unknown as string);
    expect(subdomainsEnabled()).toBe(true);
  });

  it.each(["true", "1", "yes", "anything"])("treats %s as enabled", (v) => {
    vi.stubEnv("NOCT_SUBDOMAINS_ENABLED", v);
    expect(subdomainsEnabled()).toBe(true);
  });

  it.each(["false", "FALSE", "0", "no", "off", " off "])("treats %s as disabled", (v) => {
    vi.stubEnv("NOCT_SUBDOMAINS_ENABLED", v);
    expect(subdomainsEnabled()).toBe(false);
  });

  it("does not default to false in solo mode when unset", () => {
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "solo");
    expect(subdomainsEnabled()).toBe(true);
  });
});

describe("NOCT_DEPLOYMENT_MODE parsing", () => {
  it("is platform by default and for unknown values", () => {
    expect(siteConfig().deploymentMode).toBe("platform");
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "other");
    expect(isSolo()).toBe(false);
  });

  it("is solo for solo", () => {
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "solo");
    expect(isSolo()).toBe(true);
    expect(siteConfig().deploymentMode).toBe("solo");
  });
});

describe("blogAddress", () => {
  it("uses {slug}.{host} with subdomains enabled", () => {
    expect(blogAddress("foo")).toBe("foo.example.com");
  });

  it("uses {siteUrl}/{slug} with subdomains disabled", () => {
    vi.stubEnv("NOCT_SUBDOMAINS_ENABLED", "false");
    expect(blogAddress("foo")).toBe("https://example.com/foo");
  });
});

describe("blogLinks", () => {
  it("enabled: on the blog subdomain the base path is empty and the permalink prefix is stripped", async () => {
    mockHeaders.set("host", "foo.example.com");
    const links = await blogLinks("foo");
    expect(links.basePath).toBe("");
    expect(links.fromPermalink("/foo/post-1")).toBe("/post-1");
  });

  it("enabled: on the apex host links stay path-based", async () => {
    mockHeaders.set("host", "example.com");
    const links = await blogLinks("foo");
    expect(links.basePath).toBe("/foo");
    expect(links.fromPermalink("/foo/post-1")).toBe("/foo/post-1");
  });

  it("disabled: a subdomain host is never treated as the blog host", async () => {
    vi.stubEnv("NOCT_SUBDOMAINS_ENABLED", "false");
    mockHeaders.set("host", "foo.example.com");
    const links = await blogLinks("foo");
    expect(links.basePath).toBe("/foo");
    expect(links.fromPermalink("/foo/post-1")).toBe("/foo/post-1");
  });
});

describe("blogLinks with the solo primary blog", () => {
  beforeEach(() => vi.stubEnv("NOCT_DEPLOYMENT_MODE", "solo"));

  it("primary on the apex host: basePath is empty and homeHref is /", async () => {
    mockHeaders.set("host", "example.com");
    const links = await blogLinks("foo", "foo");
    expect(links.basePath).toBe("");
    expect(links.homeHref).toBe("/");
  });

  it("primary on its subdomain: basePath is empty and homeHref is /", async () => {
    mockHeaders.set("host", "foo.example.com");
    const links = await blogLinks("foo", "foo");
    expect(links.basePath).toBe("");
    expect(links.homeHref).toBe("/");
  });

  it("primary on the apex: /post/... permalinks are unchanged", async () => {
    mockHeaders.set("host", "example.com");
    const links = await blogLinks("foo", "foo");
    expect(links.fromPermalink("/post/post-1")).toBe("/post/post-1");
  });

  it("primary on the apex: /{primary}/... permalinks lose the slug prefix", async () => {
    mockHeaders.set("host", "example.com");
    const links = await blogLinks("foo", "foo");
    expect(links.fromPermalink("/foo/post-1")).toBe("/post-1");
  });

  it("another blog on the apex is unchanged", async () => {
    mockHeaders.set("host", "example.com");
    const links = await blogLinks("bar", "foo");
    expect(links.basePath).toBe("/bar");
    expect(links.homeHref).toBe("/bar");
    expect(links.fromPermalink("/bar/post-1")).toBe("/bar/post-1");
  });

  it("another blog on its subdomain is unchanged", async () => {
    mockHeaders.set("host", "bar.example.com");
    const links = await blogLinks("bar", "foo");
    expect(links.basePath).toBe("");
    expect(links.homeHref).toBe("/");
    expect(links.fromPermalink("/bar/post-1")).toBe("/post-1");
  });
});

describe("spaceHref", () => {
  it("solo primary: the suffix is used at the root", () => {
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "solo");
    expect(spaceHref("foo", "", "foo")).toBe("/");
    expect(spaceHref("foo", "/media", "foo")).toBe("/media");
  });

  it("solo other blog: keeps the /{blog} prefix", () => {
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "solo");
    expect(spaceHref("bar", "", "foo")).toBe("/bar");
    expect(spaceHref("bar", "/x", "foo")).toBe("/bar/x");
  });

  it("solo without primary slug: keeps the /{blog} prefix", () => {
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "solo");
    expect(spaceHref("foo", "/x", null)).toBe("/foo/x");
  });

  it("platform: the primary slug is ignored", () => {
    expect(spaceHref("foo", "", "foo")).toBe("/foo");
    expect(spaceHref("foo", "/x", "foo")).toBe("/foo/x");
  });
});

describe("postHref", () => {
  it("solo: the primary blog uses /post/{postSlug}", () => {
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "solo");
    expect(postHref("foo", "post-1", "foo")).toBe("/post/post-1");
  });

  it("solo: other blogs keep /{blog}/{post}", () => {
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "solo");
    expect(postHref("bar", "post-1", "foo")).toBe("/bar/post-1");
  });

  it("solo: no primary slug keeps /{blog}/{post}", () => {
    vi.stubEnv("NOCT_DEPLOYMENT_MODE", "solo");
    expect(postHref("foo", "post-1", null)).toBe("/foo/post-1");
    expect(postHref("foo", "post-1", undefined)).toBe("/foo/post-1");
  });

  it("platform: outputs are unchanged, primary slug ignored", () => {
    expect(postHref("foo", "post-1", null)).toBe("/foo/post-1");
    expect(postHref("foo", "post-1", "foo")).toBe("/foo/post-1");
  });
});
