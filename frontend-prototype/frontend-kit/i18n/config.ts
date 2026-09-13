export const LOCALES = ["en", "it"] as const;
export type Locale = (typeof LOCALES)[number];
export const FALLBACK_LOCALE: Locale = "en";
export const LOCALE_COOKIE = "notturni_ui_locale";
export const isLocale = (v: unknown): v is Locale => typeof v === "string" && (LOCALES as readonly string[]).includes(v);
