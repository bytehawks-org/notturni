"use client";

import { useLocale, useTranslations } from "next-intl";
import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { useCallback, useEffect, useState } from "react";

import { SearchInput } from "@/components/SearchInput";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { FilterChip, Pill } from "@/components/ui/Pill";
import { EmptyState, SkeletonRows } from "@/components/ui/States";
import { useToast } from "@/components/ui/Toast";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDate } from "@/lib/format";
import type { AdminBlog, BlogAdminAction, BlogReports } from "@/lib/types";

const STATES = ["all", "reported", "active", "suspended", "paused", "deleted"] as const;
const ACTIONS: BlogAdminAction[] = ["suspend", "hide_reported_posts", "deactivate_owner", "dismiss"];

/** Tutti i blog + pannello segnalazioni (mockup 5e): tabella con contatori
 * e filtri, pannello laterale con segnalazioni aperte, azione a scelta e
 * nota obbligatoria per il registro. */
export default function DashboardAllBlogsPage() {
  const { authFetch } = useAuth();
  const t = useTranslations("AdminBlogs");
  const tc = useTranslations("Common");
  const tv = useTranslations("Visibility");
  const locale = useLocale();
  const notify = useToast();
  const searchParams = useSearchParams();
  const [q, setQ] = useState("");
  const [state, setState] = useState<(typeof STATES)[number]>((searchParams.get("state") as (typeof STATES)[number]) || "all");
  const [blogs, setBlogs] = useState<AdminBlog[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [panel, setPanel] = useState<BlogReports | null>(null);
  const [panelBusy, setPanelBusy] = useState(false);
  const [action, setAction] = useState<BlogAdminAction>("suspend");
  const [note, setNote] = useState("");

  const errorMessage = useCallback((err: unknown) => (err instanceof ApiClientError ? err.message : tc("unexpectedError")), [tc]);

  const load = useCallback(() => {
    authFetch((token) => api.admin.listBlogsFiltered(token, { q: q || undefined, state: state === "all" ? undefined : state }))
      .then(setBlogs)
      .catch((err) => setError(errorMessage(err)));
  }, [authFetch, q, state, errorMessage]);

  useEffect(load, [load]);

  async function openPanel(blog: AdminBlog) {
    setPanelBusy(true);
    try {
      setPanel(await authFetch((token) => api.admin.blogReports(token, blog.id)));
      setAction(blog.is_suspended ? "restore" : "suspend");
      setNote("");
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setPanelBusy(false);
    }
  }

  async function runAction() {
    if (!panel) return;
    setPanelBusy(true);
    try {
      const updated = await authFetch((token) => api.admin.blogAction(token, panel.blog.id, action, note));
      setBlogs((prev) => prev?.map((b) => (b.id === updated.id ? updated : b)) ?? null);
      notify(t("actionDone"));
      setPanel(null);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setPanelBusy(false);
    }
  }

  const stateOf = (b: AdminBlog) => (b.deleted_at ? "deleted" : b.is_suspended ? "suspended" : b.is_paused ? "paused" : "active");
  const stateTone = { active: "ok", suspended: "danger", paused: "warn", deleted: "neutral" } as const;
  const availableActions: BlogAdminAction[] = panel ? (panel.blog.is_suspended ? ["restore", "deactivate_owner", "dismiss"] : ACTIONS) : [];

  return (
    <div className="mx-auto flex max-w-6xl flex-col gap-5">
      <div className="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
        <div className="flex flex-col gap-1">
          <h1 className="font-serif text-[28px] font-medium leading-tight text-foreground">{t("title")}</h1>
          {blogs && <p className="text-sm text-muted">{t("summary", { count: blogs.length, reported: blogs.filter((b) => b.reports_open > 0).length })}</p>}
        </div>
        <SearchInput value={q} onChange={setQ} placeholder={t("search")} />
      </div>
      <div className="flex flex-wrap gap-1.5">
        {STATES.map((s) => (
          <FilterChip key={s} active={state === s} onClick={() => setState(s)}>
            {t(`state.${s}`)}
          </FilterChip>
        ))}
      </div>
      {error && <Alert kind="error">{error}</Alert>}

      <div className={`grid gap-6 ${panel ? "xl:grid-cols-[minmax(0,1fr)_380px]" : ""}`}>
        <div>
          {blogs === null && !error && <SkeletonRows rows={6} />}
          {blogs !== null && blogs.length === 0 && <EmptyState glyph="◫" title={t("emptyTitle")} body={t("emptyBody")} />}
          {blogs && blogs.length > 0 && (
            <div className="overflow-x-auto rounded-xl border border-border bg-surface">
              <table className="w-full text-left text-sm">
                <thead className="border-b border-border font-mono text-[11px] uppercase tracking-[.06em] text-muted">
                  <tr>
                    <th className="px-4 py-3">{t("col.blog")}</th>
                    <th className="px-4 py-3">{t("col.owner")}</th>
                    <th className="px-4 py-3">{t("col.visibility")}</th>
                    <th className="px-4 py-3">{t("col.posts")}</th>
                    <th className="px-4 py-3">{t("col.reports")}</th>
                    <th className="px-4 py-3">{t("col.state")}</th>
                  </tr>
                </thead>
                <tbody>
                  {blogs.map((b) => (
                    <tr key={b.id} className={`cursor-pointer border-b border-border last:border-0 hover:bg-background ${panel?.blog.id === b.id ? "bg-background" : ""}`} onClick={() => openPanel(b)}>
                      <td className="px-4 py-3">
                        <div className="flex flex-col">
                          <span className="font-medium text-foreground">{b.title}</span>
                          <span className="font-mono text-xs text-muted">{b.slug}</span>
                        </div>
                      </td>
                      <td className="px-4 py-3 text-muted">@{b.owner_username}</td>
                      <td className="px-4 py-3 text-muted">{tv(b.visibility)}</td>
                      <td className="px-4 py-3 text-muted">{b.posts_count}</td>
                      <td className="px-4 py-3">{b.reports_open > 0 ? <Pill tone="warn">{b.reports_open}</Pill> : <span className="text-muted">0</span>}</td>
                      <td className="px-4 py-3">
                        <Pill tone={stateTone[stateOf(b)]}>{t(`state.${stateOf(b)}`)}</Pill>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>

        {panel && (
          <aside className="flex h-fit flex-col gap-4 rounded-xl border border-border bg-surface p-5 xl:sticky xl:top-6">
            <div className="flex items-start justify-between gap-3">
              <div className="flex flex-col gap-0.5">
                <span className="font-serif text-lg text-foreground">{panel.blog.title}</span>
                <span className="text-[13px] text-muted">
                  @{panel.blog.owner_username} · {t("created", { date: formatDate(panel.blog.created_at, locale, { day: "numeric", month: "short", year: "numeric" }) })} · {panel.blog.posts_count} post ·{" "}
                  {panel.owner_mfa_enabled ? t("mfaOn") : t("mfaOff")} · {panel.owner_email_domain}
                </span>
              </div>
              <button type="button" onClick={() => setPanel(null)} className="text-muted hover:text-foreground" aria-label={tc("close")}>
                ✕
              </button>
            </div>

            <div className="flex flex-col gap-2">
              <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">
                {t("reasons")} · {panel.reports.length}
              </span>
              {panel.reports.length === 0 ? (
                <p className="text-[13px] text-muted">{t("noReports")}</p>
              ) : (
                <ul className="flex flex-col gap-2 text-[13px]">
                  {panel.reports.map((r) => (
                    <li key={r.id} className="rounded-lg border border-border px-3 py-2">
                      <div className="flex flex-wrap items-center gap-2">
                        <Pill tone="warn">{t(`reason.${r.reason}`)}</Pill>
                        <span className="text-muted">@{r.reporter_username} · {formatDate(r.created_at, locale, { day: "numeric", month: "short" })}</span>
                      </div>
                      {r.post_slug && (
                        <Link href={`/${panel.blog.slug}/${r.post_slug}`} className="mt-1 block truncate text-foreground no-underline hover:underline">
                          {r.post_title}
                        </Link>
                      )}
                      {r.note && <p className="mt-1 text-muted">“{r.note}”</p>}
                    </li>
                  ))}
                </ul>
              )}
            </div>

            <div className="flex flex-col gap-2">
              <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("action")}</span>
              {availableActions.map((a) => (
                <label key={a} className={`flex cursor-pointer items-start gap-2 rounded-lg border px-3 py-2 text-[13px] ${action === a ? "border-primary bg-primary/5" : "border-border"}`}>
                  <input type="radio" name="blog-action" className="mt-0.5" checked={action === a} onChange={() => setAction(a)} />
                  <span className="flex flex-col">
                    <span className="font-medium text-foreground">{t(`act.${a}`)}</span>
                    <span className="text-muted">{t(`act.${a}Sub`)}</span>
                  </span>
                </label>
              ))}
            </div>

            <label className="flex flex-col gap-1.5">
              <span className="text-xs font-semibold uppercase tracking-[.04em] text-muted">
                {t("auditNote")} <span className="text-danger">· {t("required")}</span>
              </span>
              <textarea
                rows={3}
                value={note}
                onChange={(e) => setNote(e.target.value)}
                className="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground focus:outline-none focus:border-primary"
              />
            </label>
            <div className="flex items-center justify-between gap-2">
              <Button size="sm" variant={action === "dismiss" || action === "restore" ? "primary" : "danger"} disabled={panelBusy || note.trim().length < 3} onClick={runAction}>
                {t(`act.${action}`)}
              </Button>
              <Link href={`/${panel.blog.slug}`} className="text-[13px] text-muted no-underline hover:text-foreground">
                {t("openBlog")}
              </Link>
            </div>
          </aside>
        )}
      </div>
    </div>
  );
}
