"use client";
import { useTranslations } from "next-intl";
import { useState } from "react";
import { Card } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { TextArea, Label, FieldGroup } from "@/components/ui/Field";
import type { Visibility } from "@/components/blog/VisibilityBand";

export interface AdminBlog { slug: string; title: string; owner: string; visibility: Visibility | "suspended"; posts: number; reports: number; createdAt: string }
export interface Report { id: string; reason: string; by: string[]; note?: string; automatic?: boolean }
type Action = "suspend" | "hide_posts" | "deactivate_owner" | "dismiss";

const band: Record<AdminBlog["visibility"], string> = { public: "var(--vis-public)", members: "var(--vis-members)", private: "var(--vis-private)", suspended: "var(--muted)" };
const visCls: Record<AdminBlog["visibility"], string> = { public: "text-ok", members: "text-[#b8862b]", private: "text-[#8b2e2e]", suspended: "text-muted" };

export function BlogsTable({ blogs, selected, onSelect }: { blogs: AdminBlog[]; selected?: string; onSelect: (b: AdminBlog) => void }) {
  const t = useTranslations("AdminBlogs");
  return (
    <Card className="overflow-hidden p-0">
      <div className="hidden grid-cols-[minmax(0,1.3fr)_130px_90px_90px_90px_80px] gap-3 border-b border-border px-[18px] py-2 font-mono text-[11px] uppercase tracking-[.06em] text-muted md:grid"><span>{t("col.blog")}</span><span>{t("col.owner")}</span><span>{t("col.visibility")}</span><span>{t("col.posts")}</span><span>{t("col.reports")}</span><span /></div>
      {blogs.map((b) => (
        <button key={b.slug} onClick={() => onSelect(b)} className={`grid w-full grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-b border-border px-[18px] py-[11px] text-left text-sm last:border-0 md:grid-cols-[minmax(0,1.3fr)_130px_90px_90px_90px_80px] ${selected === b.slug ? "bg-danger/8" : b.reports > 0 ? "bg-warn/5" : ""}`}>
          <div className="flex min-w-0 items-center gap-2.5"><span className="h-7 w-1 flex-none rounded-sm" style={{ background: band[b.visibility] }} /><div className="flex min-w-0 flex-col leading-snug"><span className="truncate font-medium">{b.title}</span><span className="font-mono text-xs text-muted">{b.slug}</span></div></div>
          <span className="hidden text-[13px] text-muted md:block">@{b.owner}</span>
          <span className={`hidden text-xs font-medium capitalize md:block ${visCls[b.visibility]}`}>{t(`vis.${b.visibility}`)}</span>
          <span className="hidden text-[13px] text-muted md:block">{b.posts}</span>
          <span className={`text-xs font-semibold ${b.reports > 2 ? "text-danger" : b.reports > 0 ? "text-warn" : "text-muted"}`}>{b.reports || "—"}</span>
          <span className={`hidden text-right text-[13px] font-medium md:block ${b.reports > 0 ? "text-danger" : b.visibility === "suspended" ? "text-muted" : "text-primary"}`}>{b.visibility === "suspended" ? t("restore") : b.reports > 0 ? t("review") : t("manage")}</span>
        </button>
      ))}
    </Card>
  );
}

/** Side panel: report reasons + action radio + mandatory audit note. */
export function ReportPanel({ blog, reports, onClose, onApply }: { blog: AdminBlog; reports: Report[]; onClose: () => void; onApply: (a: Action, note: string) => void }) {
  const t = useTranslations("AdminBlogs");
  const [action, setAction] = useState<Action>("suspend");
  const [note, setNote] = useState("");
  const opts: [Action, string, string][] = [["suspend", t("act.suspend"), t("act.suspendSub")], ["hide_posts", t("act.hide_posts"), ""], ["deactivate_owner", t("act.deactivate_owner"), ""], ["dismiss", t("act.dismiss"), ""]];
  return (
    <Card className="flex flex-col gap-3.5 p-[18px] text-sm shadow-soft">
      <div className="flex items-baseline justify-between"><span className="font-mono text-[11px] uppercase tracking-[.08em] text-danger">{t("reportsTitle", { count: reports.length, slug: blog.slug })}</span><button onClick={onClose} className="text-muted" aria-label={t("close")}>✕</button></div>
      <div className="flex flex-col gap-0.5"><span className="font-serif text-lg">{blog.title}</span><span className="text-[13px] text-muted">{t("blogMeta", { owner: blog.owner, date: blog.createdAt, count: blog.posts })}</span></div>
      <div className="flex flex-col gap-2 text-[13px]"><span className="text-xs font-semibold uppercase tracking-[.04em] text-muted">{t("reasons")}</span>{reports.map((r) => <span key={r.id} className="rounded-lg bg-muted/10 px-2.5 py-2"><b className="font-semibold">{r.reason}</b> · {r.automatic ? t("automatic") : r.by.map((b) => "@" + b).join(", ")}{r.note && <> · “{r.note}”</>}</span>)}</div>
      <div role="radiogroup" className="flex flex-col gap-1.5">
        <span className="text-xs font-semibold uppercase tracking-[.04em] text-muted">{t("action")}</span>
        {opts.map(([v, label, sub]) => <button key={v} role="radio" aria-checked={action === v} onClick={() => setAction(v)} className={`flex gap-2.5 rounded-lg border px-3 py-2 text-left ${action === v ? "border-primary" : "border-border text-muted"}`}><span className={`h-4 w-4 rounded-full bg-surface ${action === v ? "border-[5px] border-primary" : "border border-border"}`} /><span>{label}{sub && <span className="text-muted"> · {sub}</span>}</span></button>)}
      </div>
      <FieldGroup><Label>{t("auditNote")}</Label><TextArea className="min-h-16 text-[13px]" placeholder={t("required")} value={note} onChange={(e) => setNote(e.target.value)} /></FieldGroup>
      <div className="mt-auto flex gap-2"><Button variant={action === "dismiss" ? "primary" : "danger"} size="sm" disabled={!note.trim()} onClick={() => onApply(action, note)}>{opts.find((o) => o[0] === action)![1].split(" ")[0]}</Button><a href={`/${blog.slug}`} target="_blank" rel="noreferrer"><Button variant="secondary" size="sm">{t("openBlog")}</Button></a></div>
    </Card>
  );
}
