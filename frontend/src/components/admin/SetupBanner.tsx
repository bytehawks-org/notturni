"use client";

import Link from "next/link";
import { useTranslations } from "next-intl";
import { useState } from "react";

import { useSiteFlags } from "@/components/SiteConfigProvider";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";

export const SETUP_DISMISS_KEY = "noct.setupWizardDismissed";

/** Link to the first-run wizard, shown in solo only. Dismiss state is a per-viewer convenience (localStorage). */
export function SetupBanner() {
  const { isSolo } = useSiteFlags();
  const t = useTranslations("AdminSetup");
  // Only rendered after the admin layout has authenticated on the client, so reading storage here is safe.
  const [dismissed, setDismissed] = useState(() => {
    try {
      return window.localStorage.getItem(SETUP_DISMISS_KEY) === "1";
    } catch {
      return false;
    }
  });

  if (!isSolo || dismissed) return null;

  function dismiss() {
    setDismissed(true);
    try {
      window.localStorage.setItem(SETUP_DISMISS_KEY, "1");
    } catch {
      /* per-viewer convenience only */
    }
  }

  return (
    <Alert kind="info">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <span>{t("bannerText")}</span>
        <span className="flex items-center gap-2">
          <Link href="/admin/setup" className="text-primary no-underline hover:underline">
            {t("bannerOpen")}
          </Link>
          <Button size="sm" variant="secondary" onClick={dismiss}>
            {t("bannerDismiss")}
          </Button>
        </span>
      </div>
    </Alert>
  );
}
