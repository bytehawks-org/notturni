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

/** Tab Panoramica (mockup 5a, todo/UX_REDESIGN.md B1): KPI e "richiede la
 * tua attenzione" da `GET /blogs/{slug}/overview`. Il grafico delle letture
 * aggregate e lo spazio occupato arrivano con il blocco B2. */
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
    [data.pending_comments, t("kpi.pendingComments")],
    [data.members, t("kpi.members")],
  ];
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
          <span className="text-foreground">{t("mediaCount", { count: data.media })}</span>
          <span className="text-[13px] text-muted">{t("approvedComments", { count: data.approved_comments })}</span>
          {data.last_published_at && (
            <span className="text-[13px] text-muted">
              {t("lastPublished", { date: formatDate(data.last_published_at, locale, { day: "numeric", month: "short", year: "numeric" }) })}
            </span>
          )}
          <span className="text-[13px] text-muted">{t("readsNote")}</span>
        </Card>
      </div>
    </div>
  );
}
