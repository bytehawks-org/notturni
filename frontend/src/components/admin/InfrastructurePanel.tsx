"use client";

import Link from "next/link";
import { useTranslations } from "next-intl";

import { Card, CardTitle } from "@/components/ui/Card";
import type { PlatformInfrastructure } from "@/lib/types";

function show(v: unknown): string {
  if (v === null || v === undefined) return "—";
  if (typeof v === "boolean") return v ? "✓" : "✗";
  if (Array.isArray(v)) return v.length ? v.join(", ") : "—";
  return String(v);
}

/** Read-only summary of the active infrastructure (never editable from the GUI). */
export function InfrastructurePanel({ infrastructure }: { infrastructure: PlatformInfrastructure }) {
  const t = useTranslations("PlatformSettings");
  return (
    <Card className="flex flex-col gap-3">
      <CardTitle>{t("infra")}</CardTitle>
      <p className="text-[13px] text-muted">{t("infraNote")}</p>
      <dl className="grid gap-x-6 gap-y-1.5 text-sm sm:grid-cols-2">
        {Object.entries(infrastructure).map(([k, v]) => (
          <div key={k} className="flex items-start justify-between gap-3 border-b border-border py-1">
            <dt className="font-mono text-xs text-muted" title={k}>
              {t.has(`infraKeys.${k}`) ? t(`infraKeys.${k}`) : k}
            </dt>
            <dd className="break-all text-right text-foreground">{show(v)}</dd>
          </div>
        ))}
      </dl>
      <Link href="/admin/setup" className="text-[13px] text-primary no-underline hover:underline">
        {t("setupLink")}
      </Link>
    </Card>
  );
}
