"use client";

import { useLocale, useTranslations } from "next-intl";
import { useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { api, ApiClientError } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDateTime } from "@/lib/format";
import type { RevisionDiff, RevisionSummary } from "@/lib/types";

const HUNK_STYLE: Record<string, string> = {
  equal: "text-muted",
  insert: "bg-[color-mix(in_srgb,var(--success,#2f7d4f)_14%,transparent)] text-foreground",
  delete: "bg-[color-mix(in_srgb,var(--danger,#b3402f)_14%,transparent)] text-foreground line-through decoration-danger/50",
};
const HUNK_MARK: Record<string, string> = { equal: " ", insert: "+", delete: "−" };

/** Cronologia di un post negli Spazi wiki/docs: elenco delle revisioni, differenze con la precedente
 * e ripristino. `refreshKey` cambia a ogni salvataggio per ricaricare l'elenco. */
export function RevisionsPanel({
  postId,
  refreshKey,
  canRestore,
  onRestore,
}: {
  postId: string;
  refreshKey: number;
  canRestore: boolean;
  /** Ripristina il contenuto della revisione; il chiamante decide come (e se confermare). */
  onRestore: (revision: number) => Promise<void>;
}) {
  const { authFetch } = useAuth();
  const t = useTranslations("RevisionsPanel");
  const locale = useLocale();
  const [list, setList] = useState<RevisionSummary[] | null>(null);
  const [open, setOpen] = useState<number | null>(null);
  const [diff, setDiff] = useState<RevisionDiff | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    authFetch((token) => api.wiki.revisions(token, postId))
      .then(setList)
      .catch((err) => setError(err instanceof ApiClientError ? err.message : t("loadError")));
  }, [postId, refreshKey, authFetch, t]);

  async function show(revision: number) {
    if (open === revision) {
      setOpen(null);
      return;
    }
    setOpen(revision);
    setDiff(null);
    try {
      setDiff(await authFetch((token) => api.wiki.diff(token, postId, revision)));
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : t("loadError"));
    }
  }

  async function restore(revision: number) {
    setBusy(true);
    setError(null);
    try {
      await onRestore(revision);
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : t("loadError"));
    } finally {
      setBusy(false);
    }
  }

  if (!list) return error ? <Alert kind="error">{error}</Alert> : <p className="text-sm text-muted">…</p>;

  return (
    <div className="flex flex-col gap-3">
      {error && <Alert kind="error">{error}</Alert>}
      {list.length === 0 && <p className="text-sm text-muted">{t("empty")}</p>}
      <ol className="flex flex-col gap-2">
        {list.map((r, i) => (
          <li key={r.revision} className="rounded-lg border border-border bg-surface">
            <button
              type="button"
              onClick={() => show(r.revision)}
              className="flex w-full flex-col gap-0.5 px-3 py-2 text-left"
              aria-expanded={open === r.revision}
            >
              <span className="text-[13px] font-medium text-foreground">
                #{r.revision}
                {i === 0 && <span className="ml-2 text-xs font-normal text-muted">{t("current")}</span>}
              </span>
              <span className="text-xs text-muted">
                {formatDateTime(r.created_at, locale)}
                {r.editor ? ` · ${r.editor}` : ""}
              </span>
              {r.summary && <span className="text-xs text-foreground">{r.summary}</span>}
            </button>
            {open === r.revision && (
              <div className="flex flex-col gap-2 border-t border-border px-3 py-2">
                {!diff ? (
                  <p className="text-xs text-muted">…</p>
                ) : (
                  <pre className="max-h-72 overflow-auto whitespace-pre-wrap break-words font-mono text-xs leading-relaxed">
                    {diff.hunks.map((h, k) => (
                      <span key={k} className={`block ${HUNK_STYLE[h.op]}`}>
                        {h.text
                          .split("\n")
                          .map((line) => `${HUNK_MARK[h.op]} ${line}`)
                          .join("\n")}
                      </span>
                    ))}
                  </pre>
                )}
                {canRestore && i > 0 && (
                  <Button size="sm" variant="secondary" disabled={busy} onClick={() => restore(r.revision)}>
                    {t("restore")}
                  </Button>
                )}
              </div>
            )}
          </li>
        ))}
      </ol>
    </div>
  );
}
