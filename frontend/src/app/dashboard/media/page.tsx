"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { EmptyState, SkeletonCards } from "@/components/ui/States";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { MyMediaFile } from "@/lib/types";

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Vista "tutti i miei blog" (todo/UX_REDESIGN.md), generalizza la griglia
 * media del singolo blog (MediaTab.tsx, sola lettura qui) a tutti i blog
 * dell'utente insieme, con l'attribuzione del blog per elemento. */
export default function MyMediaPage() {
  const { authFetch } = useAuth();
  const t = useTranslations("MyMediaPage");
  const tc = useTranslations("Common");
  const [items, setItems] = useState<MyMediaFile[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    authFetch((token) => api.users.myMedia(token))
      .then(setItems)
      .catch((err) => setError(err instanceof ApiClientError ? err.message : tc("unexpectedError")));
  }, [authFetch, tc]);

  useEffect(load, [load]);

  return (
    <div className="mx-auto max-w-5xl">
      <div className="mb-5 flex flex-col gap-0.5">
        <h1 className="font-serif text-2xl font-medium text-foreground">{t("title")}</h1>
        <p className="text-[13px] text-muted">{items ? t("subtitle", { count: items.length }) : tc("loading")}</p>
      </div>

      {error && (
        <div className="mb-5">
          <Alert kind="error">{error}</Alert>
        </div>
      )}

      {items === null && !error && <SkeletonCards count={6} />}
      {items !== null && items.length === 0 && <EmptyState glyph="▣" title={t("emptyTitle")} body={t("emptyBody")} />}
      {items !== null && items.length > 0 && (
        <div className="grid grid-cols-2 gap-3 md:gap-5 lg:grid-cols-4 xl:grid-cols-5">
          {items.map((m) => (
            <figure key={m.id} className="flex min-w-0 flex-col gap-2">
              <div className="relative block aspect-square overflow-hidden rounded-[10px] border border-border bg-surface md:aspect-[4/3]">
                {/* eslint-disable-next-line @next/next/no-img-element -- URL storage esterno */}
                <img
                  src={m.url}
                  alt={m.alt_text}
                  className={`h-full w-full object-cover ${m.is_sensitive || m.categories.length > 0 ? "blur-xl" : ""}`}
                />
                {(m.is_sensitive || m.categories.length > 0) && (
                  <span className="absolute right-2 top-2 rounded bg-sensitive px-1.5 py-0.5 text-[11px] font-semibold text-white">
                    {t("sensitive")}
                  </span>
                )}
              </div>
              <figcaption className="flex flex-col gap-0.5 text-[13px] leading-snug">
                <Link
                  href={`/dashboard/blogs/${m.blog_slug}?tab=media`}
                  className="w-fit truncate text-[11px] font-semibold uppercase tracking-[.04em] text-primary no-underline hover:underline"
                >
                  {m.blog_title}
                </Link>
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
  );
}
