"use client";

import { useTranslations } from "next-intl";

import { Pill } from "@/components/ui/Pill";
import type { DisplayPostStatus } from "@/lib/post-status";

const TONE: Record<DisplayPostStatus | "hidden", "neutral" | "warn" | "info" | "ok" | "danger"> = {
  draft: "neutral",
  review: "warn",
  scheduled: "info",
  published: "ok",
  hidden: "danger",
};

/** Pillola di stato del post (mockup 1e/3g), colori funzionali di globals.css. */
export function StatusPill({ status }: { status: DisplayPostStatus | "hidden" }) {
  const t = useTranslations("Status");
  return <Pill tone={TONE[status]}>{t(status)}</Pill>;
}
