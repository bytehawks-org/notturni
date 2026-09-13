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

/** Variabili CSS derivate da `blog_configs.palette` (mockup 3f). Restituisce
 * `null` se il blog usa la palette di default. La palette custom definisce
 * solo il tema chiaro: in modalità scura il CSS (`.blog-palette` in
 * globals.css) ripristina i valori scuri di piattaforma. */
export function paletteStyle(config: BlogConfig | null): CSSProperties | null {
  const palette = config?.palette;
  if (!palette) return null;
  const isDefault = Object.entries(palette).every(([k, v]) => DEFAULT_PALETTE[k]?.toLowerCase() === v.toLowerCase());
  if (isDefault) return null;
  const style: Record<string, string> = {};
  for (const [key, value] of Object.entries(palette)) {
    const cssVar = PALETTE_VARS[key];
    if (cssVar && /^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i.test(value)) style[cssVar] = value;
  }
  if (style["--background"] && !style["--surface"]) style["--surface"] = style["--background"];
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
