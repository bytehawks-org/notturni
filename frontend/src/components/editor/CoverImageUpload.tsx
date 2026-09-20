"use client";

import { useTranslations } from "next-intl";
import { useRef, useState } from "react";

import { ApiClientError } from "@/lib/api";
import type { SensitivityCategory } from "@/lib/content-media";
import { Input, Label } from "@/components/ui/Field";
import { ContentWarningModal } from "./ContentWarningModal";
import { ImagePickerModal } from "./ImagePickerModal";
import { ImageIcon, ShieldIcon } from "./icons";

interface CoverImageUploadProps {
  value: string | null;
  isSensitive: boolean;
  categories: SensitivityCategory[];
  onChange: (url: string | null, isSensitive: boolean, categories: SensitivityCategory[]) => void;
  /** Esegue l'upload vero e proprio (endpoint diverso per la cover di un
   * post — `POST /blogs/{slug}/media` — e quella di un blog — `POST
   * /blogs/{slug}/cover-image`, vedi BlogCoverImageUpload): il componente
   * resta agnostico su dove va a finire il file. */
  onUpload: (file: File) => Promise<{ url: string; is_sensitive: boolean }>;
  /** Testo alternativo della cover (accessibilità). Assente: il campo non
   * viene mostrato (usato solo dove il chiamante lo gestisce, oggi sempre). */
  altText?: string;
  onAltTextChange?: (altText: string) => void;
  /** Se presenti insieme, mostra anche l'opzione "scegli dalla libreria"
   * (ImagePickerModal: libreria del blog + di tutti i propri blog) accanto
   * al caricamento diretto. Assenti nei contesti che non hanno ancora un
   * blog a cui associare l'upload (nessun caso oggi, ma il componente resta
   * utilizzabile anche senza). */
  blogSlug?: string;
  authFetch?: <T>(fn: (token: string) => Promise<T>) => Promise<T>;
}

/** Area di caricamento di una cover 16:9, stile fika.bar: click per
 * scegliere il file. Usata sia per la cover del post (editor) sia per la
 * cover del blog (SettingsTab), che caricano su endpoint diversi — vedi
 * `onUpload`. */
export function CoverImageUpload({
  value,
  isSensitive,
  categories,
  onChange,
  onUpload,
  altText,
  onAltTextChange,
  blogSlug,
  authFetch,
}: CoverImageUploadProps) {
  const t = useTranslations("CoverImageUpload");
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [revealed, setRevealed] = useState(false);
  const [showWarningModal, setShowWarningModal] = useState(false);
  const [showPickerModal, setShowPickerModal] = useState(false);
  const canPickFromLibrary = Boolean(blogSlug && authFetch);

  async function handleFile(file: File) {
    setUploading(true);
    setError(null);
    setRevealed(false);
    try {
      const media = await onUpload(file);
      onChange(media.url, media.is_sensitive, []);
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : t("uploadFailed"));
    } finally {
      setUploading(false);
    }
  }

  if (value) {
    const blurred = isSensitive && !revealed;
    return (
      <div>
        <div className="group relative aspect-[16/9] w-full overflow-hidden rounded-lg border border-border">
          {/* eslint-disable-next-line @next/next/no-img-element -- URL storage esterno, non ottimizzabile da next/image senza configurare i domini */}
          <img
            src={value}
            alt={altText || t("coverAlt")}
            onClick={() => blurred && setRevealed(true)}
            className={`h-full w-full object-cover ${blurred ? "cursor-pointer blur-2xl" : ""}`}
          />
          {blurred && (
            <div className="absolute inset-0 flex items-center justify-center bg-foreground/10 text-sm text-background">
              <span className="rounded-full bg-foreground/70 px-3 py-1">{t("sensitiveOverlay")}</span>
            </div>
          )}
          <div className="absolute right-2 top-2 flex gap-1.5 opacity-0 transition group-hover:opacity-100">
            {canPickFromLibrary && (
              <button
                type="button"
                onClick={() => setShowPickerModal(true)}
                className="rounded-md bg-background/90 px-2 py-1 text-xs text-foreground shadow-sm"
              >
                {t("changeFromLibrary")}
              </button>
            )}
            <button
              type="button"
              onClick={() => onChange(null, false, [])}
              className="rounded-md bg-background/90 px-2 py-1 text-xs text-foreground shadow-sm"
            >
              {t("remove")}
            </button>
          </div>
          <button
            type="button"
            onClick={() => setShowWarningModal(true)}
            className="absolute bottom-2 left-1/2 flex -translate-x-1/2 items-center gap-1.5 whitespace-nowrap rounded-full bg-foreground/70 px-3 py-1 text-xs font-medium text-background"
          >
            <ShieldIcon />
            {isSensitive ? t("warningOnContent") : t("addWarning")}
          </button>
          {showWarningModal && (
            <ContentWarningModal
              initialCategories={categories}
              onSave={(next) => onChange(value, next.length > 0, next)}
              onClose={() => setShowWarningModal(false)}
            />
          )}
        </div>
        {onAltTextChange && (
          <div className="mt-2">
            <Label htmlFor="cover-alt-text">{t("altTextLabel")}</Label>
            <Input
              id="cover-alt-text"
              defaultValue={altText ?? ""}
              key={value}
              placeholder={t("altTextPlaceholder")}
              maxLength={300}
              // onBlur, non onChange: per SettingsTab.tsx ogni chiamata salva
              // subito sul backend (niente stato locale intermedio lì) — un
              // salvataggio per tasto premuto sarebbe eccessivo. `key={value}`
              // rimonta il campo (scartando l'edit in corso) quando cambia
              // l'immagine, così non mostra mai l'alt text della cover precedente.
              onBlur={(e) => {
                if (e.target.value !== (altText ?? "")) onAltTextChange(e.target.value);
              }}
            />
          </div>
        )}
        {error && <p className="mt-1 text-xs text-red-700">{error}</p>}
        {showPickerModal && blogSlug && authFetch && (
          <ImagePickerModal
            blogSlug={blogSlug}
            authFetch={authFetch}
            onSelect={(image) => onChange(image.url, image.is_sensitive, [])}
            onClose={() => setShowPickerModal(false)}
          />
        )}
      </div>
    );
  }

  return (
    <div>
      <button
        type="button"
        disabled={uploading}
        onClick={() => fileInputRef.current?.click()}
        className="flex aspect-[16/9] w-full flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-border/70 text-sm text-muted transition hover:border-primary/50 hover:text-primary disabled:opacity-60"
      >
        <ImageIcon />
        <span>{uploading ? t("uploading") : t("uploadCta")}</span>
      </button>
      {canPickFromLibrary && (
        <button
          type="button"
          onClick={() => setShowPickerModal(true)}
          className="mt-1.5 w-full text-center text-xs text-muted hover:text-primary"
        >
          {t("orChooseFromLibrary")}
        </button>
      )}
      {showPickerModal && blogSlug && authFetch && (
        <ImagePickerModal
          blogSlug={blogSlug}
          authFetch={authFetch}
          onSelect={(image) => onChange(image.url, image.is_sensitive, [])}
          onClose={() => setShowPickerModal(false)}
        />
      )}
      <input
        ref={fileInputRef}
        type="file"
        accept="image/png,image/jpeg,image/webp,image/gif"
        className="hidden"
        onChange={(e) => {
          const file = e.target.files?.[0];
          if (file) void handleFile(file);
          e.target.value = "";
        }}
      />
      {error && <p className="mt-1 text-xs text-red-700">{error}</p>}
    </div>
  );
}
