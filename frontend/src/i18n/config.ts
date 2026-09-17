/** Lingue dell'interfaccia (etichette, pulsanti, messaggi). Distinte dalla
 * lingua dei contenuti (post/pagine), che ha il proprio modello locale+slug
 * lato backend. Nuova lingua = nuovo file in `src/messages/` + voce qui. */
export const LOCALES = ["it", "en"] as const;
export type Locale = (typeof LOCALES)[number];
/** Ultimo fallback: l'app oggi è nata in italiano e le stringhe non ancora
 * migrate a next-intl sono italiane — un fallback "en" mischierebbe le lingue. */
export const FALLBACK_LOCALE: Locale = "it";
export const LOCALE_COOKIE = "notturni_ui_locale";
export const LOCALE_NAMES: Record<Locale, string> = { it: "Italiano", en: "English" };
export const isLocale = (v: unknown): v is Locale =>
  typeof v === "string" && (LOCALES as readonly string[]).includes(v);
