import { KpiRow, AttentionList, ReadsChart, ActivityList, StorageCard } from "@/components/dashboard/blog/OverviewTab";

/** Overview tab (5a / 5g). Data from GET /api/v1/blogs/{slug}/overview (to add: KPIs, attention items, daily reads, activity). */
export default async function BlogOverviewPage({ params }: { params: Promise<{ slug: string }> }) {
  const { slug } = await params;
  const o = await getOverview(slug);
  return (
    <div className="flex flex-col gap-5">
      <KpiRow kpis={o.kpis} />
      <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_380px] lg:gap-[22px]">
        <div className="flex flex-col gap-4"><AttentionList items={o.attention} /><ReadsChart values={o.reads} from={o.from} to={o.to} /></div>
        <div className="flex flex-col gap-4"><ActivityList items={o.activity} /><StorageCard usedMb={o.storage.usedMb} quotaMb={o.storage.quotaMb} images={o.storage.images} backups={o.storage.backups} /></div>
      </div>
    </div>
  );
}
async function getOverview(slug: string) {
  // placeholder until the endpoint exists
  return { kpis: [{ label: "Posts", value: "42" }, { label: "Followers", value: "214", delta: "+38 this week", tone: "ok" as const }, { label: "Reads · 30d", value: "6,120", delta: "+12%", tone: "ok" as const }, { label: "Comments pending", value: "3", delta: "oldest 2 days", tone: "warn" as const }, { label: "Newsletter", value: "187", delta: "2 bounces" }],
    attention: [{ title: "3 comments waiting for approval", sub: "Oldest from @giulia, 2 days ago", href: `/dashboard/blogs/${slug}/comments`, action: "Review", tone: "warn" as const }],
    reads: [40,52,38,61,55,72,48,44,66,80,58,50,47,63,70,90,76,58,52,49,64,72,84,60,55,68,74,100,88,79], from: "14 Aug", to: "12 Sep",
    activity: [{ at: "15:02", who: "M.", what: "sent “Reading in two languages” for review" }], storage: { usedMb: 412, quotaMb: 2048, images: 128, backups: 42 } };
}
