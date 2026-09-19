"use client";

import { useLocale, useTranslations } from "next-intl";
import Link from "next/link";
import { useState, type FormEvent } from "react";

import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Field";
import { ApiClientError, api } from "@/lib/api";

/** Modulo di iscrizione alla newsletter (doppio opt-in,
 * backend/app/api/v1/newsletter.py): `variant="blog"` per il digest di un
 * singolo blog (richiede `blogSlug`), `variant="platform"` per il digest di
 * piattaforma. La risposta è sempre la stessa conferma generica — anche se
 * l'indirizzo è già iscritto — per non far trapelare quello stato
 * all'utente (anti-enumerazione, coerente col backend). */
export function NewsletterSignup({
  blogSlug = null,
  variant = "platform",
}: {
  blogSlug?: string | null;
  variant?: "blog" | "platform";
}) {
  const t = useTranslations("NewsletterSignup");
  const locale = useLocale();
  const [email, setEmail] = useState("");
  const [consent, setConsent] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [done, setDone] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    if (!consent) return;
    setSubmitting(true);
    setError(null);
    try {
      await api.newsletter.subscribe({ email, blog_slug: blogSlug, locale });
      setDone(true);
      setEmail("");
      setConsent(false);
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : t("error"));
    } finally {
      setSubmitting(false);
    }
  }

  if (done) {
    return (
      <div className="flex flex-col gap-1.5 rounded-xl border border-border bg-surface px-[18px] py-4">
        <span className="font-serif text-[17px] text-foreground">{t("title")}</span>
        <p className="text-[13px] leading-relaxed text-muted">{t("success")}</p>
      </div>
    );
  }

  return (
    <form
      onSubmit={handleSubmit}
      className="flex flex-col gap-3 rounded-xl border border-border bg-surface px-[18px] py-4"
    >
      <div className="flex flex-col gap-1">
        <span className="font-serif text-[17px] text-foreground">
          {variant === "blog" ? t("titleBlog") : t("title")}
        </span>
        <p className="text-[13px] leading-relaxed text-muted">{t("intro")}</p>
      </div>
      <Input
        type="email"
        required
        placeholder={t("emailPlaceholder")}
        aria-label={t("emailPlaceholder")}
        value={email}
        onChange={(e) => setEmail(e.target.value)}
      />
      <label className="flex items-start gap-2 text-[13px] leading-relaxed text-muted">
        <input
          type="checkbox"
          required
          checked={consent}
          onChange={(e) => setConsent(e.target.checked)}
          className="mt-0.5"
        />
        <span>
          {t.rich("consent", {
            privacy: (chunks) => (
              <Link href="/p/privacy" className="text-primary no-underline hover:underline">
                {chunks}
              </Link>
            ),
          })}
        </span>
      </label>
      {error && <p className="text-[13px] text-danger">{error}</p>}
      <Button type="submit" size="sm" disabled={submitting || !consent} className="self-start">
        {submitting ? t("submitting") : t("submit")}
      </Button>
    </form>
  );
}
