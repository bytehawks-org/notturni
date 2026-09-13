"use client";

import { useTranslations } from "next-intl";

import { useTheme, type ThemeMode } from "@/lib/theme-context";

const OPTIONS: { mode: ThemeMode; icon: string }[] = [
  { mode: "light", icon: "☀" },
  { mode: "dark", icon: "☾" },
  { mode: "auto", icon: "◐" },
];

export function ThemeToggle() {
  const { mode, setMode, autoUsesLocation } = useTheme();
  const t = useTranslations("Theme");

  return (
    <div className="inline-flex items-center gap-1 rounded-full border border-border p-1">
      {OPTIONS.map((option) => (
        <button
          key={option.mode}
          type="button"
          onClick={() => setMode(option.mode)}
          title={option.mode === "auto" ? (autoUsesLocation ? t("autoLocation") : t("autoSystem")) : t(option.mode)}
          aria-pressed={mode === option.mode}
          className={`flex h-7 w-7 items-center justify-center rounded-full text-sm transition ${
            mode === option.mode ? "bg-primary text-background" : "text-muted hover:text-foreground"
          }`}
        >
          <span aria-hidden="true">{option.icon}</span>
          <span className="sr-only">{t(option.mode)}</span>
        </button>
      ))}
    </div>
  );
}
