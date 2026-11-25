"use client";

import { useTranslations } from "next-intl";
import { useEffect, useRef, useState } from "react";

import { ApiClientError, api, type MediaUploadOutcome, uploadMediaMany } from "@/lib/api";
import { GALLERY_MAX } from "@/lib/gallery";
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
  /** Called with one image when a single file is picked or a library item is chosen. */
  onSelect: (image: PickedImage) => void;
  /**
   * Called with the images of a multi-file upload (2 to GALLERY_MAX files), in
   * selection order. Only successful uploads are passed. Optional: without it,
   * each image is passed to `onSelect` in turn.
   */
  onPickMany?: (images: PickedImage[]) => void;
  onClose: () => void;
}

type Tab = "upload" | "blog" | "mine";

/** One file of a multi-file upload batch. */
interface BatchItem {
  key: number;
  file: File;
  status: "uploading" | "done" | "failed";
  error?: string;
}

/** Modal "Inserisci immagine", stesso stile a overlay di ContentWarningModal/
 * NoteModal: tre schede — caricamento diretto (singolo file: comportamento
 * preesistente; da 2 a GALLERY_MAX file: caricamento multiplo con stato per
 * file), libreria del blog corrente (B7) e libreria aggregata di tutti i blog
 * dell'utente (già esposta in `/dashboard/media`, qui riusata come sorgente
 * di selezione invece che di sola consultazione). Nessuna integrazione
 * Unsplash in questo blocco. */
export function ImagePickerModal({ blogSlug, authFetch, onSelect, onPickMany, onClose }: ImagePickerModalProps) {
  const t = useTranslations("ImagePickerModal");
  const tg = useTranslations("Gallery");
  const [tab, setTab] = useState<Tab>("upload");
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [batch, setBatch] = useState<BatchItem[] | null>(null);
  // Mirror of `batch`, readable right after an await without stale closures.
  const batchRef = useRef<BatchItem[]>([]);
  const keySeq = useRef(0);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);

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

  function setItems(update: (items: BatchItem[]) => BatchItem[]) {
    const next = update(batchRef.current);
    batchRef.current = next;
    setBatch(next);
  }

  function emitMany(images: PickedImage[]) {
    if (images.length === 0) return;
    if (onPickMany) {
      onPickMany(images);
    } else {
      images.forEach(onSelect);
    }
  }

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

  /** Uploads the given batch items, updates their status and returns the successes to the parent. */
  async function runUploads(targets: BatchItem[]) {
    const keys = new Set(targets.map((item) => item.key));
    setItems((items) => items.map((i) => (keys.has(i.key) ? { ...i, status: "uploading", error: undefined } : i)));
    setUploading(true);
    setError(null);
    setProgress({ done: 0, total: targets.length });

    let outcomes: MediaUploadOutcome[];
    try {
      outcomes = await authFetch((token) =>
        uploadMediaMany(
          token,
          blogSlug,
          targets.map((item) => item.file),
          { onProgress: (done, total) => setProgress({ done, total }) },
        ),
      );
    } catch (err) {
      // Token or transport failure before any file settled: mark the whole run as failed.
      const message = err instanceof Error ? err.message : t("uploadFailed");
      outcomes = targets.map((item) => ({ file: item.file, ok: false, error: message }));
    }

    const byKey = new Map<number, MediaUploadOutcome>();
    targets.forEach((item, index) => byKey.set(item.key, outcomes[index]));
    setItems((items) =>
      items.map((i) => {
        const outcome = byKey.get(i.key);
        if (!outcome) return i;
        return outcome.ok ? { ...i, status: "done" } : { ...i, status: "failed", error: outcome.error };
      }),
    );

    setUploading(false);
    setProgress(null);

    const successes: PickedImage[] = [];
    targets.forEach((item, index) => {
      const outcome = outcomes[index];
      if (outcome.ok) successes.push({ url: outcome.media.url, is_sensitive: outcome.media.is_sensitive });
    });
    emitMany(successes);

    // Stay open while any file still failed, so the user can retry it.
    if (!batchRef.current.some((i) => i.status === "failed")) onClose();
  }

  function handleFiles(files: File[]) {
    if (files.length === 0) return;
    if (files.length > GALLERY_MAX) {
      setError(tg("tooMany", { max: GALLERY_MAX }));
      return;
    }
    if (files.length === 1) {
      setBatch(null);
      batchRef.current = [];
      void handleFile(files[0]);
      return;
    }
    const items: BatchItem[] = files.map((file) => {
      keySeq.current += 1;
      return { key: keySeq.current, file, status: "uploading" };
    });
    setItems(() => items);
    void runUploads(items);
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
            className="group relative aspect-square overflow-hidden rounded-lg border border-border focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
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
        className="max-h-[90vh] w-full max-w-lg overflow-y-auto rounded-2xl border border-border bg-background p-5 shadow-lg"
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
              className="flex min-h-11 aspect-video w-full flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-border/70 text-sm text-muted transition hover:border-primary/50 hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary disabled:opacity-60"
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
          multiple
          accept="image/png,image/jpeg,image/webp,image/gif"
          className="hidden"
          onChange={(e) => {
            handleFiles(Array.from(e.target.files ?? []));
            e.target.value = "";
          }}
        />

        <p aria-live="polite" className="mt-2 text-xs text-muted">
          {uploading && progress ? tg("uploading", { done: progress.done, total: progress.total }) : ""}
        </p>

        {batch && batch.length > 0 && (
          <ul className="mt-2 divide-y divide-border rounded-lg border border-border text-sm">
            {batch.map((item) => (
              <li key={item.key} className="flex flex-wrap items-center gap-2 px-3 py-2">
                {item.status === "failed" ? (
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-red-700">{tg("fileFailed", { name: item.file.name })}</p>
                    {item.error && <p className="text-xs text-red-700">{item.error}</p>}
                  </div>
                ) : (
                  <p className="min-w-0 flex-1 truncate text-foreground">
                    {item.file.name}
                    {item.status === "uploading" && <span className="ml-2 text-xs text-muted">{t("uploading")}</span>}
                  </p>
                )}
                {item.status === "failed" && (
                  <button
                    type="button"
                    disabled={uploading}
                    onClick={() => void runUploads([item])}
                    className="min-h-11 min-w-11 rounded-md px-3 text-primary underline-offset-2 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary disabled:opacity-60"
                  >
                    {tg("retry")}
                  </button>
                )}
              </li>
            ))}
          </ul>
        )}

        {error && <p className="mt-2 text-xs text-red-700">{error}</p>}

        <div className="mt-5 flex justify-end">
          <button
            type="button"
            onClick={onClose}
            className="min-h-11 min-w-11 rounded-md px-3 text-[13px] text-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
          >
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
      className={`-mb-px min-h-11 border-b-2 px-3 py-2 font-medium transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary ${
        active ? "border-primary text-primary" : "border-transparent text-muted hover:text-foreground"
      }`}
    >
      {children}
    </button>
  );
}
