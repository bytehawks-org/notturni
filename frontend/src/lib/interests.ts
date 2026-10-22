import type { Interest } from "./types";

/** Etichetta di un interesse nella lingua corrente (blocco "interessi
 * utente"): fallback en, poi la prima traduzione disponibile, poi la
 * chiave canonica stessa se l'interesse non è (più) tra quelli configurati. */
export function interestLabel(interest: Interest | undefined, key: string, locale: string): string {
  if (!interest) return key;
  return interest.translations[locale] ?? interest.translations.en ?? Object.values(interest.translations)[0] ?? key;
}

export function interestsByKey(interests: Interest[]): Map<string, Interest> {
  return new Map(interests.map((i) => [i.key, i]));
}
