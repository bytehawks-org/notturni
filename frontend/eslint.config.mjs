import { defineConfig, globalIgnores } from "eslint/config";
import nextVitals from "eslint-config-next/core-web-vitals";
import nextTs from "eslint-config-next/typescript";

const eslintConfig = defineConfig([
  ...nextVitals,
  ...nextTs,
  // Override default ignores of eslint-config-next.
  globalIgnores([
    // Default ignores of eslint-config-next:
    ".next/**",
    "out/**",
    "build/**",
    "next-env.d.ts",
    // frontend/admin/ è un progetto Next.js a sé (proprio eslint.config.mjs,
    // proprio lint da lanciare separatamente) — non va attraversato da qui.
    "admin/**",
  ]),
]);

export default eslintConfig;
