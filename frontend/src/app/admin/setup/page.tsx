"use client";

import { useTranslations } from "next-intl";
import { useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle, SectionLabel } from "@/components/ui/Card";
import { SegmentedControl, Toggle } from "@/components/ui/Controls";
import { useAuth } from "@/lib/auth-context";
import { isSolo } from "@/lib/site";
import { generateEnv, generateValues, PLATFORM_DEFAULTS, SOLO_DEFAULTS, type SetupAnswers } from "@/lib/setup-wizard";

function CopyBlock({ title, text, copyLabel, copiedLabel }: { title: string; text: string; copyLabel: string; copiedLabel: string }) {
  const [copied, setCopied] = useState(false);
  async function copy() {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  }
  return (
    <Card className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-3">
        <CardTitle>{title}</CardTitle>
        <Button size="sm" variant="secondary" onClick={copy}>
          {copied ? copiedLabel : copyLabel}
        </Button>
      </div>
      <pre className="overflow-x-auto rounded-md border border-border bg-background p-3 font-mono text-xs text-foreground">{text}</pre>
    </Card>
  );
}

/** First-run wizard: pure client-side form. It only OUTPUTS env and Helm values; it never calls the API. */
export default function SetupWizardPage() {
  const { user } = useAuth();
  const t = useTranslations("AdminSetup");
  const [answers, setAnswers] = useState<SetupAnswers>(isSolo() ? SOLO_DEFAULTS : PLATFORM_DEFAULTS);
  const set = <K extends keyof SetupAnswers>(key: K, value: SetupAnswers[K]) => setAnswers((a) => ({ ...a, [key]: value }));

  if (user && user.platform_role !== "super_admin") return <Alert kind="error">{t("superAdminOnly")}</Alert>;

  return (
    <div className="mx-auto flex max-w-4xl flex-col gap-6">
      <header className="flex flex-col gap-1">
        <h1 className="font-serif text-[28px] font-medium leading-tight text-foreground">{t("title")}</h1>
        <p className="text-sm text-muted">{t("subtitle")}</p>
      </header>
      <Alert kind="info">{t("notice")}</Alert>

      <Card className="flex flex-col gap-5">
        <CardTitle>{t("questions")}</CardTitle>
        <div className="grid gap-5 md:grid-cols-2">
          <label className="flex flex-col gap-1.5">
            <SectionLabel>{t("host")}</SectionLabel>
            <input
              type="text"
              value={answers.host}
              placeholder="example.com"
              onChange={(e) => set("host", e.target.value)}
              className="rounded-md border border-border bg-background px-3 py-2 text-sm text-foreground"
            />
          </label>
          {answers.smtp === "relay" && (
            <label className="flex flex-col gap-1.5">
              <SectionLabel>{t("relayHost")}</SectionLabel>
              <input
                type="text"
                value={answers.relayHost}
                placeholder="smtp.example.com"
                onChange={(e) => set("relayHost", e.target.value)}
                className="rounded-md border border-border bg-background px-3 py-2 text-sm text-foreground"
              />
            </label>
          )}
          <div className="flex flex-col gap-1.5">
            <SectionLabel>{t("mode")}</SectionLabel>
            <SegmentedControl
              value={answers.mode}
              options={[{ value: "solo", label: "solo" }, { value: "platform", label: "platform" }]}
              onChange={(v) => set("mode", v)}
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <SectionLabel>{t("storage")}</SectionLabel>
            <SegmentedControl
              value={answers.storage}
              options={[{ value: "local", label: t("storageLocal") }, { value: "s3", label: "S3" }, { value: "garage", label: "Garage" }]}
              onChange={(v) => set("storage", v)}
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <SectionLabel>{t("cache")}</SectionLabel>
            <SegmentedControl
              value={answers.cache}
              options={[{ value: "none", label: t("cacheNone") }, { value: "redis", label: "Redis" }]}
              onChange={(v) => set("cache", v)}
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <SectionLabel>{t("smtp")}</SectionLabel>
            <SegmentedControl
              value={answers.smtp}
              options={[
                { value: "none", label: t("smtpNone") },
                { value: "host", label: t("smtpHost") },
                { value: "relay", label: t("smtpRelay") },
              ]}
              onChange={(v) => set("smtp", v)}
            />
          </div>
        </div>
        <Toggle checked={answers.subdomains} onChange={(v) => set("subdomains", v)} label={t("subdomains")} />
        <Toggle checked={answers.moderation} onChange={(v) => set("moderation", v)} label={t("moderation")} />
        <Toggle checked={answers.nats} onChange={(v) => set("nats", v)} label={t("nats")} />
      </Card>

      <p className="text-[13px] text-muted">{t("secretsNote")}</p>
      <CopyBlock title={t("envTitle")} text={generateEnv(answers)} copyLabel={t("copy")} copiedLabel={t("copied")} />
      <CopyBlock title={t("helmTitle")} text={generateValues(answers)} copyLabel={t("copy")} copiedLabel={t("copied")} />
    </div>
  );
}
