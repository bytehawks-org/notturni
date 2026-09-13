import { useTranslations } from "next-intl";
import Link from "next/link";
import { Card, CardTitle } from "@/components/ui/Card";

export interface Queue { title: string; sub: string; n: number; sla: string; tone: "muted" | "warn" | "danger"; href: string }
export interface Service { name: string; value: string; status: "ok" | "degraded" | "down" }
const slaTone = { muted: "text-muted", warn: "text-warn", danger: "text-danger" };
const svcTone = { ok: "text-ok", degraded: "text-[#d9a441]", down: "text-danger" };

export function QueuesCard({ queues }: { queues: Queue[] }) {
  const t = useTranslations("Admin");
  return (
    <Card className="overflow-hidden p-0">
      <div className="flex items-center justify-between border-b border-border px-[18px] py-3"><CardTitle>{t("openQueues")}</CardTitle><span className="text-[13px] text-muted">{t("oldestFirst")}</span></div>
      {queues.map((q) => (
        <div key={q.title} className="grid grid-cols-[minmax(0,1fr)_50px_auto] items-center gap-3 border-b border-border px-[18px] py-[11px] text-sm last:border-0 md:grid-cols-[minmax(0,1fr)_60px_110px_auto]">
          <div className="flex flex-col leading-snug"><span className="font-medium">{q.title}</span><span className="text-xs text-muted">{q.sub}</span></div>
          <span className="font-serif text-xl">{q.n}</span>
          <span className={`hidden text-xs md:block ${slaTone[q.tone]}`}>{q.sla}</span>
          <Link href={q.href} className="text-[13px] font-medium">{t("open")}</Link>
        </div>
      ))}
    </Card>
  );
}

export function ServicesCard({ services }: { services: Service[] }) {
  const t = useTranslations("Admin");
  return (
    <Card className="grid grid-cols-2 gap-x-[18px] gap-y-2.5 px-[18px] py-4 text-[13px]">
      <span className="col-span-full font-serif text-[17px]">{t("services")}</span>
      {services.map((s) => <span key={s.name} className="flex justify-between text-muted"><span><span className={svcTone[s.status]}>●</span> {s.name}</span><span>{s.value}</span></span>)}
    </Card>
  );
}
