"use client";

import { useLocale, useTranslations } from "next-intl";
import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { Pill } from "@/components/ui/Pill";
import { EmptyState, SkeletonRows } from "@/components/ui/States";
import { useToast } from "@/components/ui/Toast";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDate } from "@/lib/format";
import type { BlockedAuthor, Blog, BlogComment, CommentsMode } from "@/lib/types";

import { errorMessage } from "./shared";

type Queue = "pending" | "approved" | "hidden" | "reported";
const QUEUES: Queue[] = ["pending", "approved", "hidden", "reported"];
const POLICIES: CommentsMode[] = ["members", "everyone", "closed"];

/** Coda commenti del blog (mockup 5b): tab In attesa/Approvati/Nascosti/
 * Segnalati con conteggi, selezione multipla con approva/nascondi in blocco,
 * azioni per riga (approva, rispondi, nascondi, blocca autore, segnala alla
 * piattaforma con nota), regole dei commenti (policy + chiusura automatica)
 * e lista dei bloccati. "Avvisami via email" non c'è: nessun canale email
 * transazionale oltre all'OTP (consumer ancora placeholder). */
export function CommentsTab({ blog, canModerate, onBlogUpdated }: { blog: Blog; canModerate: boolean; onBlogUpdated?: (blog: Blog) => void }) {
  const { accessToken, authFetch } = useAuth();
  const t = useTranslations("CommentsTab");
  const tc = useTranslations("Common");
  const locale = useLocale();
  const notify = useToast();
  const blogSlug = blog.slug;

  const [queue, setQueue] = useState<Queue>("pending");
  const [items, setItems] = useState<BlogComment[] | null>(null);
  const [counts, setCounts] = useState<Partial<Record<Queue, number>>>({});
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [blocked, setBlocked] = useState<BlockedAuthor[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [replyTo, setReplyTo] = useState<string | null>(null);
  const [replyText, setReplyText] = useState("");
  const [reportFor, setReportFor] = useState<string | null>(null);
  const [reportNote, setReportNote] = useState("");
  const [busy, setBusy] = useState(false);

  const fetchQueue = useCallback(
    (q: Queue) =>
      authFetch((token) =>
        q === "reported" ? api.comments.listForBlog(token, blogSlug, "pending", true) : api.comments.listForBlog(token, blogSlug, q === "hidden" ? "rejected" : q)
      ),
    [authFetch, blogSlug]
  );

  const load = useCallback(async () => {
    if (!canModerate || !accessToken) {
      setItems([]);
      return;
    }
    try {
      const [list, ...others] = await Promise.all([fetchQueue(queue), ...QUEUES.filter((q) => q !== queue).map(fetchQueue)]);
      setItems(list);
      const next: Partial<Record<Queue, number>> = { [queue]: list.length };
      QUEUES.filter((q) => q !== queue).forEach((q, i) => {
        next[q] = others[i].length;
      });
      setCounts(next);
      setSelected(new Set());
      setBlocked(await authFetch((token) => api.comments.listBlocked(token, blogSlug)));
    } catch (err) {
      setError(errorMessage(err));
    }
  }, [canModerate, accessToken, fetchQueue, queue, authFetch, blogSlug]);

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- fetch dati all'apertura della tab e al cambio coda
    void load();
  }, [load]);

  async function act(fn: () => Promise<unknown>, done?: string) {
    setBusy(true);
    setError(null);
    try {
      await fn();
      if (done) notify(done);
      await load();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  const moderate = (ids: string[], action: "approve" | "reject") =>
    act(
      () => Promise.all(ids.map((id) => authFetch((token) => (action === "approve" ? api.comments.approve(token, id) : api.comments.reject(token, id))))),
      action === "approve" ? t("approvedToast", { count: ids.length }) : t("hiddenToast", { count: ids.length })
    );

  async function sendReply(comment: BlogComment) {
    await act(
      () => api.comments.create(accessToken, comment.post_id, { content: replyText, parent_id: comment.parent_id ?? comment.id }),
      t("repliedToast")
    );
    setReplyTo(null);
    setReplyText("");
  }

  async function updatePolicy(payload: { comments_mode?: CommentsMode; comments_auto_close_days?: number | null }) {
    await act(async () => {
      const updated = await authFetch((token) => api.blogs.update(token, blogSlug, payload));
      onBlogUpdated?.(updated);
    }, t("policySaved"));
  }

  if (!canModerate) return <p className="text-sm text-muted">{t("ownerOnly")}</p>;

  const allSelected = items !== null && items.length > 0 && selected.size === items.length;

  return (
    <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_320px]">
      <div className="flex flex-col gap-4">
        <div className="flex flex-wrap items-center gap-1 border-b border-border">
          {QUEUES.map((q) => (
            <button
              key={q}
              type="button"
              onClick={() => setQueue(q)}
              className={`border-b-2 px-3.5 py-2 text-sm transition ${queue === q ? "border-primary font-medium text-foreground" : "border-transparent text-muted hover:text-foreground"}`}
            >
              {t(`queue.${q}`)}
              {counts[q] !== undefined && <span className="ml-1 text-muted">· {counts[q]}</span>}
            </button>
          ))}
        </div>
        {error && <Alert kind="error">{error}</Alert>}
        {items && items.length > 0 && (
          <div className="flex flex-wrap items-center gap-3 text-[13px]">
            <label className="flex items-center gap-2 text-muted">
              <input type="checkbox" checked={allSelected} onChange={(e) => setSelected(e.target.checked ? new Set(items.map((c) => c.id)) : new Set())} />
              {t("selectAll")}
            </label>
            {selected.size > 0 && (
              <>
                {queue !== "approved" && (
                  <Button size="sm" disabled={busy} onClick={() => moderate([...selected], "approve")}>
                    {t("approveN", { count: selected.size })}
                  </Button>
                )}
                {queue !== "hidden" && (
                  <Button size="sm" variant="secondary" disabled={busy} onClick={() => moderate([...selected], "reject")}>
                    {t("hideN", { count: selected.size })}
                  </Button>
                )}
              </>
            )}
          </div>
        )}
        {items === null && !error && <SkeletonRows rows={3} />}
        {items !== null && items.length === 0 && <EmptyState glyph="✓" title={t(`empty.${queue}`)} body={t("emptyBody")} />}
        {items && items.length > 0 && (
          <ul className="flex flex-col rounded-xl border border-border bg-surface">
            {items.map((comment) => (
              <li key={comment.id} className="flex gap-3 border-b border-border px-4 py-4 last:border-0">
                <input
                  type="checkbox"
                  className="mt-1"
                  checked={selected.has(comment.id)}
                  onChange={(e) =>
                    setSelected((prev) => {
                      const next = new Set(prev);
                      if (e.target.checked) next.add(comment.id);
                      else next.delete(comment.id);
                      return next;
                    })
                  }
                  aria-label={t("select")}
                />
                <div className="flex min-w-0 flex-1 flex-col gap-2">
                  <div className="flex flex-wrap items-center gap-2 text-[13px] text-muted">
                    <span className="font-medium text-foreground">{comment.author_display_name}</span>
                    <span>· {comment.author_id ? t("registered") : t("anonymous")}</span>
                    <span>· {formatDate(comment.created_at, locale, { day: "numeric", month: "short" })}</span>
                    <span>
                      · {t("on")}{" "}
                      <Link href={`/${blogSlug}/${comment.post_slug}`} className="text-foreground no-underline hover:underline">
                        {comment.post_title}
                      </Link>
                    </span>
                    {comment.reported_to_platform && <Pill tone="warn">{t("reportedPill")}</Pill>}
                    {queue === "reported" && <Pill tone={comment.status === "approved" ? "ok" : comment.status === "rejected" ? "danger" : "neutral"}>{t(`status.${comment.status}`)}</Pill>}
                  </div>
                  <p className="text-[15px] leading-relaxed text-foreground">{comment.content}</p>
                  {comment.report_note && <p className="text-[13px] text-muted">{t("reportNote")}: {comment.report_note}</p>}
                  <div className="flex flex-wrap items-center gap-3 text-[13px]">
                    {comment.status !== "approved" && (
                      <button type="button" disabled={busy} className="font-medium text-primary hover:underline" onClick={() => moderate([comment.id], "approve")}>
                        {t("approve")}
                      </button>
                    )}
                    <button type="button" className="text-muted hover:text-foreground" onClick={() => setReplyTo(replyTo === comment.id ? null : comment.id)}>
                      {t("reply")}
                    </button>
                    {comment.status !== "rejected" && (
                      <button type="button" disabled={busy} className="text-muted hover:text-foreground" onClick={() => moderate([comment.id], "reject")}>
                        {t("hide")}
                      </button>
                    )}
                    <button
                      type="button"
                      disabled={busy}
                      className="text-muted hover:text-danger"
                      onClick={() => act(() => authFetch((token) => api.comments.blockAuthor(token, comment.id)), t("blockedToast"))}
                    >
                      {t("blockAuthor")}
                    </button>
                    {!comment.reported_to_platform && (
                      <button type="button" className="text-muted hover:text-foreground" onClick={() => setReportFor(reportFor === comment.id ? null : comment.id)}>
                        {t("report")}
                      </button>
                    )}
                  </div>
                  {replyTo === comment.id && (
                    <form
                      className="flex flex-col gap-2"
                      onSubmit={(e) => {
                        e.preventDefault();
                        void sendReply(comment);
                      }}
                    >
                      <textarea
                        required
                        rows={2}
                        value={replyText}
                        onChange={(e) => setReplyText(e.target.value)}
                        placeholder={t("replyPlaceholder")}
                        className="w-full rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground focus:outline-none focus:border-primary"
                      />
                      <div className="flex gap-2">
                        <Button size="sm" type="submit" disabled={busy}>
                          {t("send")}
                        </Button>
                        <Button size="sm" variant="ghost" type="button" onClick={() => setReplyTo(null)}>
                          {tc("cancel")}
                        </Button>
                      </div>
                    </form>
                  )}
                  {reportFor === comment.id && (
                    <form
                      className="flex flex-col gap-2 rounded-lg border border-border bg-background p-3"
                      onSubmit={(e) => {
                        e.preventDefault();
                        void act(() => authFetch((token) => api.comments.report(token, comment.id, reportNote)), t("reportedToast")).then(() => {
                          setReportFor(null);
                          setReportNote("");
                        });
                      }}
                    >
                      <span className="text-[13px] text-muted">{t("reportHint")}</span>
                      <input
                        required
                        value={reportNote}
                        onChange={(e) => setReportNote(e.target.value)}
                        placeholder={t("reportNotePlaceholder")}
                        className="rounded-lg border border-border bg-surface px-3 py-2 text-sm text-foreground focus:outline-none focus:border-primary"
                      />
                      <div className="flex gap-2">
                        <Button size="sm" variant="danger" type="submit" disabled={busy || !reportNote.trim()}>
                          {t("report")}
                        </Button>
                        <Button size="sm" variant="ghost" type="button" onClick={() => setReportFor(null)}>
                          {tc("cancel")}
                        </Button>
                      </div>
                    </form>
                  )}
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>

      <aside className="flex flex-col gap-4">
        <Card className="flex flex-col gap-3">
          <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("policyTitle")}</span>
          <div className="flex flex-col gap-2">
            {POLICIES.map((m) => (
              <label key={m} className={`flex cursor-pointer gap-2.5 rounded-lg border px-3 py-2.5 ${blog.comments_mode === m ? "border-primary bg-primary/5" : "border-border"}`}>
                <input type="radio" name="comments-policy" className="mt-1" checked={blog.comments_mode === m} disabled={busy} onChange={() => updatePolicy({ comments_mode: m })} />
                <span className="flex flex-col">
                  <span className="text-sm font-medium text-foreground">{t(`policy.${m}`)}</span>
                  <span className="text-[13px] text-muted">{t(`policy.${m}Sub`)}</span>
                </span>
              </label>
            ))}
          </div>
          <label className="flex items-center justify-between gap-3 text-sm">
            <span>{t("autoClose")}</span>
            <input
              type="checkbox"
              checked={!!blog.comments_auto_close_days}
              disabled={busy}
              onChange={(e) => updatePolicy({ comments_auto_close_days: e.target.checked ? 90 : null })}
            />
          </label>
        </Card>
        <Card className="flex flex-col gap-3">
          <div className="flex items-baseline justify-between">
            <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("blocked")}</span>
            <span className="text-[13px] text-muted">{blocked.length}</span>
          </div>
          {blocked.length === 0 ? (
            <p className="text-[13px] text-muted">{t("noBlocked")}</p>
          ) : (
            <ul className="flex flex-col text-[13px]">
              {blocked.map((b) => (
                <li key={b.id} className="flex items-center justify-between gap-2 border-b border-border py-2 last:border-0">
                  <span className="min-w-0 truncate">
                    <span className="font-medium text-foreground">{b.label}</span>
                    {b.is_anonymous && <span className="text-muted"> · {t("anonymousHashed")}</span>}
                    {b.note && <span className="text-muted"> · {b.note}</span>}
                    <span className="text-muted"> · {formatDate(b.created_at, locale, { day: "numeric", month: "short" })}</span>
                  </span>
                  <button type="button" disabled={busy} className="shrink-0 text-muted hover:text-foreground" onClick={() => act(() => authFetch((token) => api.comments.unblock(token, blogSlug, b.id)), t("unblockedToast"))}>
                    {t("unblock")}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </Card>
      </aside>
    </div>
  );
}
