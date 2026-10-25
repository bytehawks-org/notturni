/** Un font per ogni voce di SERIF_FONTS/SANS_SERIF_FONTS (lib/types.ts), self-hostato con i pacchetti
 * `@fontsource/*` (vedi app/fonts.css, importato dal layout radice): nessuna richiesta a Google né al
 * build né a runtime (nessun font di terze parti nelle pagine pubbliche).
 * Ogni classe `.font-blog-*` imposta la variabile CSS `--font-blog-*`; quale delle due variabili
 * (titoli/corpo) legge `--font-heading`/`--font-body` è deciso da `BlogPageShell` in base a
 * `typography.heading_font`/`body_font`. */

/** Nome font (come salvato in `blog_configs.typography`) → classe che definisce la sua variabile. */
export const BLOG_FONT_CLASSES: Record<string, string> = {
  "Lora": "font-blog-lora",
  "Merriweather": "font-blog-merriweather",
  "Playfair Display": "font-blog-playfair-display",
  "Source Serif 4": "font-blog-source-serif-4",
  "Crimson Pro": "font-blog-crimson-pro",
  "Inter": "font-blog-inter",
  "Nunito Sans": "font-blog-nunito-sans",
  "Work Sans": "font-blog-work-sans",
  "Source Sans 3": "font-blog-source-sans-3",
  "Karla": "font-blog-karla",
  "JetBrains Mono": "font-blog-jetbrains-mono",
  "Fira Code": "font-blog-fira-code",
  "IBM Plex Mono": "font-blog-ibm-plex-mono",
  "Source Code Pro": "font-blog-source-code-pro",
  "Space Mono": "font-blog-space-mono",
};

/** Variabile CSS esposta da ciascuna classe sopra, usata per far puntare `--font-heading`/`--font-body`
 * al font scelto senza dover ricostruire l'intero stack `font-family`. */
export const BLOG_FONT_VARS: Record<string, string> = {
  "Lora": "--font-blog-lora",
  "Merriweather": "--font-blog-merriweather",
  "Playfair Display": "--font-blog-playfair-display",
  "Source Serif 4": "--font-blog-source-serif-4",
  "Crimson Pro": "--font-blog-crimson-pro",
  "Inter": "--font-blog-inter",
  "Nunito Sans": "--font-blog-nunito-sans",
  "Work Sans": "--font-blog-work-sans",
  "Source Sans 3": "--font-blog-source-sans-3",
  "Karla": "--font-blog-karla",
  "JetBrains Mono": "--font-blog-jetbrains-mono",
  "Fira Code": "--font-blog-fira-code",
  "IBM Plex Mono": "--font-blog-ibm-plex-mono",
  "Source Code Pro": "--font-blog-source-code-pro",
  "Space Mono": "--font-blog-space-mono",
};

/** Tutte le classi font insieme, da applicare una sola volta sul contenitore di ogni pagina pubblica di
 * blog: rende disponibili tutte le variabili `--font-blog-*`, poi `BlogPageShell` sceglie quella attiva. */
export const ALL_BLOG_FONT_CLASSES = Object.values(BLOG_FONT_CLASSES).join(" ");
