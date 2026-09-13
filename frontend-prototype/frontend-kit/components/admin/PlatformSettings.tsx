"use client";
import { useTranslations } from "next-intl";
import { useState } from "react";
import { Card, CardTitle } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { Label, FieldGroup, TextArea } from "@/components/ui/Field";
import { SegmentedControl, Toggle } from "@/components/ui/Controls";
import { FilterChip } from "@/components/ui/Pill";

export interface PlatformConfig {
  deploymentMode: "solo" | "platform"; defaultLocale: "en" | "it"; registration: "open" | "invite" | "closed"; ssoProviders: string[]; mfaForAdmins: boolean;
  reservedNames: string[]; moderationThreshold: number; maxBlogsPerUser: number; anonymousComments: boolean;
  infra: { key: string; value: string }[];
}
const SSO = ["Google", "Microsoft", "GitHub", "LinkedIn"];

/** Super-admin only (5f). Every save is audited by the backend. Infra values are read-only (NOCT_* env). */
export function PlatformSettingsForm({ initial, onSave }: { initial: PlatformConfig; onSave: (c: PlatformConfig) => Promise<void> }) {
  const t = useTranslations("PlatformSettings");
  const [c, setC] = useState(initial);
  const up = <K extends keyof PlatformConfig>(k: K, v: PlatformConfig[K]) => setC((p) => ({ ...p, [k]: v }));
  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-end justify-between"><div className="flex flex-col gap-1"><h2 className="font-serif text-2xl font-medium">{t("title")}</h2><span className="text-sm text-muted">{t("subtitle")}</span></div><Button onClick={() => onSave(c)}>{t("save")}</Button></div>
      <div className="grid gap-4 text-sm lg:grid-cols-3">
        <Card className="flex flex-col gap-3 p-[18px]">
          <CardTitle>{t("access")}</CardTitle>
          <FieldGroup><Label>{t("deploymentMode")}</Label><SegmentedControl value={c.deploymentMode} options={[{ value: "solo", label: "solo" }, { value: "platform", label: "platform" }]} onChange={(v) => up("deploymentMode", v)} /></FieldGroup>
          <FieldGroup><Label>{t("defaultLocale")}</Label><SegmentedControl value={c.defaultLocale} options={[{ value: "en", label: "English" }, { value: "it", label: "Italiano" }]} onChange={(v) => up("defaultLocale", v)} /><span className="text-xs text-muted">{t("defaultLocaleNote")}</span></FieldGroup>
          <FieldGroup><Label>{t("registration")}</Label><SegmentedControl value={c.registration} options={[{ value: "open", label: t("reg.open") }, { value: "invite", label: t("reg.invite") }, { value: "closed", label: t("reg.closed") }]} onChange={(v) => up("registration", v)} /></FieldGroup>
          <FieldGroup><Label>{t("sso")}</Label><div className="flex flex-wrap gap-1.5">{SSO.map((p) => <FilterChip key={p} active={c.ssoProviders.includes(p)} onClick={() => up("ssoProviders", c.ssoProviders.includes(p) ? c.ssoProviders.filter((x) => x !== p) : [...c.ssoProviders, p])}>{p}{c.ssoProviders.includes(p) && " ✓"}</FilterChip>)}</div></FieldGroup>
          <Toggle label={t("mfaAdmins")} checked={c.mfaForAdmins} onChange={(v) => up("mfaForAdmins", v)} />
        </Card>
        <Card className="flex flex-col gap-3 p-[18px]">
          <CardTitle>{t("blogsModeration")}</CardTitle>
          <FieldGroup><Label hint={t("reservedHint")}>{t("reserved")}</Label><TextArea className="min-h-20 font-mono text-xs" value={c.reservedNames.join(" ")} onChange={(e) => up("reservedNames", e.target.value.split(/\s+/).filter(Boolean))} /></FieldGroup>
          <FieldGroup><Label hint={c.moderationThreshold.toFixed(2)}>{t("threshold")}</Label><input type="range" min={0.5} max={0.99} step={0.01} value={c.moderationThreshold} onChange={(e) => up("moderationThreshold", Number(e.target.value))} className="w-full accent-[var(--primary)]" /><span className="text-xs text-muted">{t("thresholdNote")}</span></FieldGroup>
          <div className="flex items-center justify-between"><span>{t("maxBlogs")}</span><input type="number" min={1} max={20} value={c.maxBlogsPerUser} onChange={(e) => up("maxBlogsPerUser", Number(e.target.value))} className="w-16 rounded-lg border border-border bg-surface px-2 py-1 text-center font-mono text-[13px]" /></div>
          <Toggle label={t("anonComments")} checked={c.anonymousComments} onChange={(v) => up("anonymousComments", v)} />
        </Card>
        <Card className="flex flex-col gap-3 p-[18px]">
          <CardTitle>{t("infra")}</CardTitle>
          {c.infra.map((i) => <div key={i.key} className="flex justify-between gap-3 border-b border-border py-1.5 text-[13px]"><span className="text-muted">{i.key}</span><span className="text-right font-mono">{i.value}</span></div>)}
          <span className="text-xs leading-relaxed text-muted">{t("infraNote")}</span>
        </Card>
      </div>
    </div>
  );
}
