"use client";
import { useTranslations } from "next-intl";
import Link from "next/link";
import { useState } from "react";
import { Card, CardTitle } from "@/components/ui/Card";
import { FilterChip } from "@/components/ui/Pill";
import { Toggle } from "@/components/ui/Controls";

export type CommentState = "pending" | "approved" | "hidden" | "reported";
export interface ModComment { id: string; author: string; registered: boolean; postTitle: string; postHref: string; at: string; text: string; state: CommentState }
export type CommentPolicy = "registered" | "open_moderated" | "off";

export function CommentsQueue({ comments, counts, onAction }: { comments: ModComment[]; counts: Record<CommentState, number>; onAction: (id: string, a: "approve" | "hide" | "block" | "report" | "reply") => void }) {
  const t = useTranslations("Comments");
  const [state, setState] = useState<CommentState>("pending");
  const [sel, setSel] = useState<Set<string>>(new Set());
  const list = comments.filter((c) => c.state === state);
  const toggle = (id: string) => setSel((s) => { const n = new Set(s); n.has(id) ? n.delete(id) : n.add(id); return n; });
  return (
    <Card className="flex flex-col overflow-hidden p-0">
      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border px-4 py-3 text-[13px]">
        <div className="flex gap-1.5">{(["pending", "approved", "hidden", "reported"] as CommentState[]).map((s) => <FilterChip key={s} active={s === state} onClick={() => setState(s)}>{t(`state.${s}`)} · {counts[s]}</FilterChip>)}</div>
        {sel.size > 0 && <div className="flex gap-3 text-muted"><button onClick={() => sel.forEach((id) => onAction(id, "approve"))} className="text-ok">{t("approveN", { count: sel.size })}</button><button onClick={() => sel.forEach((id) => onAction(id, "hide"))}>{t("hideN", { count: sel.size })}</button></div>}
      </div>
      {list.length === 0 && <p className="px-4 py-10 text-center text-sm text-muted">{t("empty")}</p>}
      {list.map((c) => (
        <div key={c.id} className={`grid grid-cols-[18px_32px_minmax(0,1fr)] gap-3 border-b border-border px-4 py-3.5 text-sm last:border-0 ${!c.registered ? "bg-muted/6" : ""}`}>
          <input type="checkbox" checked={sel.has(c.id)} onChange={() => toggle(c.id)} className="mt-0.5 h-4 w-4 accent-[var(--primary)]" aria-label={t("select")} />
          <span className="h-8 w-8 rounded-full bg-muted/40" />
          <div className="flex min-w-0 flex-col gap-1.5">
            <div className="flex justify-between gap-3 text-[13px] text-muted"><span><b className="font-semibold text-foreground">{c.author}</b> · {c.registered ? t("registered") : t("unregistered")} · {t("on")} <Link href={c.postHref}>{c.postTitle}</Link></span><span className="whitespace-nowrap">{c.at}</span></div>
            <p className="m-0 leading-relaxed">{c.text}</p>
            <div className="flex flex-wrap gap-4 text-[13px]">
              {c.state !== "approved" && <button onClick={() => onAction(c.id, "approve")} className="font-medium text-ok">{t("approve")}</button>}
              <button onClick={() => onAction(c.id, "reply")} className="font-medium text-primary">{t("reply")}</button>
              {c.state !== "hidden" && <button onClick={() => onAction(c.id, "hide")} className="text-muted">{t("hide")}</button>}
              <button onClick={() => onAction(c.id, "block")} className="text-muted">{t("blockAuthor")}</button>
              <button onClick={() => onAction(c.id, "report")} className="ml-auto text-danger">{t("report")}</button>
            </div>
          </div>
        </div>
      ))}
    </Card>
  );
}

export function CommentPolicyCard({ policy, notify, autoClose, onChange }: { policy: CommentPolicy; notify: boolean; autoClose: boolean; onChange: (p: { policy?: CommentPolicy; notify?: boolean; autoClose?: boolean }) => void }) {
  const t = useTranslations("Comments");
  const opts: [CommentPolicy, string, string][] = [
    ["registered", t("policyRegistered"), t("policyRegisteredSub")],
    ["open_moderated", t("policyOpen"), t("policyOpenSub")],
    ["off", t("policyOff"), t("policyOffSub")],
  ];
  return (
    <Card className="flex flex-col gap-3.5 p-[18px] text-sm">
      <CardTitle>{t("policy")}</CardTitle>
      <div role="radiogroup" className="flex flex-col gap-2">
        {opts.map(([v, label, sub]) => (
          <button key={v} role="radio" aria-checked={policy === v} onClick={() => onChange({ policy: v })} className={`grid grid-cols-[18px_minmax(0,1fr)] gap-2.5 rounded-[10px] border px-3 py-2.5 text-left ${policy === v ? "border-primary" : "border-border"}`}>
            <span className={`mt-0.5 h-4 w-4 rounded-full border-[5px] bg-surface ${policy === v ? "border-primary" : "border-border"}`} />
            <span className="flex flex-col gap-0.5"><span className="font-medium">{label}</span><span className="text-xs leading-snug text-muted">{sub}</span></span>
          </button>
        ))}
      </div>
      <Toggle label={t("notify")} checked={notify} onChange={(v) => onChange({ notify: v })} />
      <Toggle label={t("autoClose")} checked={autoClose} onChange={(v) => onChange({ autoClose: v })} />
    </Card>
  );
}
