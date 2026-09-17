import { useTranslations } from "next-intl";
import { Card } from "@/components/ui/Card";
import { Pill } from "@/components/ui/Pill";

export interface GdprRequest { id: string; user: string; scope: string; type: "export" | "deletion"; receivedAt: string; deadline: string; daysLeft: number; status: string; statusTone: "ok" | "warn" | "muted" }
const statusTone = { ok: "text-ok", warn: "text-warn", muted: "text-muted" };

/** GDPR queue (5f). Exports run automatically; deletions require a second admin (enforced by the API). */
export function GdprRequestsTable({ rows }: { rows: GdprRequest[] }) {
  const t = useTranslations("Gdpr");
  return (
    <Card className="overflow-hidden p-0">
      <div className="hidden grid-cols-[minmax(0,1fr)_120px_110px_140px_150px] gap-3 border-b border-border px-[18px] py-2 font-mono text-[11px] uppercase tracking-[.06em] text-muted md:grid"><span>{t("col.request")}</span><span>{t("col.type")}</span><span>{t("col.received")}</span><span>{t("col.deadline")}</span><span>{t("col.status")}</span></div>
      {rows.map((g) => (
        <div key={g.id} className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-b border-border px-[18px] py-[11px] text-sm last:border-0 md:grid-cols-[minmax(0,1fr)_120px_110px_140px_150px]">
          <div className="flex flex-col leading-snug"><span className="font-medium">@{g.user}</span><span className="text-xs text-muted">{g.scope}</span></div>
          <Pill tone={g.type === "deletion" ? "danger" : "info"}>{t(`type.${g.type}`)}</Pill>
          <span className="hidden text-[13px] text-muted md:block">{g.receivedAt}</span>
          <span className={`hidden text-[13px] md:block ${g.daysLeft <= 3 ? "text-danger" : g.daysLeft <= 10 ? "text-warn" : "text-muted"}`}>{g.deadline}</span>
          <span className={`hidden text-[13px] font-medium md:block ${statusTone[g.statusTone]}`}>{g.status}</span>
        </div>
      ))}
    </Card>
  );
}
