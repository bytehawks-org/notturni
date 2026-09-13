import { getRequestConfig } from "next-intl/server";
import { cookies, headers } from "next/headers";

import { FALLBACK_LOCALE, LOCALE_COOKIE, isLocale, type Locale } from "./config";

/**
 * Risoluzione della lingua dell'interfaccia (nessun segmento di lingua
 * nell'URL: i contenuti hanno già il proprio modello locale/slug):
 *   1. cookie impostato dal selettore nell'header o dal profilo
 *   2. Accept-Language, solo tra le lingue supportate
 *   3. FALLBACK_LOCALE
 * Il default di piattaforma (`platform_config.default_locale`, mockup 5f) e
 * `users.ui_locale` arriveranno con il blocco backend B6 — qui andranno
 * inseriti tra il punto 1 e il 2.
 */
export default getRequestConfig(async () => {
  const fromCookie = (await cookies()).get(LOCALE_COOKIE)?.value;
  let locale: Locale | undefined = isLocale(fromCookie) ? fromCookie : undefined;
  if (!locale) {
    const acceptLanguage = (await headers()).get("accept-language") ?? "";
    locale = acceptLanguage
      .split(",")
      .map((part) => part.trim().slice(0, 2).toLowerCase())
      .find(isLocale);
  }
  locale ??= FALLBACK_LOCALE;
  return {
    locale,
    messages: (await import(`../messages/${locale}.json`)).default,
    timeZone: "Europe/Rome",
  };
});
