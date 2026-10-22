"use client";

import { useTranslations } from "next-intl";
import { useState } from "react";

import { Button } from "@/components/ui/Button";
import { SegmentedControl } from "@/components/ui/Controls";
import { displayPostStatus, type DisplayPostStatus } from "@/lib/post-status";
import type { Post } from "@/lib/types";

/** Mockup 1d: stato del post a tre vie (Bozza · Revisione · Pubblica) +
 * pianificazione. Transizioni disponibili dal backend: bozza → revisione
 * (`submit-for-review`), revisione → bozza (`return-to-draft`), qualsiasi →
 * pubblicato subito o a una data futura (`publish` con `published_at`). */
export function PostStatusControl({
  post,
  onChangeStatus,
  onPublish,
}: {
  post: Pick<Post, "status" | "published_at">;
  onChangeStatus: (target: "draft" | "review") => Promise<void>;
  onPublish: (publishedAt?: string) => Promise<void>;
}) {
  const t = useTranslations("Editor");
  const ts = useTranslations("Status");
  const status = displayPostStatus(post);
  const [scheduling, setScheduling] = useState(false);
  const [when, setWhen] = useState("");
  // Limite inferiore del selettore (adesso + 5 min), calcolato una volta sola.
  const [minWhen] = useState(() => new Date(Date.now() + 5 * 60 * 1000).toISOString().slice(0, 16));
  const [busy, setBusy] = useState(false);

  async function run(fn: () => Promise<void>) {
    setBusy(true);
    try {
      await fn();
    } finally {
      setBusy(false);
    }
  }

  const options: { value: DisplayPostStatus; label: string }[] = [
    { value: "draft", label: ts("draft") },
    { value: "review", label: ts("review") },
    { value: "published", label: status === "published" ? ts("published") : status === "scheduled" ? ts("scheduled") : t("publishNow") },
  ];

  // Lo stato "scheduled" non ha una propria voce nel controllo (mostra
  // "published" come selezionato, vedi `status === "scheduled" ? ...` sotto)
  // — il confronto nell'onChange deve usare la stessa normalizzazione,
  // altrimenti cliccare l'opzione già selezionata mentre il post è
  // programmato non veniva riconosciuto come "nessun cambiamento" e
  // pubblicava subito invece di non fare nulla.
  const selected = status === "scheduled" ? "published" : status;

  return (
    <div className="flex flex-col gap-2">
      <span className="text-xs font-semibold uppercase tracking-[.04em] text-muted">{t("status")}</span>
      <SegmentedControl
        value={selected}
        options={options}
        onChange={(v) => {
          if (busy || v === selected) return;
          if (v === "published") void run(() => onPublish());
          else void run(() => onChangeStatus(v === "review" ? "review" : "draft"));
        }}
      />
      {status === "scheduled" && post.published_at && (
        <span className="text-[13px] text-muted">{t("scheduledFor", { date: new Date(post.published_at).toLocaleString() })}</span>
      )}
      {status !== "published" && (
        <div className="flex flex-col gap-2">
          {!scheduling ? (
            <button type="button" onClick={() => setScheduling(true)} className="self-start text-[13px] text-primary hover:underline">
              {status === "scheduled" ? t("reschedule") : t("schedule")}
            </button>
          ) : (
            <div className="flex flex-col gap-2 rounded-lg border border-border bg-surface p-2.5">
              <input
                type="datetime-local"
                value={when}
                min={minWhen}
                onChange={(e) => setWhen(e.target.value)}
                className="rounded-md border border-border bg-background px-2 py-1.5 text-sm text-foreground focus:outline-none focus:border-primary"
              />
              <div className="flex gap-2">
                <Button
                  size="sm"
                  disabled={busy || !when}
                  onClick={() =>
                    run(async () => {
                      await onPublish(new Date(when).toISOString());
                      setScheduling(false);
                    })
                  }
                >
                  {t("confirmSchedule")}
                </Button>
                <Button size="sm" variant="secondary" onClick={() => setScheduling(false)}>
                  {t("cancel")}
                </Button>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
