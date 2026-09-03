"use client";

import { useRef, useState } from "react";

import { ApiClientError, api } from "@/lib/api";
import type { SensitivityCategory } from "@/lib/content-media";
import { ContentWarningModal } from "./ContentWarningModal";
import { ImageIcon, ShieldIcon } from "./icons";

interface CoverImageUploadProps {
  value: string | null;
  isSensitive: boolean;
  categories: SensitivityCategory[];
  onChange: (url: string | null, isSensitive: boolean, categories: SensitivityCategory[]) => void;
  blogSlug: string;
  authFetch: <T>(fn: (token: string) => Promise<T>) => Promise<T>;
}

/** Area di caricamento della cover del post, stile fika.bar: 16:9, click per scegliere il file. */
export function CoverImageUpload({
  value,
  isSensitive,
  categories,
  onChange,
  blogSlug,
  authFetch,
}: CoverImageUploadProps) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [revealed, setRevealed] = useState(false);
  const [showWarningModal, setShowWarningModal] = useState(false);

  async function handleFile(file: File) {
    setUploading(true);
    setError(null);
    setRevealed(false);
    try {
      const media = await authFetch((token) => api.blogs.uploadMedia(token, blogSlug, file));
      onChange(media.url, media.is_sensitive, []);
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : "Caricamento immagine non riuscito.");
    } finally {
      setUploading(false);
    }
  }

  if (value) {
    const blurred = isSensitive && !revealed;
    return (
      <div className="group relative aspect-[16/9] w-full overflow-hidden rounded-lg border border-border">
        {/* eslint-disable-next-line @next/next/no-img-element -- URL S3/MinIO esterno, non ottimizzabile da next/image senza configurare i domini */}
        <img
          src={value}
          alt="Copertina del post"
          onClick={() => blurred && setRevealed(true)}
          className={`h-full w-full object-cover ${blurred ? "cursor-pointer blur-2xl" : ""}`}
        />
        {blurred && (
          <div className="absolute inset-0 flex items-center justify-center bg-foreground/10 text-sm text-background">
            <span className="rounded-full bg-foreground/70 px-3 py-1">Contenuto sensibile — clicca per vedere</span>
          </div>
        )}
        <button
          type="button"
          onClick={() => onChange(null, false, [])}
          className="absolute right-2 top-2 rounded-md bg-background/90 px-2 py-1 text-xs text-foreground opacity-0 shadow-sm transition group-hover:opacity-100"
        >
          Rimuovi
        </button>
        <button
          type="button"
          onClick={() => setShowWarningModal(true)}
          className="absolute bottom-2 left-1/2 flex -translate-x-1/2 items-center gap-1.5 whitespace-nowrap rounded-full bg-foreground/70 px-3 py-1 text-xs font-medium text-background"
        >
          <ShieldIcon />
          {isSensitive ? "Avviso sul contenuto" : "Aggiungi un avviso"}
        </button>
        {showWarningModal && (
          <ContentWarningModal
            initialCategories={categories}
            onSave={(next) => onChange(value, next.length > 0, next)}
            onClose={() => setShowWarningModal(false)}
          />
        )}
        {error && <p className="mt-1 text-xs text-red-700">{error}</p>}
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
        <span>{uploading ? "Caricamento…" : "Carica immagine di copertina (16:9, 800 × 450 px)"}</span>
      </button>
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
