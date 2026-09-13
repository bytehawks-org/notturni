"use client";

import { useLocale, useTranslations } from "next-intl";
import { useRouter } from "next/navigation";

import { setUiLocale } from "@/i18n/actions";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { LOCALES, LOCALE_NAMES } from "@/i18n/config";

/** Selettore compatto per header/profilo: cambia solo la lingua
 * dell'interfaccia, mai quella dei contenuti. */
export function UiLanguagePicker({ className = "" }: { className?: string }) {
  const locale = useLocale();
  const router = useRouter();
  const t = useTranslations("Common");
  const { user, authFetch, refreshUser } = useAuth();
  return (
    <select
      aria-label={t("interfaceLanguage")}
      value={locale}
      onChange={async (e) => {
        const next = e.target.value;
        await setUiLocale(next);
        if (user) {
          // B6: preferenza per account, così vale anche su altri dispositivi
          await authFetch((token) => api.users.updateMe(token, { ui_locale: next })).then(refreshUser).catch(() => undefined);
        }
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
