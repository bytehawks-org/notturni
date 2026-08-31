/** Lingue comuni proposte nei selettori (lingua madre / lingue di fallback
 * del profilo, vedi ROADMAP.md). Non è l'elenco ISO 639-1 completo — quello
 * accettato dal backend è qualsiasi codice di 2 lettere valido
 * (app/domain/i18n.py) — solo le più diffuse, per un menu comodo. */
export const COMMON_LANGUAGES = [
  "it",
  "en",
  "fr",
  "de",
  "es",
  "pt",
  "nl",
  "pl",
  "ro",
  "sv",
  "el",
  "ru",
  "uk",
  "tr",
  "ar",
  "hi",
  "ja",
  "zh",
  "ko",
];

const displayNames = new Intl.DisplayNames(["it"], { type: "language" });

export function languageName(code: string): string {
  try {
    const name = displayNames.of(code);
    return name ? name.charAt(0).toUpperCase() + name.slice(1) : code;
  } catch {
    return code;
  }
}
