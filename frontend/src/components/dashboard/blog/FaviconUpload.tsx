"use client";

import { useTranslations } from "next-intl";
import { useRef, useState } from "react";

import { ApiClientError } from "@/lib/api";

interface FaviconUploadProps {
  value: string | null;
  onUpload: (file: File) => Promise<string | null>;
  onRemove: () => Promise<void>;
}

/** Favicon dedicata del blog (facoltativa): icona quadrata, nessun avviso
 * sui contenuti a differenza della cover (BlogCoverImageUpload) — non ha
 * senso "sfocare" l'icona di una scheda del browser. */
export function FaviconUpload({ value, onUpload, onRemove }: FaviconUploadProps) {
  const t = useTranslations("FaviconUpload");
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleFile(file: File) {
    setUploading(true);
    setError(null);
    try {
      await onUpload(file);
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : t("uploadFailed"));
    } finally {
      setUploading(false);
    }
  }

  return (
    <div className="flex items-center gap-3">
      {value ? (
        // eslint-disable-next-line @next/next/no-img-element -- URL storage esterno
        <img src={value} alt={t("faviconAlt")} className="h-10 w-10 rounded-md border border-border object-cover" />
      ) : (
        <div className="h-10 w-10 rounded-md border border-dashed border-border/70" />
      )}
      <button
        type="button"
        disabled={uploading}
        onClick={() => fileInputRef.current?.click()}
        className="rounded-lg border border-border bg-surface px-3 py-1.5 text-sm text-foreground hover:border-primary/40 disabled:opacity-60"
      >
        {uploading ? t("uploading") : t("uploadCta")}
      </button>
      {value && (
        <button type="button" onClick={() => void onRemove()} className="text-sm text-muted hover:text-foreground">
          {t("remove")}
        </button>
      )}
      <input
        ref={fileInputRef}
        type="file"
        accept="image/png,image/jpeg,image/webp"
        className="hidden"
        onChange={(e) => {
          const file = e.target.files?.[0];
          if (file) void handleFile(file);
          e.target.value = "";
        }}
      />
      {error && <p className="text-xs text-red-700">{error}</p>}
    </div>
  );
}
