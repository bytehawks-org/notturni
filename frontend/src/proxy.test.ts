import { NextRequest } from "next/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { proxy } from "@/proxy";

function req(host: string, path = "/") {
  return new NextRequest(`http://${host}${path}`, { headers: { host } });
}

const connectSrc = (res: Response) =>
  (res.headers.get("Content-Security-Policy") ?? "").match(/connect-src ([^;]*);/)?.[1] ?? "";

beforeEach(() => {
  vi.stubEnv("NOCT_SITE_URL", "https://example.com");
  vi.stubEnv("NOCT_API_PUBLIC_URL", "https://example.com");
  vi.stubEnv("NOCT_SUBDOMAINS_ENABLED", "");
});

afterEach(() => vi.unstubAllEnvs());

describe("proxy with subdomains enabled (default)", () => {
  it("rewrites {slug}.{host} to /{slug}", () => {
    const res = proxy(req("foo.example.com", "/post-1"));
    expect(res.headers.get("x-middleware-rewrite")).toContain("/foo/post-1");
  });

  it("keeps platform-only paths and the apex untouched", () => {
    expect(proxy(req("foo.example.com", "/login")).headers.get("x-middleware-rewrite")).toBeNull();
    expect(proxy(req("example.com", "/foo")).headers.get("x-middleware-rewrite")).toBeNull();
  });

  it("does not rewrite reserved subdomains", () => {
    expect(proxy(req("www.example.com", "/")).headers.get("x-middleware-rewrite")).toBeNull();
  });

  it("includes the wildcard origin in connect-src", () => {
    expect(connectSrc(proxy(req("example.com")))).toContain("https://*.example.com");
  });
});

describe("proxy with subdomains disabled", () => {
  beforeEach(() => vi.stubEnv("NOCT_SUBDOMAINS_ENABLED", "false"));

  it("never rewrites by host", () => {
    const res = proxy(req("foo.example.com", "/post-1"));
    expect(res.headers.get("x-middleware-rewrite")).toBeNull();
    expect(res.headers.get("x-middleware-next")).toBe("1");
  });

  it("has no wildcard origin in connect-src but keeps the site and API origins", () => {
    const value = connectSrc(proxy(req("foo.example.com")));
    expect(value).not.toContain("*.");
    expect(value).toContain("'self' https://example.com");
  });

  it("still sets a per-request nonce CSP", () => {
    const csp = proxy(req("example.com")).headers.get("Content-Security-Policy") ?? "";
    expect(csp).toMatch(/script-src 'self' 'nonce-[^']+' 'strict-dynamic'/);
  });
});

describe("storage origins in img-src", () => {
  it("adds the S3 and local storage public origins", () => {
    vi.stubEnv("NOCT_S3_PUBLIC_URL", "http://192.168.1.10:9000/bucket");
    vi.stubEnv("NOCT_LOCAL_STORAGE_PUBLIC_URL", "http://192.168.1.11:8080/media");
    const csp = proxy(req("example.com")).headers.get("Content-Security-Policy") ?? "";
    expect(csp).toContain("img-src 'self' data: https: http://192.168.1.10:9000 http://192.168.1.11:8080;");
  });
});
