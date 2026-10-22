"use client";

import { useTranslations } from "next-intl";
import { useEffect, useRef, useState } from "react";

import { ApiClientError, api } from "@/lib/api";
import type { MediaFile, MyMediaFile } from "@/lib/types";
import { ImageIcon } from "./icons";

export interface PickedImage {
  url: string;
  is_sensitive: boolean;
}

interface ImagePickerModalProps {
  /** Blog a cui va registrato un eventuale nuovo upload (tab "Carica") e di
   * cui mostrare la libreria (tab "Libreria del blog"). */
  blogSlug: string;
  authFetch: <T>(fn: (token: string) => Promise<T>) => Promise<T>;
  onSelect: (image: PickedImage) => void;
  onClose: () => void;
}

type Tab = "upload" | "blog" | "mine";

/** Modal "Inserisci immagine", stesso stile a overlay di ContentWarningModal/
 * NoteModal: tre schede — caricamento diretto (comportamento preesistente),
 * libreria del blog corrente (B7) e libreria aggregata di tutti i blog
 * dell'utente (già esposta in `/dashboard/media`, qui riusata come sorgente
 * di selezione invece che di sola consultazione). Nessuna integrazione
 * Unsplash in questo blocco. */
export function ImagePickerModal({ blogSlug, authFetch, onSelect, onClose }: ImagePickerModalProps) {
  const t = useTranslations("ImagePickerModal");
  const [tab, setTab] = useState<Tab>("upload");
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [blogItems, setBlogItems] = useState<MediaFile[] | null>(null);
  const [mineItems, setMineItems] = useState<MyMediaFile[] | null>(null);

  useEffect(() => {
    if (tab === "blog" && blogItems === null) {
      authFetch((token) => api.blogs.mediaLibrary(token, blogSlug))
        .then((lib) => setBlogItems(lib.items))
        .catch((err) => setError(err instanceof ApiClientError ? err.message : t("loadFailed")));
    }
    if (tab === "mine" && mineItems === null) {
      authFetch((token) => api.users.myMedia(token))
        .then(setMineItems)
        .catch((err) => setError(err instanceof ApiClientError ? err.message : t("loadFailed")));
    }
  }, [tab, blogItems, mineItems, authFetch, blogSlug, t]);

  async function handleFile(file: File) {
    setUploading(true);
    setError(null);
    try {
      const media = await authFetch((token) => api.blogs.uploadMedia(token, blogSlug, file));
      onSelect({ url: media.url, is_sensitive: media.is_sensitive });
      onClose();
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : t("uploadFailed"));
    } finally {
      setUploading(false);
    }
  }

  function pick(item: MediaFile | MyMediaFile) {
    onSelect({ url: item.url, is_sensitive: item.is_sensitive });
    onClose();
  }

  function renderGrid(items: (MediaFile | MyMediaFile)[] | null) {
    if (items === null) return <p className="py-10 text-center text-sm text-muted">{t("loading")}</p>;
    if (items.length === 0) return <p className="py-10 text-center text-sm text-muted">{t("empty")}</p>;
    return (
      <div className="grid max-h-80 grid-cols-3 gap-2 overflow-y-auto sm:grid-cols-4">
        {items.map((item) => (
          <button
            key={item.id}
            type="button"
            onClick={() => pick(item)}
            title={item.alt_text || undefined}
            className="group relative aspect-square overflow-hidden rounded-lg border border-border"
          >
            {/* eslint-disable-next-line @next/next/no-img-element -- URL storage esterno, non ottimizzabile da next/image */}
            <img
              src={item.url}
              alt=""
              className={`h-full w-full object-cover transition group-hover:opacity-80 ${
                item.is_sensitive || item.categories.length > 0 ? "blur-md" : ""
              }`}
            />
            {"blog_title" in item && (
              <span className="absolute inset-x-0 bottom-0 truncate bg-foreground/70 px-1.5 py-0.5 text-[10px] font-medium text-background">
                {item.blog_title}
              </span>
            )}
          </button>
        ))}
      </div>
    );
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-foreground/40 p-4"
      onClick={onClose}
      role="presentation"
    >
      <div
        className="w-full max-w-lg rounded-2xl border border-border bg-background p-5 shadow-lg"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
      >
        <h2 className="font-serif text-lg text-foreground">{t("title")}</h2>

        <div className="mt-4 flex gap-1 border-b border-border text-sm">
          <TabButton active={tab === "upload"} onClick={() => setTab("upload")}>
            {t("tabUpload")}
          </TabButton>
          <TabButton active={tab === "blog"} onClick={() => setTab("blog")}>
            {t("tabBlogLibrary")}
          </TabButton>
          <TabButton active={tab === "mine"} onClick={() => setTab("mine")}>
            {t("tabMyLibrary")}
          </TabButton>
        </div>

        <div className="mt-4">
          {tab === "upload" && (
            <button
              type="button"
              disabled={uploading}
              onClick={() => fileInputRef.current?.click()}
              className="flex aspect-video w-full flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-border/70 text-sm text-muted transition hover:border-primary/50 hover:text-primary disabled:opacity-60"
            >
              <ImageIcon />
              <span>{uploading ? t("uploading") : t("uploadCta")}</span>
            </button>
          )}
          {tab === "blog" && renderGrid(blogItems)}
          {tab === "mine" && renderGrid(mineItems)}
        </div>

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

        {error && <p className="mt-2 text-xs text-red-700">{error}</p>}

        <div className="mt-5 flex justify-end">
          <button type="button" onClick={onClose} className="text-[13px] text-muted hover:text-foreground">
            {t("cancel")}
          </button>
        </div>
      </div>
    </div>
  );
}

function TabButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`-mb-px border-b-2 px-3 py-2 font-medium transition ${
        active ? "border-primary text-primary" : "border-transparent text-muted hover:text-foreground"
      }`}
    >
      {children}
    </button>
  );
}
