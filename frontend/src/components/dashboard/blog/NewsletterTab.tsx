"use client";

import { useLocale, useTranslations } from "next-intl";
import { useCallback, useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { Toggle } from "@/components/ui/Controls";
import { SkeletonRows } from "@/components/ui/States";
import { Pill } from "@/components/ui/Pill";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDateTime } from "@/lib/format";
import type { NewsletterCampaign, NewsletterCampaignStatus, NewsletterStats } from "@/lib/types";

import { NewsletterCampaignModal } from "./NewsletterCampaignModal";
import { errorMessage } from "./shared";

const STATUS_TONE: Record<NewsletterCampaignStatus, "neutral" | "ok" | "warn" | "info" | "danger"> = {
  draft: "neutral",
  scheduled: "info",
  sending: "warn",
  sent: "ok",
  canceled: "neutral",
  failed: "danger",
};

/** Tab Newsletter (dashboard blog): iscritti, avviso automatico alla
 * pubblicazione (`newsletter_auto_notify_enabled`) e campagne manuali,
 * backend/app/api/v1/newsletter.py. */
export function NewsletterTab({
  blogSlug,
  initialAutoNotify,
}: {
  blogSlug: string;
  initialAutoNotify: boolean;
}) {
  const { authFetch } = useAuth();
  const t = useTranslations("NewsletterTab");
  const tKind = useTranslations("NewsletterKind");
  const tStatus = useTranslations("NewsletterStatus");
  const tc = useTranslations("Common");
  const locale = useLocale();

  const [stats, setStats] = useState<NewsletterStats | null>(null);
  const [campaigns, setCampaigns] = useState<NewsletterCampaign[] | null>(null);
  const [autoNotify, setAutoNotify] = useState(initialAutoNotify);
  const [error, setError] = useState<string | null>(null);
  const [showModal, setShowModal] = useState(false);

  const load = useCallback(() => {
    authFetch((token) => api.newsletter.blogStats(token, blogSlug))
      .then(setStats)
      .catch((err) => setError(errorMessage(err, tc("unexpectedError"))));
    authFetch((token) => api.newsletter.blogCampaigns(token, blogSlug))
      .then(setCampaigns)
      .catch((err) => setError(errorMessage(err, tc("unexpectedError"))));
  }, [authFetch, blogSlug, tc]);

  useEffect(load, [load]);

  async function handleToggleAutoNotify(value: boolean) {
    setAutoNotify(value);
    try {
      await authFetch((token) => api.newsletter.updateBlogSettings(token, blogSlug, { newsletter_auto_notify_enabled: value }));
    } catch (err) {
      setAutoNotify(!value);
      setError(errorMessage(err, tc("unexpectedError")));
    }
  }

  if (error) return <Alert kind="error">{error}</Alert>;
  if (!stats || !campaigns) return <SkeletonRows rows={4} />;

  return (
    <div className="space-y-6">
      <div className="grid grid-cols-3 gap-3">
        <div className="flex flex-col gap-1 rounded-xl border border-border bg-surface px-4 py-3.5">
          <span className="font-serif text-2xl text-foreground">{stats.pending}</span>
          <span className="text-xs text-muted">{t("pending")}</span>
        </div>
        <div className="flex flex-col gap-1 rounded-xl border border-border bg-surface px-4 py-3.5">
          <span className="font-serif text-2xl text-foreground">{stats.confirmed}</span>
          <span className="text-xs text-muted">{t("confirmed")}</span>
        </div>
        <div className="flex flex-col gap-1 rounded-xl border border-border bg-surface px-4 py-3.5">
          <span className="font-serif text-2xl text-foreground">{stats.unsubscribed}</span>
          <span className="text-xs text-muted">{t("unsubscribed")}</span>
        </div>
      </div>

      <Card className="flex flex-col gap-1">
        <Toggle checked={autoNotify} onChange={handleToggleAutoNotify} label={t("autoNotifyLabel")} />
        <p className="text-[13px] leading-relaxed text-muted">{t("autoNotifyHint")}</p>
      </Card>

      <Card className="flex flex-col gap-3">
        <div className="flex items-baseline justify-between">
          <CardTitle>{t("campaignsTitle")}</CardTitle>
          <Button size="sm" onClick={() => setShowModal(true)}>
            {t("newCampaign")}
          </Button>
        </div>
        {campaigns.length === 0 ? (
          <p className="text-sm text-muted">{t("noCampaigns")}</p>
        ) : (
          <div className="overflow-hidden rounded-lg border border-border">
            {campaigns.map((c) => (
              <div key={c.id} className="flex flex-wrap items-center justify-between gap-2 border-b border-border px-4 py-2.5 text-sm last:border-0">
                <div className="flex min-w-0 flex-col gap-0.5">
                  <span className="truncate text-foreground">{c.subject}</span>
                  <span className="text-[12px] text-muted">
                    {tKind(c.kind)}
                    {" · "}
                    {c.sent_at
                      ? t("sentAt", { date: formatDateTime(c.sent_at, locale) })
                      : c.scheduled_at
                        ? t("scheduledAt", { date: formatDateTime(c.scheduled_at, locale) })
                        : formatDateTime(c.created_at, locale)}
                  </span>
                </div>
                <div className="flex items-center gap-2.5">
                  <span className="text-[12px] text-muted">
                    {t("recipients", { count: c.recipient_count })}
                    {c.failed_count > 0 && ` · ${t("failed", { count: c.failed_count })}`}
                  </span>
                  <Pill tone={STATUS_TONE[c.status]}>{tStatus(c.status)}</Pill>
                </div>
              </div>
            ))}
          </div>
        )}
      </Card>

      <NewsletterCampaignModal
        open={showModal}
        blogSlug={blogSlug}
        onCancel={() => setShowModal(false)}
        onCreated={() => {
          setShowModal(false);
          load();
        }}
      />
    </div>
  );
}
