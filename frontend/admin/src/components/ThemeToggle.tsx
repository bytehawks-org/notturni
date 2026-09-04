"use client";

import { useTheme, type ThemeMode } from "@/lib/theme-context";

const OPTIONS: { mode: ThemeMode; label: string; icon: string }[] = [
  { mode: "light", label: "Chiaro", icon: "☀" },
  { mode: "dark", label: "Scuro", icon: "☾" },
  { mode: "auto", label: "Automatico", icon: "◐" },
];

export function ThemeToggle() {
  const { mode, setMode, autoUsesLocation } = useTheme();

  return (
    <div className="inline-flex items-center gap-1 rounded-full border border-border p-1">
      {OPTIONS.map((option) => (
        <button
          key={option.mode}
          type="button"
          onClick={() => setMode(option.mode)}
          title={
            option.mode === "auto"
              ? autoUsesLocation
                ? "Segue alba/tramonto della tua posizione"
                : "Segue le preferenze del sistema (posizione non disponibile)"
              : option.label
          }
          aria-pressed={mode === option.mode}
          className={`flex h-7 w-7 items-center justify-center rounded-full text-sm transition ${
            mode === option.mode
              ? "bg-primary text-background"
              : "text-muted hover:text-foreground"
          }`}
        >
          <span aria-hidden="true">{option.icon}</span>
          <span className="sr-only">{option.label}</span>
        </button>
      ))}
    </div>
  );
}
