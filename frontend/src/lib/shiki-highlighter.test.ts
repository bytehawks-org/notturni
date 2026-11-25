import { describe, expect, it, vi } from "vitest";

vi.mock("server-only", () => ({}));

import { ensureLanguagesLoaded } from "./shiki-highlighter";

describe("ensureLanguagesLoaded", () => {
  it("does not throw for unknown fence languages", async () => {
    const md = "```foobar\nx\n```\n\n```gallery\n![a](https://example.com/a.png)\n```\n";
    await expect(ensureLanguagesLoaded(md)).resolves.toBeDefined();
  });

  it("does not try to load the gallery pseudo-language", async () => {
    const hl = await ensureLanguagesLoaded("```gallery\n```\n");
    expect(hl.getLoadedLanguages()).not.toContain("gallery");
  });
});
