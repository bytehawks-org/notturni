"use client";

import { useTranslations } from "next-intl";
import { useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { RichTextEditor } from "@/components/editor/RichTextEditorLazy";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { NewsletterCampaign } from "@/lib/types";

import { errorMessage } from "./shared";

/** `scheduled_at` (UTC, dal backend) → valore locale per un input
 * `datetime-local`, che si aspetta l'ora del fuso del browser senza offset. */
function toLocalInputValue(iso: string): string {
  const date = new Date(iso);
  const localMs = date.getTime() - date.getTimezoneOffset() * 60000;
  return new Date(localMs).toISOString().slice(0, 16);
}

/** Nuova campagna manuale (backend `CampaignCreateRequest`) o modifica di una
 * già pianificata (`editingCampaign`, backend `CampaignUpdateRequest` —
 * consentita solo mentre `status === "scheduled"`, il backend risponde 409
 * altrimenti). `blogSlug` null chiama l'endpoint di digest di piattaforma
 * (`/admin/newsletter/campaigns`, solo admin), valorizzato quello per blog —
 * stesso form per entrambi i contesti (dashboard blog e admin/newsletter). */
export function NewsletterCampaignModal({
  open,
  blogSlug,
  editingCampaign,
  onCancel,
  onCreated,
}: {
  open: boolean;
  blogSlug: string | null;
  editingCampaign?: NewsletterCampaign | null;
  onCancel: () => void;
  onCreated: () => void;
}) {
  const { authFetch } = useAuth();
  const t = useTranslations("NewsletterCampaignModal");
  const tc = useTranslations("Common");
  const [subject, setSubject] = useState(editingCampaign?.subject ?? "");
  const [bodyMarkdown, setBodyMarkdown] = useState(editingCampaign?.body_markdown ?? "");
  const [scheduledAt, setScheduledAt] = useState(
    editingCampaign?.scheduled_at ? toLocalInputValue(editingCampaign.scheduled_at) : ""
  );
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!open) return null;

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    // RichTextEditor non è un controllo nativo: niente validazione HTML
    // "required" automatica come per il <textarea> sostituito, va rifatta qui.
    if (!bodyMarkdown.trim()) {
      setError(t("bodyRequired"));
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      const scheduled_at = scheduledAt ? new Date(scheduledAt).toISOString() : null;
      if (editingCampaign) {
        await authFetch((token) =>
          blogSlug
            ? api.newsletter.updateBlogCampaign(token, blogSlug, editingCampaign.id, {
                subject,
                body_markdown: bodyMarkdown,
                scheduled_at,
              })
            : api.newsletter.updateAdminCampaign(token, editingCampaign.id, {
                subject,
                body_markdown: bodyMarkdown,
                scheduled_at,
              })
        );
      } else {
        const payload = { subject, body_markdown: bodyMarkdown, scheduled_at };
        await authFetch((token) =>
          blogSlug ? api.newsletter.createBlogCampaign(token, blogSlug, payload) : api.newsletter.createAdminCampaign(token, payload)
        );
        setSubject("");
        setBodyMarkdown("");
        setScheduledAt("");
      }
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
        className="flex max-h-[90vh] w-full max-w-2xl flex-col gap-3 overflow-y-auto rounded-xl border border-border bg-surface p-6 text-sm shadow-soft"
      >
        <h2 className="font-serif text-xl">{editingCampaign ? t("editTitle") : t("title")}</h2>

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
          <Label hint={t("bodyHint")}>{t("body")}</Label>
          <RichTextEditor
            value={bodyMarkdown}
            onChange={setBodyMarkdown}
            blogSlug={blogSlug ?? undefined}
            authFetch={authFetch}
            placeholder={t("bodyPlaceholder")}
            stickyToolbar={false}
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
            {submitting ? t("creating") : editingCampaign ? tc("save") : t("create")}
          </Button>
        </div>
      </form>
    </div>
  );
}
