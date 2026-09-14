import {
  Crimson_Pro,
  Inter,
  Karla,
  Lora,
  Merriweather,
  Nunito_Sans,
  Playfair_Display,
  Source_Sans_3,
  Source_Serif_4,
  Work_Sans,
} from "next/font/google";

/** Un font per ogni voce di SERIF_FONTS/SANS_SERIF_FONTS (lib/types.ts),
 * self-hostato da Next al build (nessuna richiesta a Google a runtime — vedi
 * todo/UX_REDESIGN.md, "nessun font di terze parti nelle pagine pubbliche").
 * `next/font` richiede una chiamata statica per font: elencati tutti qui
 * invece che in un ciclo, anche se il risultato è mappato dinamicamente. */
const lora = Lora({ subsets: ["latin", "latin-ext"], weight: ["400", "500", "600"], style: ["normal", "italic"], variable: "--font-blog-lora" });
const merriweather = Merriweather({ subsets: ["latin", "latin-ext"], weight: ["400", "700"], style: ["normal", "italic"], variable: "--font-blog-merriweather" });
const playfairDisplay = Playfair_Display({ subsets: ["latin", "latin-ext"], weight: ["400", "500", "600"], style: ["normal", "italic"], variable: "--font-blog-playfair-display" });
const sourceSerif4 = Source_Serif_4({ subsets: ["latin", "latin-ext"], weight: ["400", "500", "600"], style: ["normal", "italic"], variable: "--font-blog-source-serif-4" });
const crimsonPro = Crimson_Pro({ subsets: ["latin", "latin-ext"], weight: ["400", "500", "600"], style: ["normal", "italic"], variable: "--font-blog-crimson-pro" });

const inter = Inter({ subsets: ["latin", "latin-ext"], weight: ["400", "500", "600"], variable: "--font-blog-inter" });
const nunitoSans = Nunito_Sans({ subsets: ["latin", "latin-ext"], weight: ["400", "500", "600"], variable: "--font-blog-nunito-sans" });
const workSans = Work_Sans({ subsets: ["latin", "latin-ext"], weight: ["400", "500", "600"], variable: "--font-blog-work-sans" });
const sourceSans3 = Source_Sans_3({ subsets: ["latin", "latin-ext"], weight: ["400", "500", "600"], variable: "--font-blog-source-sans-3" });
const karla = Karla({ subsets: ["latin", "latin-ext"], weight: ["400", "500", "600"], variable: "--font-blog-karla" });

/** Nome font (come salvato in `blog_configs.typography`) → classe generata da
 * next/font. La classe imposta la variabile CSS `--font-blog-*`; quale delle
 * due variabili (titoli/corpo) legge `--font-heading`/`--font-body` è deciso
 * da `BlogPageShell` in base a `typography.heading_font`/`body_font`. */
export const BLOG_FONT_CLASSES: Record<string, string> = {
  Lora: lora.variable,
  Merriweather: merriweather.variable,
  "Playfair Display": playfairDisplay.variable,
  "Source Serif 4": sourceSerif4.variable,
  "Crimson Pro": crimsonPro.variable,
  Inter: inter.variable,
  "Nunito Sans": nunitoSans.variable,
  "Work Sans": workSans.variable,
  "Source Sans 3": sourceSans3.variable,
  Karla: karla.variable,
};

/** Variabile CSS esposta da ciascuna classe sopra (stesso ordine di
 * BLOG_FONT_CLASSES), usata per far puntare `--font-heading`/`--font-body`
 * al font scelto senza dover ricostruire l'intero stack `font-family`. */
export const BLOG_FONT_VARS: Record<string, string> = {
  Lora: "--font-blog-lora",
  Merriweather: "--font-blog-merriweather",
  "Playfair Display": "--font-blog-playfair-display",
  "Source Serif 4": "--font-blog-source-serif-4",
  "Crimson Pro": "--font-blog-crimson-pro",
  Inter: "--font-blog-inter",
  "Nunito Sans": "--font-blog-nunito-sans",
  "Work Sans": "--font-blog-work-sans",
  "Source Sans 3": "--font-blog-source-sans-3",
  Karla: "--font-blog-karla",
};

/** Tutte le classi font insieme, da applicare una sola volta sul contenitore
 * di ogni pagina pubblica di blog: rende disponibili tutte le variabili
 * `--font-blog-*`, poi `BlogPageShell` sceglie quella attiva. */
export const ALL_BLOG_FONT_CLASSES = Object.values(BLOG_FONT_CLASSES).join(" ");
