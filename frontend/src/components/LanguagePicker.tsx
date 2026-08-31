"use client";

import { COMMON_LANGUAGES, languageName } from "@/lib/languages";
import { MAX_FALLBACK_LANGUAGES } from "@/lib/types";

interface LanguagePickerProps {
  nativeLanguage: string | null;
  onNativeLanguageChange: (code: string | null) => void;
  fallbackLanguages: string[];
  onFallbackLanguagesChange: (codes: string[]) => void;
}

/** Lingua madre + lingue di fallback (anche verso cui tradurre i propri
 * contenuti), in stile fika.bar: la lingua madre come selettore singolo, le
 * altre come pillole da attivare/disattivare. */
export function LanguagePicker({
  nativeLanguage,
  onNativeLanguageChange,
  fallbackLanguages,
  onFallbackLanguagesChange,
}: LanguagePickerProps) {
  function toggleFallback(code: string) {
    if (fallbackLanguages.includes(code)) {
      onFallbackLanguagesChange(fallbackLanguages.filter((c) => c !== code));
    } else if (fallbackLanguages.length < MAX_FALLBACK_LANGUAGES) {
      onFallbackLanguagesChange([...fallbackLanguages, code]);
    }
  }

  const atLimit = fallbackLanguages.length >= MAX_FALLBACK_LANGUAGES;

  return (
    <div className="space-y-4">
      <div>
        <p className="mb-1.5 text-sm text-muted">Lingua madre</p>
        <select
          value={nativeLanguage ?? ""}
          onChange={(e) => onNativeLanguageChange(e.target.value || null)}
          className="rounded-md border border-border bg-background px-3 py-2 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-primary/40"
        >
          <option value="">Non specificata</option>
          {COMMON_LANGUAGES.map((code) => (
            <option key={code} value={code}>
              {languageName(code)}
            </option>
          ))}
        </select>
      </div>

      <div>
        <p className="mb-1.5 text-sm text-muted">
          Lingue di fallback{" "}
          <span className="text-xs">
            (anche verso cui tradurre i tuoi contenuti — massimo {MAX_FALLBACK_LANGUAGES})
          </span>
        </p>
        <div className="flex flex-wrap gap-2">
          {COMMON_LANGUAGES.filter((code) => code !== nativeLanguage).map((code) => {
            const active = fallbackLanguages.includes(code);
            const disabled = !active && atLimit;
            return (
              <button
                key={code}
                type="button"
                disabled={disabled}
                onClick={() => toggleFallback(code)}
                className={`rounded-full px-3 py-1 text-sm transition disabled:opacity-40 ${
                  active
                    ? "bg-primary text-background"
                    : "bg-foreground/5 text-foreground/70 hover:bg-foreground/10"
                }`}
              >
                {languageName(code)}
                {active && " ✓"}
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
