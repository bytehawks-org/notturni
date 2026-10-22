"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { EmptyState, SkeletonRows } from "@/components/ui/States";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { MyPublication } from "@/lib/types";

/** Vista "tutti i miei blog" (todo/UX_REDESIGN.md), generalizza l'elenco
 * pubblicazioni del singolo blog (PublicationsTab.tsx, sola lettura qui) a
 * tutti i blog dell'utente insieme, con l'attribuzione del blog per riga. */
export default function MyPublicationsPage() {
  const { authFetch } = useAuth();
  const t = useTranslations("MyPublicationsPage");
  const tc = useTranslations("Common");
  const [list, setList] = useState<MyPublication[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    authFetch((token) => api.users.myPublications(token))
      .then(setList)
      .catch((err) => setError(err instanceof ApiClientError ? err.message : tc("unexpectedError")));
  }, [authFetch, tc]);

  useEffect(load, [load]);

  return (
    <div className="mx-auto max-w-3xl">
      <div className="mb-5 flex flex-col gap-0.5">
        <h1 className="font-serif text-2xl font-medium text-foreground">{t("title")}</h1>
        <p className="text-[13px] text-muted">{t("subtitle")}</p>
      </div>

      {error && (
        <div className="mb-5">
          <Alert kind="error">{error}</Alert>
        </div>
      )}

      {list === null && !error && <SkeletonRows rows={3} />}
      {list !== null && list.length === 0 && <EmptyState glyph="▤" title={t("emptyTitle")} body={t("emptyBody")} />}
      {list !== null && list.length > 0 && (
        <ul className="flex flex-col rounded-xl border border-border bg-surface">
          {list.map((p) => (
            <li key={p.id} className="border-b border-border px-4 py-3 last:border-0">
              <Link
                href={`/dashboard/blogs/${p.blog_slug}?tab=publications`}
                className="w-fit truncate text-[11px] font-semibold uppercase tracking-[.04em] text-primary no-underline hover:underline"
              >
                {p.blog_title}
              </Link>
              <div className="flex flex-wrap items-baseline justify-between gap-2">
                <span className="font-serif text-[17px] text-foreground">{p.title}</span>
                <span className="text-[13px] text-muted">
                  {t("chaptersOf", { published: p.chapters_published, total: p.chapters_total })}
                </span>
              </div>
              <span className="font-mono text-xs text-muted">
                /{p.blog_slug}/pub/{p.name}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
