"use client";

import { useTranslations } from "next-intl";
import { useSearchParams } from "next/navigation";
import { useState } from "react";

import { SiteHeader } from "@/components/SiteHeader";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { FieldGroup, Label, TextArea } from "@/components/ui/Field";
import { ApiClientError, api } from "@/lib/api";

/** Disiscrizione (link dall'email di ogni campagna): niente side-effect al
 * solo caricamento della pagina (`GET`) — email scanner e prefetch dei
 * client di posta seguirebbero il link senza che l'utente l'abbia mai
 * cliccato. Le due azioni partono solo da un click esplicito su un
 * pulsante, ciascuna con un proprio `POST`. */
export default function NewsletterUnsubscribePage() {
  const t = useTranslations("NewsletterUnsubscribePage");
  const tc = useTranslations("Common");
  const searchParams = useSearchParams();
  const token = searchParams.get("token") ?? "";

  const [reason, setReason] = useState("");
  const [submitting, setSubmitting] = useState<"unsubscribe" | "delete" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState<"unsubscribed" | "deleted" | null>(null);

  async function handleUnsubscribe() {
    setSubmitting("unsubscribe");
    setError(null);
    try {
      await api.newsletter.unsubscribe({ token, reason: reason.trim() || null });
      setDone("unsubscribed");
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : tc("unexpectedError"));
    } finally {
      setSubmitting(null);
    }
  }

  async function handleDelete() {
    setSubmitting("delete");
    setError(null);
    try {
      await api.newsletter.unsubscribeAndDelete({ token });
      setDone("deleted");
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : tc("unexpectedError"));
    } finally {
      setSubmitting(null);
    }
  }

  return (
    <>
      <SiteHeader />
      <main className="mx-auto flex w-full max-w-md flex-1 flex-col gap-6 px-5 py-16">
        <h1 className="font-serif text-3xl font-medium text-foreground">{t("title")}</h1>

        {!token ? (
          <Alert kind="error">{t("missingToken")}</Alert>
        ) : done === "unsubscribed" ? (
          <Alert kind="success">{t("unsubscribedSuccess")}</Alert>
        ) : done === "deleted" ? (
          <Alert kind="success">{t("deletedSuccess")}</Alert>
        ) : (
          <>
            <p className="text-[15px] leading-relaxed text-muted">{t("intro")}</p>

            <FieldGroup className="mb-0">
              <Label htmlFor="unsubscribe-reason">{t("reasonLabel")}</Label>
              <TextArea
                id="unsubscribe-reason"
                rows={3}
                placeholder={t("reasonPlaceholder")}
                value={reason}
                onChange={(e) => setReason(e.target.value)}
              />
            </FieldGroup>

            {error && <Alert kind="error">{error}</Alert>}

            <Button size="lg" onClick={handleUnsubscribe} disabled={submitting !== null}>
              {submitting === "unsubscribe" ? t("unsubscribing") : t("unsubscribe")}
            </Button>

            <div className="mt-2 flex flex-col gap-2 border-t border-border pt-5">
              <p className="text-[13px] leading-relaxed text-muted">{t("deleteExplanation")}</p>
              <Button variant="danger" onClick={handleDelete} disabled={submitting !== null}>
                {submitting === "delete" ? t("deleting") : t("deleteData")}
              </Button>
            </div>
          </>
        )}
      </main>
    </>
  );
}
