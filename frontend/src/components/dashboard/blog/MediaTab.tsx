"use client";

import { useLocale, useTranslations } from "next-intl";
import Link from "next/link";
import { useCallback, useEffect, useRef, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { FilterChip, Pill } from "@/components/ui/Pill";
import { EmptyState, SkeletonCards } from "@/components/ui/States";
import { useToast } from "@/components/ui/Toast";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { SENSITIVITY_CATEGORIES, SENSITIVITY_CATEGORY_LABELS, type SensitivityCategory } from "@/lib/content-media";
import { formatDate } from "@/lib/format";
import type { MediaFile, MediaLibrary } from "@/lib/types";

import { errorMessage } from "./shared";

type Filter = "all" | "unused" | "no-alt" | "sensitive";
const FILTERS: Filter[] = ["all", "unused", "no-alt", "sensitive"];

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Libreria media del blog (mockup 3c): griglia con badge, filtri, upload,
 * pannello "lightbox" con alt (richiesto), didascalia, avviso, "usata in",
 * dettagli, copia URL ed eliminazione (solo se non usata). */
export function MediaTab({ blogSlug, canWrite }: { blogSlug: string; canWrite: boolean }) {
  const { authFetch } = useAuth();
  const t = useTranslations("MediaTab");
  const tc = useTranslations("Common");
  const locale = useLocale();
  const notify = useToast();
  const fileInput = useRef<HTMLInputElement>(null);
  const [library, setLibrary] = useState<MediaLibrary | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [selected, setSelected] = useState<MediaFile | null>(null);
  const [alt, setAlt] = useState("");
  const [caption, setCaption] = useState("");
  const [categories, setCategories] = useState<SensitivityCategory[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    authFetch((token) => api.blogs.mediaLibrary(token, blogSlug))
      .then(setLibrary)
      .catch((err) => setError(errorMessage(err)));
  }, [authFetch, blogSlug]);
  useEffect(load, [load]);

  function open(m: MediaFile) {
    setSelected(m);
    setAlt(m.alt_text);
    setCaption(m.caption ?? "");
    setCategories(m.categories);
  }

  async function act(fn: () => Promise<unknown>, done?: string) {
    setBusy(true);
    setError(null);
    try {
      await fn();
      if (done) notify(done);
      load();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function upload(file: File) {
    await act(() => authFetch((token) => api.blogs.uploadMedia(token, blogSlug, file)), t("uploaded"));
  }

  const items = (library?.items ?? []).filter((m) =>
    filter === "unused" ? m.used_in.length === 0 : filter === "no-alt" ? !m.alt_text : filter === "sensitive" ? m.is_sensitive || m.categories.length > 0 : true
  );

  return (
    <div className={`grid gap-6 ${selected ? "xl:grid-cols-[minmax(0,1fr)_340px]" : ""}`}>
      <div className="flex flex-col gap-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex flex-col gap-0.5">
            <span className="text-sm text-muted">
              {library ? t("summary", { count: library.items.length, size: formatBytes(library.total_bytes) }) : tc("loading")}
            </span>
            <span className="text-[13px] text-muted">{t("storedNote")}</span>
          </div>
          {canWrite && (
            <div className="flex items-center gap-2">
              <Button size="sm" variant="secondary" disabled={busy} onClick={() => act(() => authFetch((token) => api.blogs.syncMediaLibrary(token, blogSlug)), t("synced"))}>
                {t("sync")}
              </Button>
              <Button size="sm" disabled={busy} onClick={() => fileInput.current?.click()}>
                {t("upload")}
              </Button>
              <input
                ref={fileInput}
                type="file"
                accept="image/png,image/jpeg,image/webp,image/gif"
                className="hidden"
                onChange={(e) => {
                  const f = e.target.files?.[0];
                  if (f) void upload(f);
                  e.target.value = "";
                }}
              />
            </div>
          )}
        </div>
        <div className="flex flex-wrap gap-1.5">
          {FILTERS.map((f) => (
            <FilterChip key={f} active={filter === f} onClick={() => setFilter(f)}>
              {t(`filter.${f}`)}
            </FilterChip>
          ))}
        </div>
        {error && <Alert kind="error">{error}</Alert>}
        {library === null && !error && <SkeletonCards count={4} />}
        {library !== null && items.length === 0 && <EmptyState glyph="▣" title={t("emptyTitle")} body={t("emptyBody")} />}
        {items.length > 0 && (
          <div className="grid grid-cols-2 gap-3 md:gap-5 lg:grid-cols-4 xl:grid-cols-5">
            {items.map((m) => (
              <figure key={m.id} className="flex min-w-0 flex-col gap-2">
                <button
                  type="button"
                  onClick={() => open(m)}
                  className={`relative block aspect-square overflow-hidden rounded-[10px] border bg-surface md:aspect-[4/3] ${selected?.id === m.id ? "border-primary" : "border-border"}`}
                >
                  {/* eslint-disable-next-line @next/next/no-img-element -- URL storage esterno */}
                  <img src={m.url} alt={m.alt_text} className={`h-full w-full object-cover transition ${m.is_sensitive || m.categories.length > 0 ? "blur-xl" : ""}`} />
                  {!m.alt_text && <span className="absolute left-2 top-2 rounded bg-danger px-1.5 py-0.5 text-[11px] font-semibold text-white">{t("badge.noAlt")}</span>}
                  {m.alt_text && m.used_in.length === 0 && <span className="absolute left-2 top-2 rounded bg-muted px-1.5 py-0.5 text-[11px] font-semibold text-background">{t("badge.unused")}</span>}
                  {(m.is_sensitive || m.categories.length > 0) && <span className="absolute right-2 top-2 rounded bg-sensitive px-1.5 py-0.5 text-[11px] font-semibold text-white">{t("badge.sensitive")}</span>}
                </button>
                <figcaption className="flex flex-col gap-0.5 text-[13px] leading-snug">
                  <span className="truncate font-medium">{m.alt_text || <span className="text-danger">{t("missingAlt")}</span>}</span>
                  <span className="truncate text-muted">
                    {m.used_in[0] ? `${t("in")} ${m.used_in[0].post_title}` : t("notUsed")}
                    {m.size_bytes > 0 && ` · ${formatBytes(m.size_bytes)}`}
                  </span>
                </figcaption>
              </figure>
            ))}
          </div>
        )}
      </div>

      {selected && (
        <aside className="flex h-fit flex-col gap-4 rounded-xl border border-border bg-surface p-5 xl:sticky xl:top-6">
          <div className="flex items-start justify-between gap-3">
            <span className="truncate font-mono text-xs text-muted">{selected.url.split("/").pop()}</span>
            <button type="button" onClick={() => setSelected(null)} className="text-muted hover:text-foreground" aria-label={tc("close")}>
              ✕
            </button>
          </div>
          {/* eslint-disable-next-line @next/next/no-img-element -- URL storage esterno */}
          <img src={selected.url} alt={selected.alt_text} className="max-h-60 w-full rounded-lg border border-border object-contain" />
          <label className="flex flex-col gap-1.5 text-sm">
            <span className="text-xs font-semibold uppercase tracking-[.04em] text-muted">
              {t("alt")} <span className="text-danger">· {t("required")}</span>
            </span>
            <input value={alt} onChange={(e) => setAlt(e.target.value)} disabled={!canWrite} className="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground focus:outline-none focus:border-primary" />
          </label>
          <label className="flex flex-col gap-1.5 text-sm">
            <span className="text-xs font-semibold uppercase tracking-[.04em] text-muted">{t("caption")}</span>
            <input value={caption} onChange={(e) => setCaption(e.target.value)} disabled={!canWrite} placeholder={t("captionPlaceholder")} className="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground focus:outline-none focus:border-primary" />
          </label>
          <div className="flex flex-col gap-1.5">
            <span className="text-xs font-semibold uppercase tracking-[.04em] text-muted">{t("warning")}</span>
            <div className="flex flex-wrap gap-1.5">
              <FilterChip active={categories.length === 0} onClick={() => canWrite && setCategories([])}>
                {t("none")}
              </FilterChip>
              {SENSITIVITY_CATEGORIES.map((c) => (
                <FilterChip key={c} active={categories.includes(c)} onClick={() => canWrite && setCategories((prev) => (prev.includes(c) ? prev.filter((x) => x !== c) : [...prev, c]))}>
                  {SENSITIVITY_CATEGORY_LABELS[c]}
                </FilterChip>
              ))}
            </div>
            <span className="text-[13px] text-muted">
              {t("autoModeration")}: {selected.is_sensitive ? t("flagged") : t("clean")} · {t("neverLeaves")}
            </span>
          </div>
          <div className="flex flex-col gap-1 text-[13px]">
            <span className="text-xs font-semibold uppercase tracking-[.04em] text-muted">{t("usedIn")}</span>
            {selected.used_in.length === 0 ? (
              <span className="text-muted">{t("notUsed")}</span>
            ) : (
              selected.used_in.map((u) => (
                <Link key={u.post_id} href={`/dashboard/blogs/${blogSlug}/posts/${u.post_id}`} className="text-primary no-underline hover:underline">
                  {u.post_title}
                </Link>
              ))
            )}
          </div>
          <span className="text-[13px] text-muted">
            {t("uploadedOn", { date: formatDate(selected.created_at, locale, { day: "numeric", month: "short", year: "numeric" }) })}
            {selected.uploader_username && ` · @${selected.uploader_username}`}
            {selected.size_bytes > 0 && ` · ${formatBytes(selected.size_bytes)}`}
            {selected.content_type && ` · ${selected.content_type}`}
          </span>
          <div className="flex flex-wrap items-center gap-2">
            {canWrite && (
              <Button
                size="sm"
                disabled={busy || !alt.trim()}
                onClick={() =>
                  act(async () => {
                    const updated = await authFetch((token) => api.blogs.updateMedia(token, blogSlug, selected.id, { alt_text: alt, caption, categories }));
                    setSelected(updated);
                  }, t("saved"))
                }
              >
                {tc("save")}
              </Button>
            )}
            <Button
              size="sm"
              variant="secondary"
              onClick={() => navigator.clipboard.writeText(selected.url).then(() => notify(t("urlCopied"))).catch(() => undefined)}
            >
              {t("copyUrl")}
            </Button>
            {canWrite && (
              <Button
                size="sm"
                variant="ghost"
                disabled={busy || selected.used_in.length > 0}
                title={selected.used_in.length > 0 ? t("deleteBlocked") : undefined}
                onClick={() =>
                  act(async () => {
                    await authFetch((token) => api.blogs.deleteMedia(token, blogSlug, selected.id));
                    setSelected(null);
                  }, t("deleted"))
                }
              >
                {tc("delete")}
              </Button>
            )}
          </div>
          {selected.used_in.length > 0 && <Pill tone="neutral">{t("deleteBlocked")}</Pill>}
        </aside>
      )}
    </div>
  );
}
