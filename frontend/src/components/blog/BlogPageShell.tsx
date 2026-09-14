import type { CSSProperties, ReactNode } from "react";

import { BLOG_FONT_CLASSES, BLOG_FONT_VARS } from "@/lib/blog-fonts";
import type { BlogConfig } from "@/lib/types";

/** Copia di `DEFAULT_BLOG_CONFIG.palette`/`typography` (backend/app/domain/blog_config.py):
 * un blog mai personalizzato deve restare identico allo shell di piattaforma,
 * quindi palette e font di default non vengono iniettati. */
const DEFAULT_PALETTE: Record<string, string> = {
  background: "#fbf9f6",
  foreground: "#2b2a28",
  primary: "#3e6259",
  muted: "#a8a29a",
  border: "#e7e2da",
};
const DEFAULT_HEADING_FONT = "Lora";
const DEFAULT_BODY_FONT = "Source Sans 3";

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

/** `typography.body_size` valido (mockup 2e): 17/18/19px, corpo di default 18. */
export function blogBodySize(config: BlogConfig | null): string {
  const value = config?.typography?.body_size;
  return typeof value === "string" && ["17", "18", "19"].includes(value) ? value : "18";
}

/** `typography.measure` valido: colonna di lettura stretta o normale. */
export function blogMeasure(config: BlogConfig | null): "narrow" | "normal" {
  return config?.typography?.measure === "narrow" ? "narrow" : "normal";
}

/** `layout` del blog (mockup 3f, `AppearanceTab`): influenza la disposizione
 * del feed (standard = righe, magazine = griglia con copertina, minimal =
 * lista compatta senza copertina) — vedi `FeedPostCard`. */
export function blogLayout(config: BlogConfig | null): "standard" | "magazine" | "minimal" {
  const value = config?.layout;
  return value === "magazine" || value === "minimal" ? value : "standard";
}

/** Font titoli/corpo (mockup 2e) come variabili CSS `--font-heading`/`--font-body`
 * puntate al font scelto tra quelli curati (`SERIF_FONTS`/`SANS_SERIF_FONTS`,
 * self-hostati da `lib/blog-fonts.ts`); più la classe che rende disponibili
 * quelle variabili. `null` se il blog usa i font di default di piattaforma
 * (nessuna classe/variabile aggiuntiva da caricare). */
function typographyStyle(config: BlogConfig | null): { style: CSSProperties; className: string } | null {
  const typography = config?.typography ?? {};
  const headingFont = typeof typography.heading_font === "string" ? typography.heading_font : DEFAULT_HEADING_FONT;
  const bodyFont = typeof typography.body_font === "string" ? typography.body_font : DEFAULT_BODY_FONT;
  if (headingFont === DEFAULT_HEADING_FONT && bodyFont === DEFAULT_BODY_FONT) return null;
  const headingVar = BLOG_FONT_VARS[headingFont];
  const bodyVar = BLOG_FONT_VARS[bodyFont];
  const style: Record<string, string> = {};
  if (headingVar) style["--font-heading"] = `var(${headingVar})`;
  if (bodyVar) style["--font-body"] = `var(${bodyVar})`;
  const className = [BLOG_FONT_CLASSES[headingFont], BLOG_FONT_CLASSES[bodyFont]].filter(Boolean).join(" ");
  return { style: style as CSSProperties, className };
}

/** Wrapper delle pagine pubbliche di un blog: applica palette e tipografia
 * custom come variabili CSS sulla root (mockup 2e/3f), così tutto ciò che sta
 * dentro (header, feed, post) legge colori e font del blog. `--blog-body-size`
 * è letto dalla pagina del post per la dimensione del corpo del testo. */
export function BlogPageShell({ config, children }: { config: BlogConfig | null; children: ReactNode }) {
  const palette = paletteStyle(config);
  const typography = typographyStyle(config);
  const style: Record<string, string> = {
    ...(palette as Record<string, string> | null),
    ...(typography?.style as Record<string, string> | undefined),
    "--blog-body-size": `${blogBodySize(config)}px`,
  };
  const className = ["flex flex-1 flex-col bg-background text-foreground", palette ? "blog-palette" : "", typography?.className ?? ""]
    .filter(Boolean)
    .join(" ");
  return (
    <div className={className} style={style as CSSProperties}>
      {children}
    </div>
  );
}
