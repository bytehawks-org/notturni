import type { CSSProperties, ReactNode } from "react";

import type { BlogConfig } from "@/lib/types";

/** Copia di `DEFAULT_BLOG_CONFIG.palette` (backend/app/domain/blog_config.py):
 * un blog mai personalizzato deve restare identico allo shell di piattaforma,
 * quindi la palette di default non viene iniettata. */
const DEFAULT_PALETTE: Record<string, string> = {
  background: "#fbf9f6",
  foreground: "#2b2a28",
  primary: "#3e6259",
  muted: "#a8a29a",
  border: "#e7e2da",
};

const PALETTE_VARS: Record<string, string> = {
  background: "--background",
  surface: "--surface",
  foreground: "--foreground",
  primary: "--primary",
  muted: "--muted",
  border: "--border-color",
};

const HEX_RE = /^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i;

/** Variabili CSS derivate da `blog_configs.palette` (mockup 3f). Restituisce
 * `null` se il blog usa la palette di default e non ha una variante scura.
 * La palette chiara vale in tema chiaro; `palette_dark` (se presente) viene
 * esposta come `--blog-dark-*` e applicata in tema scuro dal CSS
 * `.blog-palette` in globals.css, altrimenti vale la palette scura di
 * piattaforma. */
export function paletteStyle(config: BlogConfig | null): CSSProperties | null {
  const palette = config?.palette;
  const dark = config?.palette_dark;
  const lightIsDefault =
    !palette || Object.entries(palette).every(([k, v]) => DEFAULT_PALETTE[k]?.toLowerCase() === v.toLowerCase());
  if (lightIsDefault && !dark) return null;
  const style: Record<string, string> = {};
  if (palette && !lightIsDefault) {
    for (const [key, value] of Object.entries(palette)) {
      const cssVar = PALETTE_VARS[key];
      if (cssVar && HEX_RE.test(value)) style[cssVar] = value;
    }
    if (style["--background"] && !style["--surface"]) style["--surface"] = style["--background"];
  }
  if (dark) {
    for (const [key, value] of Object.entries(dark)) {
      if (PALETTE_VARS[key] && HEX_RE.test(value)) style[`--blog-dark-${key}`] = value;
    }
    if (style["--blog-dark-background"] && !style["--blog-dark-surface"]) style["--blog-dark-surface"] = style["--blog-dark-background"];
  }
  return style as CSSProperties;
}

/** Wrapper delle pagine pubbliche di un blog: applica la palette custom come
 * variabili CSS sulla root (mockup 3f), così tutto ciò che sta dentro
 * (header, feed, post) legge i colori del blog. */
export function BlogPageShell({ config, children }: { config: BlogConfig | null; children: ReactNode }) {
  const style = paletteStyle(config);
  return (
    <div className={`flex flex-1 flex-col bg-background text-foreground ${style ? "blog-palette" : ""}`} style={style ?? undefined}>
      {children}
    </div>
  );
}
