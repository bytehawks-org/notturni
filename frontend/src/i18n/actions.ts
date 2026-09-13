"use server";

import { cookies } from "next/headers";

import { LOCALE_COOKIE, isLocale } from "./config";

/** Cambia solo la lingua dell'interfaccia (cookie, 1 anno). Chiamata dal
 * selettore nell'header e dal profilo. `users.ui_locale` (B6) si aggiungerà qui. */
export async function setUiLocale(locale: string): Promise<void> {
  if (!isLocale(locale)) return;
  (await cookies()).set(LOCALE_COOKIE, locale, { path: "/", maxAge: 60 * 60 * 24 * 365, sameSite: "lax" });
}
