"use client";

import { useLocale, useTranslations } from "next-intl";
import Link from "next/link";
import { useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { SkeletonRows } from "@/components/ui/States";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDate } from "@/lib/format";
import type { BlogOverview } from "@/lib/types";

import { errorMessage } from "./shared";

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

/** Grafico a barre delle letture giornaliere (mockup 5a): solo aggregati
 * lato server, nessun tracciamento del singolo lettore. */
function ReadsChart({ data, ariaLabel }: { data: { day: string; reads: number }[]; ariaLabel: string }) {
  const max = Math.max(1, ...data.map((d) => d.reads));
  const w = 600;
  const h = 120;
  const gap = 4;
  const bw = (w - gap * (data.length - 1)) / data.length;
  return (
    <svg viewBox={`0 0 ${w} ${h}`} className="h-28 w-full" role="img" aria-label={ariaLabel}>
      {data.map((d, i) => {
        const bh = Math.max(2, (d.reads / max) * (h - 8));
        return (
          <rect
            key={d.day}
            x={i * (bw + gap)}
            y={h - bh}
            width={bw}
            height={bh}
            rx={2}
            className={d.reads > 0 ? "fill-primary" : "fill-border"}
          >
            <title>{`${d.day}: ${d.reads}`}</title>
          </rect>
        );
      })}
    </svg>
  );
}

/** Tab Panoramica (mockup 5a, todo/UX_REDESIGN.md B1/B2): KPI, "richiede la
 * tua attenzione", letture aggregate degli ultimi 30 giorni e spazio
 * occupato, da `GET /blogs/{slug}/overview`. */
export function OverviewTab({ blogSlug }: { blogSlug: string }) {
  const { authFetch } = useAuth();
  const t = useTranslations("Overview");
  const locale = useLocale();
  const [data, setData] = useState<BlogOverview | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    authFetch((token) => api.blogs.overview(token, blogSlug))
      .then(setData)
      .catch((err) => setError(errorMessage(err)));
  }, [authFetch, blogSlug]);

  if (error) return <Alert kind="error">{error}</Alert>;
  if (!data) return <SkeletonRows rows={4} />;

  const kpis: [number, string][] = [
    [data.posts_published, t("kpi.posts")],
    [data.followers, t("kpi.followers")],
    [data.reads_total_30d, t("kpi.reads")],
    [data.pending_comments, t("kpi.pendingComments")],
  ];
  const first = data.reads_30d[0]?.day ?? "";
  const last = data.reads_30d[data.reads_30d.length - 1]?.day ?? "";
  const fmtDay = (d: string) => formatDate(d, locale, { day: "numeric", month: "short" });
  const attention: { label: string; count: number; href: string }[] = [
    { label: t("attention.pendingComments"), count: data.pending_comments, href: `/dashboard/blogs/${blogSlug}?tab=comments` },
    { label: t("attention.inReview"), count: data.posts_in_review, href: `/dashboard/blogs/${blogSlug}?tab=posts` },
    { label: t("attention.drafts"), count: data.posts_draft, href: `/dashboard/blogs/${blogSlug}?tab=posts` },
    { label: t("attention.scheduled"), count: data.posts_scheduled, href: `/dashboard/blogs/${blogSlug}?tab=posts` },
  ].filter((a) => a.count > 0);

  return (
    <div className="flex flex-col gap-6">
      <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
        {kpis.map(([value, label]) => (
          <div key={label} className="flex flex-col gap-1 rounded-xl border border-border bg-surface px-4 py-3.5">
            <span className="font-serif text-2xl text-foreground">{value}</span>
            <span className="text-xs text-muted">{label}</span>
          </div>
        ))}
      </div>
      <Card className="flex flex-col gap-3">
        <div className="flex flex-wrap items-baseline justify-between gap-2">
          <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("reads")}</span>
          <span className="text-[13px] text-muted">{t("readsNote")}</span>
        </div>
        <ReadsChart data={data.reads_30d} ariaLabel={t("readsAria", { from: fmtDay(first), to: fmtDay(last) })} />
        <div className="flex justify-between font-mono text-[11px] text-muted">
          <span>{fmtDay(first)}</span>
          <span>{fmtDay(last)}</span>
        </div>
      </Card>
      <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_300px]">
        <Card className="flex flex-col gap-3">
          <div className="flex items-baseline justify-between">
            <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("attention")}</span>
            <span className="text-[13px] text-muted">{t("items", { count: attention.length })}</span>
          </div>
          {attention.length === 0 ? (
            <p className="text-sm text-muted">{t("nothingToDo")}</p>
          ) : (
            <ul className="flex flex-col">
              {attention.map((a) => (
                <li key={a.label} className="flex items-center justify-between border-b border-border py-2.5 text-sm last:border-0">
                  <span className="text-foreground">{a.label}</span>
                  <span className="flex items-center gap-3">
                    <span className="font-serif text-lg">{a.count}</span>
                    <Link href={a.href} className="text-[13px] text-primary no-underline hover:underline">
                      {t("open")}
                    </Link>
                  </span>
                </li>
              ))}
            </ul>
          )}
        </Card>
        <Card className="flex flex-col gap-2 text-sm">
          <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("storage")}</span>
          <span className="font-serif text-2xl text-foreground">
            {data.storage_bytes === null ? t("storageUnknown") : formatBytes(data.storage_bytes)}
            {data.storage_limit_mb !== null && (
              <span className="text-base text-muted"> / {formatBytes(data.storage_limit_mb * 1024 * 1024)}</span>
            )}
          </span>
          <span className="text-foreground">{t("mediaCount", { count: data.media })}</span>
          <span className="text-[13px] text-muted">{t("kpi.members")}: {data.members}</span>
          <span className="text-[13px] text-muted">{t("approvedComments", { count: data.approved_comments })}</span>
          {data.last_published_at && (
            <span className="text-[13px] text-muted">
              {t("lastPublished", { date: formatDate(data.last_published_at, locale, { day: "numeric", month: "short", year: "numeric" }) })}
            </span>
          )}
        </Card>
      </div>
    </div>
  );
}
