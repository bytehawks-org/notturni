"use client";
import type { ReactNode } from "react";

type Tone = "ok" | "warn" | "danger";
const DOT_CLASSES: Record<Tone, string> = { ok: "bg-ok", warn: "bg-[#d9a441]", danger: "bg-danger" };

export function Toast({ tone = "ok", children, onClose }: { tone?: Tone; children: ReactNode; onClose?: () => void }) {
  return (
    <div role="status" className="flex items-center gap-3 rounded-[10px] border border-border bg-background px-3.5 py-3 text-sm shadow-soft">
      <span className={`h-2 w-2 rounded-full ${DOT_CLASSES[tone]}`} />
      <span className="flex-1">{children}</span>
      {onClose && (
        <button type="button" onClick={onClose} aria-label="Chiudi" className="text-muted hover:text-foreground">
          ✕
        </button>
      )}
    </div>
  );
}

/** Stack fisso in basso a sinistra, auto-dismiss a 5s gestito dal chiamante. */
export function ToastStack({ children }: { children: ReactNode }) {
  return <div className="fixed bottom-6 left-6 z-50 flex w-[360px] max-w-[calc(100vw-3rem)] flex-col gap-2">{children}</div>;
}
