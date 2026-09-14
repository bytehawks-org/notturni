"use server";
import { cookies } from "next/headers";
import { LOCALE_COOKIE, isLocale } from "./config";

/** Called by the profile "Interface language" field and by the header picker; also PATCH /users/me { ui_locale } when signed in. */
export async function setUiLocale(locale: string) {
  if (!isLocale(locale)) return;
  (await cookies()).set(LOCALE_COOKIE, locale, { path: "/", maxAge: 60 * 60 * 24 * 365, sameSite: "lax" });
}
