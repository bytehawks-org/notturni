"use client";

import { useTranslations } from "next-intl";
import { useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { FieldGroup, Input, Label, TextArea } from "@/components/ui/Field";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";

import { errorMessage } from "./shared";

/** Nuova campagna manuale (backend `CampaignCreateRequest`): `blogSlug` null
 * chiama l'endpoint di digest di piattaforma (`/admin/newsletter/campaigns`,
 * solo admin), valorizzato quello per blog — stesso form per entrambi i
 * contesti (dashboard blog e admin/newsletter). */
export function NewsletterCampaignModal({
  open,
  blogSlug,
  onCancel,
  onCreated,
}: {
  open: boolean;
  blogSlug: string | null;
  onCancel: () => void;
  onCreated: () => void;
}) {
  const { authFetch } = useAuth();
  const t = useTranslations("NewsletterCampaignModal");
  const tc = useTranslations("Common");
  const [subject, setSubject] = useState("");
  const [bodyMarkdown, setBodyMarkdown] = useState("");
  const [scheduledAt, setScheduledAt] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!open) return null;

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      const payload = {
        subject,
        body_markdown: bodyMarkdown,
        scheduled_at: scheduledAt ? new Date(scheduledAt).toISOString() : null,
      };
      await authFetch((token) =>
        blogSlug ? api.newsletter.createBlogCampaign(token, blogSlug, payload) : api.newsletter.createAdminCampaign(token, payload)
      );
      setSubject("");
      setBodyMarkdown("");
      setScheduledAt("");
      onCreated();
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-[rgb(10_10_12/0.55)] p-4" role="dialog" aria-modal="true">
      <form
        onSubmit={handleSubmit}
        className="flex w-full max-w-lg flex-col gap-3 rounded-xl border border-border bg-surface p-6 text-sm shadow-soft"
      >
        <h2 className="font-serif text-xl">{t("title")}</h2>

        <FieldGroup className="mb-0">
          <Label htmlFor="campaign-subject">{t("subject")}</Label>
          <Input
            id="campaign-subject"
            required
            value={subject}
            onChange={(e) => setSubject(e.target.value)}
          />
        </FieldGroup>

        <FieldGroup className="mb-0">
          <Label htmlFor="campaign-body" hint={t("bodyHint")}>
            {t("body")}
          </Label>
          <TextArea
            id="campaign-body"
            required
            rows={8}
            placeholder={t("bodyPlaceholder")}
            value={bodyMarkdown}
            onChange={(e) => setBodyMarkdown(e.target.value)}
          />
        </FieldGroup>

        <FieldGroup className="mb-0">
          <Label htmlFor="campaign-scheduled-at" hint={t("scheduledAtHint")}>
            {t("scheduledAt")}
          </Label>
          <Input
            id="campaign-scheduled-at"
            type="datetime-local"
            value={scheduledAt}
            onChange={(e) => setScheduledAt(e.target.value)}
          />
        </FieldGroup>

        {error && <Alert kind="error">{error}</Alert>}

        <div className="mt-1.5 flex justify-end gap-2">
          <Button type="button" variant="secondary" size="sm" onClick={onCancel}>
            {tc("cancel")}
          </Button>
          <Button type="submit" size="sm" disabled={submitting}>
            {submitting ? t("creating") : t("create")}
          </Button>
        </div>
      </form>
    </div>
  );
}
