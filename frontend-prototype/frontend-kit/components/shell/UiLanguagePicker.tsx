"use client";
import { useLocale } from "next-intl";
import { useRouter } from "next/navigation";
import { LOCALES } from "@/i18n/config";
import { setUiLocale } from "@/i18n/actions";

const NAMES: Record<string, string> = { en: "English", it: "Italiano" };

/** Compact select for the site header / profile. Changes only the interface language, never the content. */
export function UiLanguagePicker({ className = "" }: { className?: string }) {
  const locale = useLocale();
  const router = useRouter();
  return (
    <select aria-label="Interface language" value={locale} onChange={async (e) => { await setUiLocale(e.target.value); router.refresh(); }} className={`rounded-lg border border-border bg-surface px-2 py-1 text-[13px] text-muted ${className}`}>
      {LOCALES.map((l) => <option key={l} value={l}>{NAMES[l]}</option>)}
    </select>
  );
}
