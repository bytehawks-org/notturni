"use client";
import { useTranslations } from "next-intl";
import { useState } from "react";
import { Button } from "./Button";
import { Input } from "./Field";

/** Destructive confirmation: the user must type `confirmText` (e.g. the blog slug). */
export function ConfirmDialog({ open, title, body, confirmText, confirmLabel, onCancel, onConfirm }: {
  open: boolean; title: string; body: string; confirmText: string; confirmLabel: string; onCancel: () => void; onConfirm: () => void;
}) {
  const t = useTranslations("Common");
  const [typed, setTyped] = useState("");
  if (!open) return null;
  const ok = typed.trim() === confirmText;
  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-[rgb(10_10_12/0.55)] p-4" role="dialog" aria-modal="true" aria-labelledby="confirm-title">
      <div className="flex w-full max-w-md flex-col gap-3 rounded-xl border border-border bg-surface p-6 text-sm shadow-soft">
        <h2 id="confirm-title" className="font-serif text-xl">{title}</h2>
        <p className="leading-relaxed text-muted">{body}</p>
        <Input value={typed} onChange={(e) => setTyped(e.target.value)} placeholder={confirmText} className="font-mono text-[13px]" autoFocus />
        <div className="mt-1.5 flex justify-end gap-2">
          <Button variant="secondary" size="sm" onClick={onCancel}>{t("keepIt")}</Button>
          <Button variant="danger" size="sm" disabled={!ok} onClick={onConfirm}>{confirmLabel}</Button>
        </div>
      </div>
    </div>
  );
}
