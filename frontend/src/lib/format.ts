/** Formattazione date/numeri nella lingua dell'interfaccia (next-intl
 * `useLocale()`/`getLocale()`), al posto dei vari `toLocaleDateString("it-IT")`. */
export function formatDate(iso: string, locale: string, opts: Intl.DateTimeFormatOptions = { day: "numeric", month: "long", year: "numeric" }): string {
  return new Date(iso).toLocaleDateString(locale, opts);
}

export function formatDateTime(iso: string, locale: string): string {
  return new Date(iso).toLocaleString(locale, { day: "numeric", month: "short", year: "numeric", hour: "2-digit", minute: "2-digit" });
}

/** Tempo di lettura stimato (≈200 parole/min), minimo 1. */
export function readingMinutes(text: string): number {
  const words = text.trim().split(/\s+/).filter(Boolean).length;
  return Math.max(1, Math.round(words / 200));
}
