"use client";

import { useLocale, useTranslations } from "next-intl";
import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { FilterChip } from "@/components/ui/Pill";
import { Pill } from "@/components/ui/Pill";
import { EmptyState, SkeletonRows } from "@/components/ui/States";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDate } from "@/lib/format";
import { type AdminComment, type CommentStatus } from "@/lib/types";

type Filter = CommentStatus | "reported";
const FILTERS: Filter[] = ["reported", "pending", "approved", "rejected"];

/** Moderazione commenti di piattaforma (mockup 5d "escalations"): i commenti
 * segnalati dai blog (con nota) più le code per stato di tutti i blog. */
export default function DashboardCommentModerationPage() {
  const { authFetch } = useAuth();
  const t = useTranslations("AdminComments");
  const tc = useTranslations("Common");
  const locale = useLocale();
  const [filter, setFilter] = useState<Filter>("reported");
  const [comments, setComments] = useState<AdminComment[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    authFetch((token) => api.admin.listComments(token, filter === "reported" ? { reported: true } : { status: filter }))
      .then(setComments)
      .catch((err) => setError(err instanceof ApiClientError ? err.message : tc("unexpectedError")));
  }, [authFetch, filter, tc]);

  useEffect(load, [load]);

  async function moderate(commentId: string, action: "approve" | "reject") {
    try {
      await authFetch((token) => (action === "approve" ? api.comments.approve(token, commentId) : api.comments.reject(token, commentId)));
      load();
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : tc("unexpectedError"));
    }
  }

  return (
    <div className="mx-auto flex max-w-6xl flex-col gap-5">
      <div className="flex flex-col gap-1">
        <h1 className="font-serif text-[28px] font-medium leading-tight text-foreground">{t("title")}</h1>
        <p className="text-sm text-muted">{t("subtitle")}</p>
      </div>
      <div className="flex flex-wrap gap-1.5">
        {FILTERS.map((f) => (
          <FilterChip key={f} active={filter === f} onClick={() => setFilter(f)}>
            {t(`filter.${f}`)}
          </FilterChip>
        ))}
      </div>
      {error && <Alert kind="error">{error}</Alert>}
      {comments === null && !error && <SkeletonRows rows={4} />}
      {comments !== null && comments.length === 0 && <EmptyState glyph="✓" title={t("emptyTitle")} body={t("emptyBody")} />}
      {comments && comments.length > 0 && (
        <ul className="flex flex-col rounded-xl border border-border bg-surface">
          {comments.map((c) => (
            <li key={c.id} className="flex flex-col gap-2 border-b border-border px-4 py-4 last:border-0">
              <div className="flex flex-wrap items-center gap-2 text-[13px] text-muted">
                <Link href={`/${c.blog_slug}`} className="font-mono text-foreground no-underline hover:underline">
                  {c.blog_slug}
                </Link>
                <span>
                  · {t("on")}{" "}
                  <Link href={`/${c.blog_slug}/${c.post_slug}`} className="text-foreground no-underline hover:underline">
                    {c.post_title}
                  </Link>
                </span>
                <span>· {c.author_display_name}</span>
                <span>· {formatDate(c.created_at, locale, { day: "numeric", month: "short" })}</span>
                <Pill tone={c.status === "approved" ? "ok" : c.status === "rejected" ? "danger" : "neutral"}>{t(`status.${c.status}`)}</Pill>
                {c.reported_to_platform && <Pill tone="warn">{t("reported")}</Pill>}
              </div>
              <p className="text-[15px] leading-relaxed text-foreground">{c.content}</p>
              {c.report_note && (
                <p className="rounded-lg bg-[var(--warn-bg)] px-3 py-2 text-[13px] text-foreground">
                  <span className="font-semibold">{t("reportNote")}:</span> {c.report_note}
                  {c.reported_at && <span className="text-muted"> · {formatDate(c.reported_at, locale, { day: "numeric", month: "short" })}</span>}
                </p>
              )}
              <div className="flex gap-2">
                {c.status !== "approved" && (
                  <Button size="sm" onClick={() => moderate(c.id, "approve")}>
                    {t("approve")}
                  </Button>
                )}
                {c.status !== "rejected" && (
                  <Button size="sm" variant="secondary" onClick={() => moderate(c.id, "reject")}>
                    {t("hide")}
                  </Button>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
