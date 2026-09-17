"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { useState, type FormEvent } from "react";

import { Button } from "@/components/ui/Button";
import { useToast } from "@/components/ui/Toast";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { ReportReason } from "@/lib/types";

const REASONS: ReportReason[] = ["spam", "abuse", "illegal", "other"];

/** "Segnala" su blog e post (B5, mockup 5e): motivo + nota facoltativa,
 * solo da autenticati; da anonimi rimanda al login. */
export function ReportButton({ target }: { target: { type: "blog"; slug: string } | { type: "post"; id: string } }) {
  const { user, loading, authFetch } = useAuth();
  const t = useTranslations("Report");
  const notify = useToast();
  const [open, setOpen] = useState(false);
  const [reason, setReason] = useState<ReportReason>("spam");
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);

  const cls = "text-[13px] text-muted hover:text-foreground";
  if (loading) return null;
  if (!user) {
    return (
      <Link href="/login" className={`${cls} no-underline`}>
        {t("report")}
      </Link>
    );
  }

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    try {
      await authFetch((token) =>
        target.type === "blog" ? api.reports.reportBlog(token, target.slug, reason, note) : api.reports.reportPost(token, target.id, reason, note)
      );
      notify(t("sent"));
      setOpen(false);
      setNote("");
    } catch (err) {
      notify(err instanceof ApiClientError ? err.message : t("failed"), "danger");
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <button type="button" onClick={() => setOpen(true)} className={cls}>
        {t("report")}
      </button>
      {open && (
        <div className="fixed inset-0 z-50 grid place-items-center bg-[rgb(10_10_12/0.55)] p-4" role="dialog" aria-modal="true">
          <form onSubmit={submit} className="flex w-full max-w-md flex-col gap-3 rounded-xl border border-border bg-surface p-6 text-sm shadow-soft">
            <h2 className="font-serif text-xl">{target.type === "blog" ? t("titleBlog") : t("titlePost")}</h2>
            <p className="text-muted">{t("intro")}</p>
            <div className="flex flex-col gap-1.5">
              {REASONS.map((r) => (
                <label key={r} className={`flex cursor-pointer items-center gap-2 rounded-lg border px-3 py-2 ${reason === r ? "border-primary bg-primary/5" : "border-border"}`}>
                  <input type="radio" name="reason" checked={reason === r} onChange={() => setReason(r)} />
                  <span>{t(`reason.${r}`)}</span>
                </label>
              ))}
            </div>
            <textarea
              rows={2}
              maxLength={500}
              value={note}
              onChange={(e) => setNote(e.target.value)}
              placeholder={t("notePlaceholder")}
              className="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground focus:outline-none focus:border-primary"
            />
            <div className="mt-1 flex justify-end gap-2">
              <Button type="button" variant="secondary" size="sm" onClick={() => setOpen(false)}>
                {t("cancel")}
              </Button>
              <Button type="submit" variant="danger" size="sm" disabled={busy}>
                {t("send")}
              </Button>
            </div>
          </form>
        </div>
      )}
    </>
  );
}
