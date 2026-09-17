import { getRequestConfig } from "next-intl/server";
import { cookies, headers } from "next/headers";
import { FALLBACK_LOCALE, LOCALE_COOKIE, isLocale, type Locale } from "./config";
import { getPlatformConfig } from "@/lib/server-api"; // GET /api/v1/config → { deployment_mode, default_locale, ... }

/**
 * UI locale resolution (no locale segment in the URL — content has its own locale/slug model):
 *   1. cookie set by the user's profile choice (or the header picker)
 *   2. platform default (admin → Platform settings → Default interface language; NOCT_DEFAULT_LOCALE at install)
 *   3. Accept-Language, only among supported locales
 *   4. "en"
 */
export default getRequestConfig(async () => {
  const c = (await cookies()).get(LOCALE_COOKIE)?.value;
  let locale: Locale | undefined = isLocale(c) ? c : undefined;
  if (!locale) { const cfg = await getPlatformConfig().catch(() => null); if (isLocale(cfg?.default_locale)) locale = cfg.default_locale; }
  if (!locale) { const al = (await headers()).get("accept-language") ?? ""; const hit = al.split(",").map((s) => s.trim().slice(0, 2).toLowerCase()).find(isLocale); if (hit) locale = hit; }
  locale ??= FALLBACK_LOCALE;
  return { locale, messages: (await import(`../messages/${locale}.json`)).default, timeZone: "Europe/Rome" };
});
