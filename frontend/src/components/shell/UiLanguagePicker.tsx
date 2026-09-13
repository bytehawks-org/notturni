"use client";

import { useLocale, useTranslations } from "next-intl";
import { useRouter } from "next/navigation";

import { setUiLocale } from "@/i18n/actions";
import { LOCALES, LOCALE_NAMES } from "@/i18n/config";

/** Selettore compatto per header/profilo: cambia solo la lingua
 * dell'interfaccia, mai quella dei contenuti. */
export function UiLanguagePicker({ className = "" }: { className?: string }) {
  const locale = useLocale();
  const router = useRouter();
  const t = useTranslations("Common");
  return (
    <select
      aria-label={t("interfaceLanguage")}
      value={locale}
      onChange={async (e) => {
        await setUiLocale(e.target.value);
        router.refresh();
      }}
      className={`rounded-lg border border-border bg-surface px-2 py-1 text-[13px] text-muted ${className}`}
    >
      {LOCALES.map((l) => (
        <option key={l} value={l}>
          {LOCALE_NAMES[l]}
        </option>
      ))}
    </select>
  );
}
