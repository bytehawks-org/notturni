import { useTranslations } from "next-intl";
import Link from "next/link";
import { Card, CardTitle, SectionLabel } from "@/components/ui/Card";

export interface Kpi { label: string; value: string; delta?: string; tone?: "ok" | "warn" | "danger" | "muted" }
export interface Attention { title: string; sub: string; href: string; action: string; tone: "warn" | "primary" | "muted" | "danger" | "info" }
export interface Activity { at: string; who: string; what: string }

const kpiTone = { ok: "text-ok", warn: "text-warn", danger: "text-danger", muted: "text-muted" };
const dot = { warn: "bg-warn", primary: "bg-primary", muted: "bg-muted", danger: "bg-danger", info: "bg-info" };

export function KpiRow({ kpis }: { kpis: Kpi[] }) {
  const t = useTranslations("Overview");
  return (
    <div className="grid grid-cols-3 gap-2.5 lg:grid-cols-5 lg:gap-3.5">
      {kpis.map((k) => (
        <Card key={k.label} className="flex flex-col gap-1 px-3 py-3 lg:px-[18px] lg:py-4">
          <span className="text-[11px] font-semibold uppercase tracking-[.04em] text-muted lg:text-xs">{k.label}</span>
          <span className="font-serif text-2xl leading-none lg:text-[30px]">{k.value}</span>
          {k.delta && <span className={`hidden text-xs lg:block ${kpiTone[k.tone ?? "muted"]}`}>{k.delta}</span>}
        </Card>
      ))}
    </div>
  );
}

export function AttentionList({ items }: { items: Attention[] }) {
  const t = useTranslations("Overview");
  return (
    <Card className="overflow-hidden p-0">
      <div className="flex items-center justify-between border-b border-border px-[18px] py-3"><CardTitle>{t("attention")}</CardTitle><span className="text-[13px] text-muted">{t("items", { count: items.length })}</span></div>
      <ul>
        {items.map((i) => (
          <li key={i.title} className="grid grid-cols-[10px_minmax(0,1fr)_auto] items-center gap-3.5 border-b border-border px-[18px] py-[11px] text-sm last:border-0">
            <span className={`h-2 w-2 rounded-full ${dot[i.tone]}`} />
            <div className="flex min-w-0 flex-col leading-snug"><span className="truncate font-medium">{i.title}</span><span className="text-xs text-muted">{i.sub}</span></div>
            <Link href={i.href} className="text-[13px] font-medium">{i.action}</Link>
          </li>
        ))}
      </ul>
    </Card>
  );
}

/** Reads sparkline. Values are daily aggregates from the backend — never per-reader. */
export function ReadsChart({ values, from, to }: { values: number[]; from: string; to: string }) {
  const t = useTranslations("Overview");
  const max = Math.max(...values, 1);
  return (
    <Card className="flex flex-col gap-3 px-[18px] py-4">
      <div className="flex items-baseline justify-between"><CardTitle>{t("reads")}</CardTitle><span className="hidden text-xs text-muted md:block">{t("readsNote")}</span></div>
      <div className="flex h-[72px] items-end gap-1" role="img" aria-label={t("readsAria", { from, to })}>{values.map((v, i) => <span key={i} className="flex-1 rounded-t-[3px] bg-primary/75" style={{ height: `${(v / max) * 100}%` }} />)}</div>
      <div className="flex justify-between text-xs text-muted"><span>{from}</span><span>{to}</span></div>
    </Card>
  );
}

export function ActivityList({ items }: { items: Activity[] }) {
  const t = useTranslations("Overview");
  return (
    <Card className="flex flex-col gap-2.5 px-[18px] py-4">
      <CardTitle>{t("activity")}</CardTitle>
      {items.map((a, i) => <div key={i} className="grid grid-cols-[52px_minmax(0,1fr)] gap-2.5 text-[13px] leading-snug"><span className="font-mono text-muted">{a.at}</span><span><b className="font-medium">{a.who}</b> {a.what}</span></div>)}
    </Card>
  );
}

export function StorageCard({ usedMb, quotaMb, images, backups }: { usedMb: number; quotaMb: number; images: number; backups: number }) {
  const t = useTranslations("Overview");
  return (
    <Card className="flex flex-col gap-2 px-[18px] py-4 text-[13px] text-muted">
      <CardTitle>{t("storage")}</CardTitle>
      <span className="block h-1.5 rounded-full bg-border"><span className="block h-1.5 rounded-full bg-primary" style={{ width: `${(usedMb / quotaMb) * 100}%` }} /></span>
      <span>{t("storageLine", { used: usedMb, quota: Math.round(quotaMb / 1024), images, backups })}</span>
    </Card>
  );
}
export { SectionLabel };
