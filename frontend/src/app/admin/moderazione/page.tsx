"use client";

import { useLocale, useTranslations } from "next-intl";
import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

import { SearchInput } from "@/components/SearchInput";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { NoteDialog } from "@/components/ui/NoteDialog";
import { Pill } from "@/components/ui/Pill";
import { EmptyState, SkeletonRows } from "@/components/ui/States";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDate } from "@/lib/format";
import { type AdminPost } from "@/lib/types";

/** Moderazione post (mockup 5d/5e): tabella con segnalazioni aperte e
 * nascondi/mostra con nota obbligatoria per il registro. */
export default function DashboardModerationPage() {
  const { authFetch } = useAuth();
  const t = useTranslations("AdminPosts");
  const tc = useTranslations("Common");
  const ts = useTranslations("Status");
  const locale = useLocale();
  const [q, setQ] = useState("");
  const [posts, setPosts] = useState<AdminPost[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<AdminPost | null>(null);

  const errorMessage = useCallback((err: unknown) => (err instanceof ApiClientError ? err.message : tc("unexpectedError")), [tc]);

  const load = useCallback(() => {
    authFetch((token) => api.admin.listPosts(token, q))
      .then(setPosts)
      .catch((err) => setError(errorMessage(err)));
  }, [authFetch, q, errorMessage]);

  useEffect(load, [load]);

  async function toggleHidden(post: AdminPost, note: string) {
    setPending(null);
    try {
      const updated = await authFetch((token) => api.admin.updatePost(token, post.id, { is_hidden: !post.is_hidden, note }));
      setPosts((prev) => prev?.map((p) => (p.id === post.id ? updated : p)) ?? null);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  const statusKey = (p: AdminPost) => (p.status === "pending_review" ? "review" : p.status);

  return (
    <div className="mx-auto flex max-w-6xl flex-col gap-5">
      <div className="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
        <div className="flex flex-col gap-1">
          <h1 className="font-serif text-[28px] font-medium leading-tight text-foreground">{t("title")}</h1>
          <p className="text-sm text-muted">{t("subtitle")}</p>
        </div>
        <SearchInput value={q} onChange={setQ} placeholder={t("search")} />
      </div>
      {error && <Alert kind="error">{error}</Alert>}
      {posts === null && !error && <SkeletonRows rows={6} />}
      {posts !== null && posts.length === 0 && <EmptyState glyph="✎" title={t("emptyTitle")} body={t("emptyBody")} />}
      {posts && posts.length > 0 && (
        <div className="overflow-x-auto rounded-xl border border-border bg-surface">
          <table className="w-full text-left text-sm">
            <thead className="border-b border-border font-mono text-[11px] uppercase tracking-[.06em] text-muted">
              <tr>
                <th className="px-4 py-3">{t("col.post")}</th>
                <th className="px-4 py-3">{t("col.blog")}</th>
                <th className="px-4 py-3">{t("col.status")}</th>
                <th className="px-4 py-3">{t("col.reports")}</th>
                <th className="px-4 py-3">{t("col.moderation")}</th>
              </tr>
            </thead>
            <tbody>
              {posts.map((p) => (
                <tr key={p.id} className="border-b border-border last:border-0">
                  <td className="px-4 py-3">
                    <div className="flex flex-col">
                      <Link href={`/${p.blog_slug}/${p.slug}`} className="font-medium text-foreground no-underline hover:underline">
                        {p.title}
                      </Link>
                      <span className="text-xs text-muted">
                        @{p.author_username} · {formatDate(p.published_at ?? p.created_at, locale, { day: "numeric", month: "short", year: "numeric" })}
                      </span>
                    </div>
                  </td>
                  <td className="px-4 py-3 font-mono text-xs text-muted">{p.blog_slug}</td>
                  <td className="px-4 py-3">
                    <span className="flex flex-wrap gap-1">
                      <Pill tone={p.status === "published" ? "ok" : p.status === "pending_review" ? "warn" : "neutral"}>{ts(statusKey(p))}</Pill>
                      {p.is_hidden && <Pill tone="danger">{ts("hidden")}</Pill>}
                    </span>
                  </td>
                  <td className="px-4 py-3">{p.reports_open > 0 ? <Pill tone="warn">{p.reports_open}</Pill> : <span className="text-muted">0</span>}</td>
                  <td className="px-4 py-3">
                    <Button size="sm" variant={p.is_hidden ? "secondary" : "danger"} onClick={() => setPending(p)}>
                      {p.is_hidden ? t("show") : t("hide")}
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <NoteDialog
        open={pending !== null}
        title={pending ? (pending.is_hidden ? t("showTitle", { title: pending.title }) : t("hideTitle", { title: pending.title })) : ""}
        confirmLabel={pending?.is_hidden ? t("show") : t("hide")}
        danger={!pending?.is_hidden}
        onCancel={() => setPending(null)}
        onConfirm={(note) => pending && toggleHidden(pending, note)}
      />
    </div>
  );
}
