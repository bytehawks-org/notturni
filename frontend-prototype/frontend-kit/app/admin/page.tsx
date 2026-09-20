import { getTranslations } from "next-intl/server";

import { DashboardShell, ADMIN_NAV } from "@/components/shell/DashboardShell";
import { KpiRow, ActivityList } from "@/components/dashboard/blog/OverviewTab";
import { QueuesCard, ServicesCard } from "@/components/admin/AdminOverview";

/** /admin overview (5d). Endpoints to add: GET /api/v1/admin/overview (kpis, queues, audit tail, services health). */
export default async function AdminOverviewPage() {
  const [d, t] = await Promise.all([getAdminOverview(), getTranslations("Admin")]);
  return (
    <DashboardShell items={ADMIN_NAV} eyebrow="administration">
      <div className="flex flex-col gap-5">
        <div className="flex items-end justify-between"><div className="flex flex-col gap-1"><h1 className="font-serif text-[30px] font-medium">{t("overview")}</h1><span className="text-sm text-muted">{d.date} · {t("allSystems")} <span className="text-ok">● {t("operational")}</span></span></div></div>
        <KpiRow kpis={d.kpis} />
        <div className="grid gap-4 lg:grid-cols-2"><QueuesCard queues={d.queues} /><div className="flex flex-col gap-4"><ActivityList items={d.audit} /><ServicesCard services={d.services} /></div></div>
      </div>
    </DashboardShell>
  );
}
async function getAdminOverview() {
  return { date: new Date().toLocaleDateString("en-GB", { weekday: "long", day: "numeric", month: "long", year: "numeric" }),
    kpis: [{ label: "Users", value: "1,284", delta: "+12 this week", tone: "ok" as const }, { label: "Blogs", value: "143", delta: "86 public" }, { label: "Posts · 7d", value: "318", delta: "+9%", tone: "ok" as const }, { label: "Moderation", value: "8", delta: "2 over 48 h", tone: "danger" as const }, { label: "GDPR open", value: "3", delta: "next due in 4 d", tone: "warn" as const }],
    queues: [{ title: "Flagged images", sub: "self-hosted model · threshold 0.80", n: 2, sla: "oldest 40 min", tone: "muted" as const, href: "/admin/moderazione?type=image" }, { title: "Reported blogs", sub: "reserved names, spam", n: 1, sla: "3 reports", tone: "danger" as const, href: "/admin/blog?reported=1" }, { title: "GDPR requests", sub: "export · deletion", n: 3, sla: "due in 4 days", tone: "warn" as const, href: "/admin/gdpr" }],
    audit: [{ at: "15:02", who: "@root", what: "changed role of @giulia.b → Admin" }], services: [{ name: "API", value: "p95 84 ms", status: "ok" as const }, { name: "SMTP relay", value: "degraded · retrying", status: "degraded" as const }] };
}
