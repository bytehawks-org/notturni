"use client";

import { useTranslations } from "next-intl";
import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from "react";

type Tone = "ok" | "warn" | "danger";
const DOT_CLASSES: Record<Tone, string> = { ok: "bg-ok", warn: "bg-[#d9a441]", danger: "bg-danger" };

export function Toast({ tone = "ok", children, onClose }: { tone?: Tone; children: ReactNode; onClose?: () => void }) {
  const t = useTranslations("Common");
  return (
    <div role="status" className="flex items-center gap-3 rounded-[10px] border border-border bg-background px-3.5 py-3 text-sm shadow-soft">
      <span className={`h-2 w-2 rounded-full ${DOT_CLASSES[tone]}`} />
      <span className="flex-1">{children}</span>
      {onClose && (
        <button type="button" onClick={onClose} aria-label={t("dismiss")} className="text-muted hover:text-foreground">
          ✕
        </button>
      )}
    </div>
  );
}

/** Stack fisso in basso a sinistra (mockup 3d). */
export function ToastStack({ children }: { children: ReactNode }) {
  return <div className="fixed bottom-6 left-6 z-50 flex w-[360px] max-w-[calc(100vw-3rem)] flex-col gap-2">{children}</div>;
}

interface ToastItem {
  id: number;
  tone: Tone;
  message: ReactNode;
}

const ToastContext = createContext<((message: ReactNode, tone?: Tone) => void) | null>(null);

/** Provider con auto-dismiss a 5 s; `useToast()` restituisce `notify(message, tone)`. */
export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<ToastItem[]>([]);
  const dismiss = useCallback((id: number) => setItems((prev) => prev.filter((i) => i.id !== id)), []);
  const notify = useCallback(
    (message: ReactNode, tone: Tone = "ok") => {
      const id = Date.now() + Math.random();
      setItems((prev) => [...prev, { id, tone, message }]);
      setTimeout(() => dismiss(id), 5000);
    },
    [dismiss]
  );
  const value = useMemo(() => notify, [notify]);
  return (
    <ToastContext.Provider value={value}>
      {children}
      {items.length > 0 && (
        <ToastStack>
          {items.map((item) => (
            <Toast key={item.id} tone={item.tone} onClose={() => dismiss(item.id)}>
              {item.message}
            </Toast>
          ))}
        </ToastStack>
      )}
    </ToastContext.Provider>
  );
}

export function useToast(): (message: ReactNode, tone?: Tone) => void {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error("useToast deve essere usato dentro ToastProvider");
  return ctx;
}
