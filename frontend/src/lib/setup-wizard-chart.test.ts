import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { parse } from "yaml";

import { generateValues, PLATFORM_DEFAULTS, SOLO_DEFAULTS, type SetupAnswers } from "@/lib/setup-wizard";

const chart = parse(readFileSync(resolve(__dirname, "../../../helm/notturni/values.yaml"), "utf8")) as Record<string, unknown>;

function leafPaths(o: unknown, prefix = ""): string[] {
  if (o && typeof o === "object" && !Array.isArray(o) && Object.keys(o).length) {
    return Object.entries(o).flatMap(([k, v]) => leafPaths(v, prefix ? `${prefix}.${k}` : k));
  }
  return [prefix];
}
function get(o: unknown, path: string): unknown {
  return path.split(".").reduce<unknown>((acc, k) => (acc && typeof acc === "object" ? (acc as Record<string, unknown>)[k] : undefined), o);
}
function has(o: unknown, path: string): boolean {
  const parts = path.split(".");
  let cur: unknown = o;
  for (const k of parts) {
    if (!cur || typeof cur !== "object" || !(k in (cur as object))) return false;
    cur = (cur as Record<string, unknown>)[k];
  }
  return true;
}

const variants: Record<string, SetupAnswers> = {
  solo: SOLO_DEFAULTS,
  platform: PLATFORM_DEFAULTS,
  s3: { ...SOLO_DEFAULTS, storage: "s3" },
  garage: { ...PLATFORM_DEFAULTS, storage: "garage" },
  relay: { ...SOLO_DEFAULTS, smtp: "relay", relayHost: "smtp.example.com" },
  smtpHost: { ...SOLO_DEFAULTS, smtp: "host" },
};

describe("wizard output matches helm/notturni/values.yaml", () => {
  for (const [name, a] of Object.entries(variants)) {
    it(`${name}: every emitted key exists in the chart`, () => {
      const out = parse(generateValues(a));
      for (const p of leafPaths(out)) expect(has(chart, p), p).toBe(true);
    });
  }

  it("emits required keys for solo and platform", () => {
    for (const a of [SOLO_DEFAULTS, PLATFORM_DEFAULTS]) {
      const out = parse(generateValues(a));
      expect(get(out, "mode")).toBe(a.mode);
      expect(String(get(out, "ingress.host"))).not.toBe("");
      expect(get(out, "storage.type")).toBeTruthy();
    }
  });

  it("external S3 has endpoint, public URL and region; Garage enables the chart component", () => {
    const s3 = parse(generateValues(variants.s3));
    for (const k of ["endpointUrl", "publicUrl", "region"]) expect(get(s3, `storage.s3.${k}`)).toBeTruthy();
    expect(get(parse(generateValues(variants.garage)), "storage.s3.garage.enabled")).toBe(true);
  });

  it("emits no modules and no credential values", () => {
    for (const a of Object.values(variants)) {
      const text = generateValues(a);
      expect(has(parse(text), "modules")).toBe(false);
      expect(text).not.toMatch(/<generate>|password:|accessKey|secretKey/i);
    }
    expect(generateValues(variants.s3)).toContain("#   NOCT_S3_SECRET_ACCESS_KEY");
  });

  it("admin.email only for platform", () => {
    expect(has(parse(generateValues(SOLO_DEFAULTS)), "admin.email")).toBe(false);
    expect(has(parse(generateValues(PLATFORM_DEFAULTS)), "admin.email")).toBe(true);
  });
});
