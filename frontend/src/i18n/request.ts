import { getRequestConfig } from "next-intl/server";
import { cookies, headers } from "next/headers";

import { FALLBACK_LOCALE, LOCALE_COOKIE, isLocale, type Locale } from "./config";

/**
 * Risoluzione della lingua dell'interfaccia (nessun segmento di lingua
 * nell'URL: i contenuti hanno già il proprio modello locale/slug):
 *   1. cookie impostato dal selettore nell'header o dal profilo
 *   2. default di piattaforma (`GET /api/v1/config`, `platform_config.default_locale`)
 *   3. Accept-Language, solo tra le lingue supportate
 *   4. FALLBACK_LOCALE
 * La preferenza per account (`users.ui_locale`) viene copiata nel cookie
 * dal client al login e quando cambia (AuthProvider/UiLanguagePicker): il
 * server non ha l'access token, che vive solo in memoria nel browser.
 */
const BACKEND_INTERNAL_URL =
  process.env.NOCT_BACKEND_INTERNAL_URL || process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000";

async function platformDefaultLocale(): Promise<Locale | undefined> {
  try {
    const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/config`, { next: { revalidate: 300 } });
    if (!res.ok) return undefined;
    const data = (await res.json()) as { default_locale?: string };
    return isLocale(data.default_locale) ? data.default_locale : undefined;
  } catch {
    return undefined;
  }
}
export default getRequestConfig(async () => {
  const fromCookie = (await cookies()).get(LOCALE_COOKIE)?.value;
  let locale: Locale | undefined = isLocale(fromCookie) ? fromCookie : undefined;
  if (!locale) locale = await platformDefaultLocale();
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
