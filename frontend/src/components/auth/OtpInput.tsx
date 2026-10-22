"use client";

import { useRef, type ClipboardEvent, type KeyboardEvent } from "react";

const OTP_LENGTH = 6;

/** Sei caselle per cifra (mockup 1h), condiviso da login e reset password.
 * `legend`/`digitLabel` sono a carico del chiamante (entrambi via i18n
 * `next-intl`): componente condiviso da flussi in più lingue, un testo
 * fisso qui annuncerebbe sempre in italiano allo screen reader anche in
 * pagine EN (bug segnalato dalla review Copilot). */
export function OtpInput({
  value,
  onChange,
  legend = "Codice di verifica a 6 cifre",
  digitLabel = (index, total) => `Cifra ${index} di ${total}`,
}: {
  value: string;
  onChange: (value: string) => void;
  legend?: string;
  digitLabel?: (index: number, total: number) => string;
}) {
  const refs = useRef<(HTMLInputElement | null)[]>([]);
  const digits = Array.from({ length: OTP_LENGTH }, (_, i) => value[i] ?? "");

  function setDigit(index: number, digit: string) {
    const next = digits.slice();
    next[index] = digit;
    onChange(next.join(""));
    if (digit && index < OTP_LENGTH - 1) refs.current[index + 1]?.focus();
  }

  function handleKeyDown(index: number, event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Backspace" && !digits[index] && index > 0) {
      refs.current[index - 1]?.focus();
    }
  }

  function handlePaste(event: ClipboardEvent<HTMLInputElement>) {
    const pasted = event.clipboardData.getData("text").replace(/\D/g, "").slice(0, OTP_LENGTH);
    if (!pasted) return;
    event.preventDefault();
    onChange(pasted.padEnd(value.length, ""));
    refs.current[Math.min(pasted.length, OTP_LENGTH - 1)]?.focus();
  }

  return (
    <fieldset className="grid grid-cols-6 gap-2 border-0 p-0 m-0">
      <legend className="sr-only">{legend}</legend>
      {digits.map((digit, i) => (
        <input
          key={i}
          ref={(el) => {
            refs.current[i] = el;
          }}
          type="text"
          inputMode="numeric"
          maxLength={1}
          autoFocus={i === 0}
          value={digit}
          onChange={(e) => setDigit(i, e.target.value.replace(/\D/g, "").slice(-1))}
          onKeyDown={(e) => handleKeyDown(i, e)}
          onPaste={handlePaste}
          aria-label={digitLabel(i + 1, OTP_LENGTH)}
          className="h-[52px] rounded-xl border border-border bg-surface text-center font-serif text-2xl text-foreground focus:outline-none focus:border-primary focus:ring-[3px] focus:ring-primary/20"
        />
      ))}
    </fieldset>
  );
}

export const OTP_INPUT_LENGTH = OTP_LENGTH;
