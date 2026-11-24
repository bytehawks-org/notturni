import { describe, expect, it } from "vitest";

import { generateEnv, generateValues, PLATFORM_DEFAULTS, SOLO_DEFAULTS } from "@/lib/setup-wizard";

describe("setup wizard generator", () => {
  it("solo defaults: local storage, no redis, no nats, no moderation", () => {
    const env = generateEnv(SOLO_DEFAULTS);
    expect(env).toContain("NOCT_DEPLOYMENT_MODE=solo");
    expect(env).toContain("NOCT_SUBDOMAINS_ENABLED=false");
    expect(env).toContain("NOCT_STORAGE_BACKEND=localstorage");
    expect(env).toContain("NOCT_WORKER_INLINE=true");
    expect(env).toContain("NOCT_MODERATION_SERVICE_URL=\n");
    expect(env).not.toContain("image_moderation");
    expect(env).not.toMatch(/^NOCT_REDIS_URL=/m);
    const values = generateValues(SOLO_DEFAULTS);
    expect(values).toContain("mode: solo");
    expect(values).toContain("type: local");
    expect(values).toContain("redis:\n  enabled: false");
    expect(values).toContain("moderation:\n  enabled: false");
    expect(values).toContain("subdomains:\n  enabled: false");
  });

  it("S3 storage adds endpoint and placeholder keys, never real secrets", () => {
    const a = { ...SOLO_DEFAULTS, storage: "s3" as const };
    const env = generateEnv(a);
    expect(env).toContain("NOCT_STORAGE_BACKEND=s3");
    expect(env).toContain("NOCT_S3_SECRET_ACCESS_KEY=<generate>");
    expect(generateValues(a)).toContain("type: s3");
    expect(generateValues(a)).not.toContain("accessKey");
  });

  it("redis and nats", () => {
    const a = { ...SOLO_DEFAULTS, cache: "redis" as const, nats: true };
    expect(generateEnv(a)).toContain("NOCT_REDIS_URL=redis://redis:6379/0");
    expect(generateEnv(a)).toContain("NOCT_NATS_URL=nats://nats:4222");
    expect(generateEnv(a)).not.toContain("NOCT_WORKER_INLINE");
    expect(generateValues(a)).toContain("redis:\n  enabled: true");
    expect(generateValues(a)).toContain("nats:\n  enabled: true");
  });

  it("smtp host and relay", () => {
    const host = generateEnv({ ...SOLO_DEFAULTS, smtp: "host" });
    expect(host).toContain("NOCT_SMTP_HOST=<smtp-host>");
    expect(host).toContain("NOCT_SMTP_PASSWORD=<generate>");
    const relay = { ...SOLO_DEFAULTS, smtp: "relay" as const };
    expect(generateEnv(relay)).toContain("NOCT_SMTP_HOST=smtp-relay");
    expect(generateValues(relay)).toContain("relay:\n    enabled: true");
    expect(generateValues(relay)).not.toContain("    host:");
    expect(generateValues({ ...relay, relayHost: "smtp.example.com" })).toContain("    host: smtp.example.com");
  });

  it("platform defaults enable moderation and subdomains", () => {
    const env = generateEnv(PLATFORM_DEFAULTS);
    expect(env).toContain("NOCT_DEPLOYMENT_MODE=platform");
    expect(env).toContain("NOCT_SUBDOMAINS_ENABLED=true");
    expect(env).toContain("image_moderation");
    expect(env).toContain("NOCT_MODERATION_SERVICE_URL=http://moderation:8100");
    // chart drives image_moderation via moderation.enabled: no modules list in values
    expect(generateValues(PLATFORM_DEFAULTS)).not.toMatch(/^modules:/m);
    expect(generateValues(PLATFORM_DEFAULTS)).toContain("moderation:\n  enabled: true");
  });
});
