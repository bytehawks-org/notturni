import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { blogLinks } from "@/lib/blog-path";
import { blogAddress, isSolo, siteConfig, subdomainsEnabled } from "@/lib/site";
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
