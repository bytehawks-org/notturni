"use client";

import { useTranslations } from "next-intl";
import { useRef, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { ApiClientError } from "@/lib/api";

import { errorMessage } from "./shared";

interface NewsletterBrandingValue {
  newsletter_sender_name: string | null;
  newsletter_banner_url: string | null;
  newsletter_banner_alt_text: string;
}

/** Nome mittente e banner delle email di campagna (backend
 * NewsletterSettingsRequest/AdminNewsletterSettingsRequest), condiviso tra
 * la tab Newsletter di un blog e `/admin/newsletter` (digest di
 * piattaforma): stesso form, endpoint diverso passato dal chiamante.
 * `onUpload` assente (digest di piattaforma, nessun blog a cui associare
 * l'upload): il banner si imposta incollando un URL già ospitato altrove,
 * come già il Markdown del footer di piattaforma. */
export function NewsletterSettingsCard({
  value,
  onSave,
  onUpload,
}: {
  value: NewsletterBrandingValue;
  onSave: (patch: Partial<NewsletterBrandingValue>) => Promise<void>;
  onUpload?: (file: File) => Promise<{ url: string; is_sensitive: boolean }>;
}) {
  const t = useTranslations("NewsletterSettingsCard");
  const tc = useTranslations("Common");
  const fileInputRef = useRef<HTMLInputElement>(null);

  const [senderName, setSenderName] = useState(value.newsletter_sender_name ?? "");
  const [bannerUrl, setBannerUrl] = useState(value.newsletter_banner_url ?? "");
  const [bannerAlt, setBannerAlt] = useState(value.newsletter_banner_alt_text);
  const [uploading, setUploading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleUpload(file: File) {
    if (!onUpload) return;
    setUploading(true);
    setError(null);
    try {
      const media = await onUpload(file);
      setBannerUrl(media.url);
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : t("uploadFailed"));
    } finally {
      setUploading(false);
    }
  }

  async function handleSave() {
    setSaving(true);
    setSaved(false);
    setError(null);
    try {
      await onSave({
        newsletter_sender_name: senderName.trim() || null,
        newsletter_banner_url: bannerUrl.trim() || null,
        newsletter_banner_alt_text: bannerAlt.trim(),
      });
      setSaved(true);
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    } finally {
      setSaving(false);
    }
  }

  return (
    <Card className="flex flex-col gap-3">
      <CardTitle>{t("title")}</CardTitle>
      <p className="text-[13px] leading-relaxed text-muted">{t("hint")}</p>

      <FieldGroup className="mb-0">
        <Label htmlFor="newsletter-sender-name">{t("senderNameLabel")}</Label>
        <Input
          id="newsletter-sender-name"
          value={senderName}
          placeholder={t("senderNamePlaceholder")}
          maxLength={120}
          onChange={(e) => {
            setSenderName(e.target.value);
            setSaved(false);
          }}
        />
      </FieldGroup>

      <FieldGroup className="mb-0">
        <Label htmlFor="newsletter-banner-url">{t("bannerLabel")}</Label>
        {bannerUrl && (
          <div className="mb-2 overflow-hidden rounded-lg border border-border">
            {/* eslint-disable-next-line @next/next/no-img-element -- URL storage esterno */}
            <img src={bannerUrl} alt={bannerAlt || t("bannerPreviewAlt")} className="max-h-32 w-full object-cover" />
          </div>
        )}
        <div className="flex gap-2">
          <Input
            id="newsletter-banner-url"
            value={bannerUrl}
            placeholder={t("bannerPlaceholder")}
            onChange={(e) => {
              setBannerUrl(e.target.value);
              setSaved(false);
            }}
          />
          {onUpload && (
            <>
              <Button type="button" variant="secondary" size="sm" disabled={uploading} onClick={() => fileInputRef.current?.click()}>
                {uploading ? t("uploading") : t("uploadCta")}
              </Button>
              <input
                ref={fileInputRef}
                type="file"
                accept="image/png,image/jpeg,image/webp,image/gif"
                className="hidden"
                onChange={(e) => {
                  const file = e.target.files?.[0];
                  if (file) void handleUpload(file);
                  e.target.value = "";
                }}
              />
            </>
          )}
        </div>
      </FieldGroup>

      <FieldGroup className="mb-0">
        <Label htmlFor="newsletter-banner-alt" hint={t("bannerAltHint")}>
          {t("bannerAltLabel")}
        </Label>
        <Input
          id="newsletter-banner-alt"
          value={bannerAlt}
          maxLength={300}
          disabled={!bannerUrl}
          placeholder={t("bannerAltPlaceholder")}
          onChange={(e) => {
            setBannerAlt(e.target.value);
            setSaved(false);
          }}
        />
      </FieldGroup>

      {error && <Alert kind="error">{error}</Alert>}
      {saved && !error && <Alert kind="success">{t("saved")}</Alert>}

      <div className="flex justify-end">
        <Button type="button" size="sm" disabled={saving} onClick={handleSave}>
          {saving ? tc("saving") : tc("save")}
        </Button>
      </div>
    </Card>
  );
}
