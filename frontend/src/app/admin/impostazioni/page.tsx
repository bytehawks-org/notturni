"use client";

import { useLocale, useTranslations } from "next-intl";
import { useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle, SectionLabel } from "@/components/ui/Card";
import { SegmentedControl, Toggle } from "@/components/ui/Controls";
import { Input, Label } from "@/components/ui/Field";
import { SkeletonRows } from "@/components/ui/States";
import { useToast } from "@/components/ui/Toast";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDateTime } from "@/lib/format";
import type { PlatformConfig } from "@/lib/types";

const SSO_ALL = ["google", "microsoft", "github", "linkedin"];

/** Impostazioni di piattaforma (mockup 5f), solo super admin: accesso,
 * blog e moderazione, infrastruttura in sola lettura. */
export default function PlatformSettingsPage() {
  const { user, authFetch } = useAuth();
  const t = useTranslations("PlatformSettings");
  const tc = useTranslations("Common");
  const locale = useLocale();
  const notify = useToast();
  const [config, setConfig] = useState<PlatformConfig | null>(null);
  const [draft, setDraft] = useState<PlatformConfig | null>(null);
  const [reservedInput, setReservedInput] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!user) return;
    authFetch((token) => api.admin.getConfig(token))
      .then((c) => {
        setConfig(c);
        setDraft(c);
      })
      .catch((err) => setError(err instanceof ApiClientError ? err.message : tc("unexpectedError")));
  }, [user, authFetch, tc]);

  if (user && user.platform_role !== "super_admin") return <Alert kind="error">{t("superAdminOnly")}</Alert>;
  if (error) return <Alert kind="error">{error}</Alert>;
  if (!draft || !config) return <SkeletonRows rows={5} />;

  const patch = (changes: Partial<PlatformConfig>) => setDraft((d) => (d ? { ...d, ...changes } : d));
  const dirty = JSON.stringify(draft) !== JSON.stringify(config);

  async function save() {
    if (!draft) return;
    setSaving(true);
    setError(null);
    try {
      const updated = await authFetch((token) =>
        api.admin.updateConfig(token, {
          default_locale: draft.default_locale,
          registration_mode: draft.registration_mode,
          sso_providers: draft.sso_providers,
          mfa_required_for_admins: draft.mfa_required_for_admins,
          reserved_blog_names: draft.reserved_blog_names,
          moderation_threshold: draft.moderation_threshold,
          max_blogs_per_user: draft.max_blogs_per_user,
          anonymous_comments_allowed: draft.anonymous_comments_allowed,
          audit_retention_days: draft.audit_retention_days,
        })
      );
      setConfig(updated);
      setDraft(updated);
      notify(t("savedToast"));
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : tc("unexpectedError"));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="mx-auto flex max-w-4xl flex-col gap-8">
      <header className="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
        <div className="flex flex-col gap-1">
          <h1 className="font-serif text-[28px] font-medium leading-tight text-foreground">{t("title")}</h1>
          <p className="text-sm text-muted">
            {t("subtitle")}
            {config.updated_at && ` · ${t("lastUpdated", { date: formatDateTime(config.updated_at, locale) })}`}
          </p>
        </div>
        <Button onClick={save} disabled={!dirty || saving}>
          {saving ? tc("saving") : t("save")}
        </Button>
      </header>

      <Card className="flex flex-col gap-5">
        <CardTitle>{t("access")}</CardTitle>
        <div className="grid gap-5 md:grid-cols-2">
          <div className="flex flex-col gap-1.5">
            <SectionLabel>{t("deploymentMode")}</SectionLabel>
            <span className="text-sm text-foreground">{String(config.infrastructure.deployment_mode)}</span>
            <span className="text-[13px] text-muted">{t("deploymentNote")}</span>
          </div>
          <div className="flex flex-col gap-1.5">
            <SectionLabel>{t("defaultLocale")}</SectionLabel>
            <SegmentedControl
              value={draft.default_locale}
              options={[
                { value: "it", label: "Italiano" },
                { value: "en", label: "English" },
              ]}
              onChange={(v) => patch({ default_locale: v })}
            />
            <span className="text-[13px] text-muted">{t("defaultLocaleNote")}</span>
          </div>
          <div className="flex flex-col gap-1.5">
            <SectionLabel>{t("registration")}</SectionLabel>
            <SegmentedControl
              value={draft.registration_mode}
              options={[
                { value: "open", label: t("reg.open") },
                { value: "invite", label: t("reg.invite") },
                { value: "closed", label: t("reg.closed") },
              ]}
              onChange={(v) => patch({ registration_mode: v })}
            />
            <span className="text-[13px] text-muted">{t("registrationNote")}</span>
          </div>
          <div className="flex flex-col gap-1.5">
            <SectionLabel>{t("sso")}</SectionLabel>
            <div className="flex flex-wrap gap-2">
              {SSO_ALL.map((p) => {
                const configured = config.sso_configured.includes(p);
                const enabled = draft.sso_providers.length === 0 ? configured : draft.sso_providers.includes(p);
                return (
                  <button
                    key={p}
                    type="button"
                    disabled={!configured}
                    onClick={() => {
                      const base = draft.sso_providers.length === 0 ? config.sso_configured : draft.sso_providers;
                      patch({ sso_providers: enabled ? base.filter((x) => x !== p) : [...base, p] });
                    }}
                    className={`rounded-full border px-3 py-1 text-[13px] capitalize ${enabled ? "border-primary bg-primary/10 text-primary" : "border-border text-muted"} disabled:opacity-40`}
                  >
                    {p} {enabled && "✓"}
                  </button>
                );
              })}
            </div>
            <span className="text-[13px] text-muted">{t("ssoNote")}</span>
          </div>
        </div>
        <Toggle checked={draft.mfa_required_for_admins} onChange={(v) => patch({ mfa_required_for_admins: v })} label={t("mfaAdmins")} />
      </Card>

      <Card className="flex flex-col gap-5">
        <CardTitle>{t("blogsModeration")}</CardTitle>
        <div className="flex flex-col gap-2">
          <SectionLabel>{t("reserved")}</SectionLabel>
          <div className="flex flex-wrap gap-2">
            {config.reserved_builtin.map((n) => (
              <span key={n} className="rounded-full bg-[color-mix(in_srgb,var(--muted)_16%,transparent)] px-2.5 py-1 font-mono text-xs text-muted">
                {n}
              </span>
            ))}
            {draft.reserved_blog_names.map((n) => (
              <span key={n} className="flex items-center gap-1 rounded-full border border-border px-2.5 py-1 font-mono text-xs">
                {n}
                <button type="button" onClick={() => patch({ reserved_blog_names: draft.reserved_blog_names.filter((x) => x !== n) })} className="text-muted hover:text-foreground" aria-label={tc("remove")}>
                  ×
                </button>
              </span>
            ))}
            <form
              className="flex items-center gap-1"
              onSubmit={(e) => {
                e.preventDefault();
                const v = reservedInput.trim().toLowerCase();
                if (v && !draft.reserved_blog_names.includes(v)) patch({ reserved_blog_names: [...draft.reserved_blog_names, v] });
                setReservedInput("");
              }}
            >
              <Input value={reservedInput} onChange={(e) => setReservedInput(e.target.value)} placeholder={t("reservedPlaceholder")} className="h-7 w-32 px-2 py-1 text-xs" />
              <Button type="submit" size="sm" variant="secondary">
                {tc("add")}
              </Button>
            </form>
          </div>
          <span className="text-[13px] text-muted">{t("reservedHint")}</span>
        </div>
        <div className="grid gap-5 md:grid-cols-2">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="threshold">{t("threshold")}</Label>
            <input
              id="threshold"
              type="range"
              min={0.5}
              max={0.99}
              step={0.01}
              value={draft.moderation_threshold}
              onChange={(e) => patch({ moderation_threshold: Number(e.target.value) })}
            />
            <span className="font-mono text-xs text-muted">{draft.moderation_threshold.toFixed(2)}</span>
            <span className="text-[13px] text-muted">{t("thresholdNote")}</span>
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="max-blogs">{t("maxBlogs")}</Label>
            <Input id="max-blogs" type="number" min={1} max={100} value={draft.max_blogs_per_user} onChange={(e) => patch({ max_blogs_per_user: Number(e.target.value) })} className="w-24" />
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="audit-retention">{t("auditRetention")}</Label>
            <Input
              id="audit-retention"
              type="number"
              min={7}
              max={3650}
              value={draft.audit_retention_days}
              onChange={(e) => patch({ audit_retention_days: Number(e.target.value) })}
              className="w-24"
            />
            <span className="text-[13px] text-muted">{t("auditRetentionNote")}</span>
          </div>
        </div>
        <Toggle checked={draft.anonymous_comments_allowed} onChange={(v) => patch({ anonymous_comments_allowed: v })} label={t("anonComments")} />
      </Card>

      <Card className="flex flex-col gap-3">
        <CardTitle>{t("infra")}</CardTitle>
        <p className="text-[13px] text-muted">{t("infraNote")}</p>
        <dl className="grid gap-x-6 gap-y-1.5 text-sm sm:grid-cols-2">
          {Object.entries(config.infrastructure).map(([k, v]) => (
            <div key={k} className="flex items-center justify-between gap-3 border-b border-border py-1">
              <dt className="font-mono text-xs text-muted">{k}</dt>
              <dd className="text-foreground">{v === null ? "—" : typeof v === "boolean" ? (v ? "✓" : "✗") : String(v)}</dd>
            </div>
          ))}
        </dl>
      </Card>
    </div>
  );
}
