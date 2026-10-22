"use client";

import { useTranslations } from "next-intl";
import { useState, type FormEvent } from "react";

import { Button } from "./Button";

/** Nota obbligatoria per le azioni admin (mockup 5e "Note for the audit
 * register"): il backend rifiuta le azioni senza nota (B5). */
export function NoteDialog({
  open,
  title,
  body,
  confirmLabel,
  danger = false,
  onCancel,
  onConfirm,
}: {
  open: boolean;
  title: string;
  body?: string;
  confirmLabel: string;
  danger?: boolean;
  onCancel: () => void;
  onConfirm: (note: string) => void;
}) {
  const t = useTranslations("NoteDialog");
  const tc = useTranslations("Common");
  const [note, setNote] = useState("");
  if (!open) return null;
  const ok = note.trim().length >= 3;
  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-[rgb(10_10_12/0.55)] p-4" role="dialog" aria-modal="true">
      <form
        onSubmit={(e: FormEvent) => {
          e.preventDefault();
          if (ok) {
            onConfirm(note.trim());
            setNote("");
          }
        }}
        className="flex w-full max-w-md flex-col gap-3 rounded-xl border border-border bg-surface p-6 text-sm shadow-soft"
      >
        <h2 className="font-serif text-xl">{title}</h2>
        {body && <p className="leading-relaxed text-muted">{body}</p>}
        <label className="flex flex-col gap-1.5">
          <span className="text-xs font-semibold uppercase tracking-[.04em] text-muted">
            {t("label")} <span className="text-danger">· {t("required")}</span>
          </span>
          <textarea
            autoFocus
            rows={3}
            value={note}
            onChange={(e) => setNote(e.target.value)}
            placeholder={t("placeholder")}
            className="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground focus:outline-none focus:border-primary focus:ring-[3px] focus:ring-primary/20"
          />
        </label>
        <div className="mt-1.5 flex justify-end gap-2">
          <Button
            type="button"
            variant="secondary"
            size="sm"
            onClick={() => {
              // Il componente resta montato tra un'apertura e l'altra (`open`
              // controlla solo il render, non lo smontaggio): senza questo la
              // nota della volta precedente restava nello stato e poteva
              // essere inviata per un'azione diversa alla riapertura.
              setNote("");
              onCancel();
            }}
          >
            {tc("cancel")}
          </Button>
          <Button type="submit" variant={danger ? "danger" : "primary"} size="sm" disabled={!ok}>
            {confirmLabel}
          </Button>
        </div>
      </form>
    </div>
  );
}
