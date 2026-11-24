import { describe, expect, it } from "vitest";

import { slugify } from "@/lib/slug";

describe("slugify", () => {
  it("lowercases, strips accents and joins with dashes", () => {
    expect(slugify("Città  Di Notte!")).toBe("citta-di-notte");
  });

  it("trims dashes and respects maxLength", () => {
    expect(slugify("--Hello World--")).toBe("hello-world");
    expect(slugify("abcdef-ghi", 7)).toBe("abcdef");
  });
});
