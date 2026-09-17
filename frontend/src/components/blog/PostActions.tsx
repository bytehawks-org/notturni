"use client";

import { useTranslations } from "next-intl";

import { useToast } from "@/components/ui/Toast";

/** Mockup 1a: "Condividi" (Web Share API, altrimenti copia del link) e
 * "Cita" (citazione testuale con autore, titolo, blog, data e permalink). */
export function PostActions({
  permalink,
  title,
  citation,
}: {
  permalink: string;
  title: string;
  citation: string;
}) {
  const t = useTranslations("PostPage");
  const notify = useToast();
  const absolute = () => (permalink.startsWith("http") ? permalink : `${window.location.origin}${permalink}`);

  async function share() {
    const url = absolute();
    try {
      if (navigator.share) {
        await navigator.share({ title, url });
        return;
      }
      await navigator.clipboard.writeText(url);
      notify(t("linkCopied"));
    } catch {
      // condivisione annullata dall'utente o appunti non disponibili: nessun errore da mostrare
    }
  }

  async function cite() {
    try {
      await navigator.clipboard.writeText(`${citation} ${absolute()}`);
      notify(t("citationCopied"));
    } catch {
      notify(t("copyFailed"), "warn");
    }
  }

  const cls = "rounded-lg border border-border px-3 py-1.5 text-[13px] font-medium text-foreground hover:border-primary";
  return (
    <div className="flex items-center gap-2">
      <button type="button" onClick={share} className={cls}>
        {t("share")}
      </button>
      <button type="button" onClick={cite} className={cls}>
        {t("cite")}
      </button>
    </div>
  );
}
