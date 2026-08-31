"use client";

import { useRef, useState } from "react";

import { ApiClientError, api } from "@/lib/api";
import { ImageIcon } from "./icons";

interface CoverImageUploadProps {
  value: string | null;
  onChange: (url: string | null) => void;
  blogSlug: string;
  authFetch: <T>(fn: (token: string) => Promise<T>) => Promise<T>;
}

/** Area di caricamento della cover del post, stile fika.bar: 16:9, click per scegliere il file. */
export function CoverImageUpload({ value, onChange, blogSlug, authFetch }: CoverImageUploadProps) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleFile(file: File) {
    setUploading(true);
    setError(null);
    try {
      const { url } = await authFetch((token) => api.blogs.uploadMedia(token, blogSlug, file));
      onChange(url);
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : "Caricamento immagine non riuscito.");
    } finally {
      setUploading(false);
    }
  }

  if (value) {
    return (
      <div className="group relative aspect-[16/9] w-full overflow-hidden rounded-lg border border-border">
        {/* eslint-disable-next-line @next/next/no-img-element -- URL S3/MinIO esterno, non ottimizzabile da next/image senza configurare i domini */}
        <img src={value} alt="Copertina del post" className="h-full w-full object-cover" />
        <button
          type="button"
          onClick={() => onChange(null)}
          className="absolute right-2 top-2 rounded-md bg-background/90 px-2 py-1 text-xs text-foreground opacity-0 shadow-sm transition group-hover:opacity-100"
        >
          Rimuovi
        </button>
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
