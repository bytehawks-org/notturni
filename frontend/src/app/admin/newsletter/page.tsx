"use client";

import { useLocale, useTranslations } from "next-intl";
import { useCallback, useEffect, useState } from "react";

import { NewsletterCampaignModal } from "@/components/dashboard/blog/NewsletterCampaignModal";
import { NewsletterSettingsCard } from "@/components/dashboard/blog/NewsletterSettingsCard";
import { errorMessage } from "@/components/dashboard/blog/shared";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { Pill } from "@/components/ui/Pill";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDateTime } from "@/lib/format";
import type { AdminNewsletterSettings, NewsletterCampaign, NewsletterCampaignStatus, NewsletterStats } from "@/lib/types";

const STATUS_TONE: Record<NewsletterCampaignStatus, "neutral" | "ok" | "warn" | "info" | "danger"> = {
  draft: "neutral",
  scheduled: "info",
  sending: "warn",
  sent: "ok",
  canceled: "neutral",
  failed: "danger",
};

/** Digest di piattaforma (mockup admin, `blog_id=None`): stessi endpoint del
 * blog ma sotto `/admin/newsletter`, riservati ad Amministratore/Super Admin. */
export default function AdminNewsletterPage() {
  const { authFetch } = useAuth();
  const t = useTranslations("AdminNewsletter");
  const tStatus = useTranslations("NewsletterStatus");
  const tc = useTranslations("Common");
  const locale = useLocale();

  const [stats, setStats] = useState<NewsletterStats | null>(null);
  const [campaigns, setCampaigns] = useState<NewsletterCampaign[] | null>(null);
  const [settings, setSettings] = useState<AdminNewsletterSettings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showModal, setShowModal] = useState(false);
  const [editingCampaign, setEditingCampaign] = useState<NewsletterCampaign | null>(null);

  const load = useCallback(() => {
    authFetch((token) => api.newsletter.adminStats(token))
      .then(setStats)
      .catch((err) => setError(errorMessage(err, tc("unexpectedError"))));
    authFetch((token) => api.newsletter.adminCampaigns(token))
      .then(setCampaigns)
      .catch((err) => setError(errorMessage(err, tc("unexpectedError"))));
    authFetch((token) => api.newsletter.adminSettings(token))
      .then(setSettings)
      .catch((err) => setError(errorMessage(err, tc("unexpectedError"))));
  }, [authFetch, tc]);

  useEffect(load, [load]);

  async function handleCancelCampaign(campaignId: string) {
    if (!window.confirm(t("confirmCancelCampaign"))) return;
    try {
      await authFetch((token) => api.newsletter.cancelAdminCampaign(token, campaignId));
      load();
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    }
  }

  return (
    <div>
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <h1 className="font-serif text-2xl text-foreground">{t("title")}</h1>
        <Button onClick={() => setShowModal(true)}>{t("newCampaign")}</Button>
      </div>

      {error && <Alert kind="error">{error}</Alert>}

      {stats && (
        <div className="mb-6 grid grid-cols-3 gap-3">
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
      )}

      {settings && (
        <div className="mb-6">
          <NewsletterSettingsCard
            value={settings}
            onSave={async (patch) => {
              const updated = await authFetch((token) => api.newsletter.updateAdminSettings(token, patch));
              setSettings(updated);
            }}
          />
        </div>
      )}

      <Card className="flex flex-col gap-3">
        <CardTitle>{t("campaignsTitle")}</CardTitle>
        {campaigns !== null && campaigns.length === 0 && <p className="text-sm text-muted">{t("noCampaigns")}</p>}
        {campaigns && campaigns.length > 0 && (
          <div className="overflow-hidden rounded-lg border border-border">
            {campaigns.map((c) => (
              <div key={c.id} className="flex flex-wrap items-center justify-between gap-2 border-b border-border px-4 py-2.5 text-sm last:border-0">
                <div className="flex min-w-0 flex-col gap-0.5">
                  <span className="truncate text-foreground">{c.subject}</span>
                  <span className="text-[12px] text-muted">
                    {c.sent_at
                      ? t("sentAt", { date: formatDateTime(c.sent_at, locale) })
                      : c.scheduled_at
                        ? t("scheduledAt", { date: formatDateTime(c.scheduled_at, locale) })
                        : formatDateTime(c.created_at, locale)}
                  </span>
                </div>
                <div className="flex items-center gap-2.5">
                  <span className="text-[12px] text-muted">{t("recipients", { count: c.recipient_count })}</span>
                  <Pill tone={STATUS_TONE[c.status]}>{tStatus(c.status)}</Pill>
                  {c.status === "scheduled" && (
                    <>
                      <Button variant="secondary" size="sm" onClick={() => setEditingCampaign(c)}>
                        {tc("edit")}
                      </Button>
                      <Button variant="secondary" size="sm" onClick={() => handleCancelCampaign(c.id)}>
                        {t("cancelCampaign")}
                      </Button>
                    </>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </Card>

      <NewsletterCampaignModal
        open={showModal || editingCampaign !== null}
        blogSlug={null}
        editingCampaign={editingCampaign}
        onCancel={() => {
          setShowModal(false);
          setEditingCampaign(null);
        }}
        onCreated={() => {
          setShowModal(false);
          setEditingCampaign(null);
          load();
        }}
      />
    </div>
  );
}
