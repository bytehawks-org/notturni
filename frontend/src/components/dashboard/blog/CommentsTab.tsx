"use client";

import { useLocale, useTranslations } from "next-intl";
import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { EmptyState, SkeletonRows } from "@/components/ui/States";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDate } from "@/lib/format";
import { type BlogComment } from "@/lib/types";

import { errorMessage } from "./shared";

/** Coda dei commenti in attesa (mockup 5b, versione con le sole azioni oggi
 * disponibili: approva/rifiuta). Stati "nascosti"/"segnalati", lista dei
 * bloccati, escalation e policy estesa arrivano con il blocco B4. */
export function CommentsTab({ blogSlug, canModerate }: { blogSlug: string; canModerate: boolean }) {
  const { accessToken, authFetch } = useAuth();
  const t = useTranslations("CommentsTab");
  const locale = useLocale();
  const [pending, setPending] = useState<BlogComment[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!canModerate || !accessToken) {
      setPending([]);
      return;
    }
    try {
      // Un'unica richiesta aggregata per tutto il blog, non una per post.
      setPending(await authFetch((token) => api.comments.listForBlog(token, blogSlug)));
    } catch (err) {
      setError(errorMessage(err));
    }
  }, [blogSlug, canModerate, accessToken, authFetch]);

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- fetch dati all'apertura della tab
    load();
  }, [load]);

  async function handleModerate(commentId: string, action: "approve" | "reject") {
    try {
      await authFetch((token) =>
        action === "approve" ? api.comments.approve(token, commentId) : api.comments.reject(token, commentId)
      );
      setPending((prev) => prev?.filter((c) => c.id !== commentId) ?? null);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  if (!canModerate) return <p className="text-sm text-muted">{t("ownerOnly")}</p>;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-baseline gap-3 text-sm">
        <span className="font-semibold text-foreground">
          {t("pending")}
          {pending && <span className="text-muted"> · {pending.length}</span>}
        </span>
      </div>
      {error && <Alert kind="error">{error}</Alert>}
      {pending === null && !error && <SkeletonRows rows={3} />}
      {pending !== null && pending.length === 0 && <EmptyState glyph="✓" title={t("emptyTitle")} body={t("emptyBody")} />}
      {pending && pending.length > 0 && (
        <ul className="flex flex-col rounded-xl border border-border bg-surface">
          {pending.map((comment) => (
            <li key={comment.id} className="flex flex-col gap-2.5 border-b border-border px-4 py-4 last:border-0">
              <div className="flex flex-wrap items-center gap-2 text-[13px] text-muted">
                <span className="font-medium text-foreground">{comment.author_display_name}</span>
                <span>· {formatDate(comment.created_at, locale, { day: "numeric", month: "short" })}</span>
                <span>
                  · {t("on")}{" "}
                  <Link href={`/${blogSlug}/${comment.post_slug}`} className="text-foreground no-underline hover:underline">
                    {comment.post_title}
                  </Link>
                </span>
              </div>
              <p className="text-[15px] leading-relaxed text-foreground">{comment.content}</p>
              <div className="flex gap-2">
                <Button size="sm" onClick={() => handleModerate(comment.id, "approve")}>
                  {t("approve")}
                </Button>
                <Button size="sm" variant="secondary" onClick={() => handleModerate(comment.id, "reject")}>
                  {t("reject")}
                </Button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
