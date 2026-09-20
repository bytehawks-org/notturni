"use client";

import { useLocale, useTranslations } from "next-intl";
import { useCallback, useEffect, useMemo, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { EmptyState, SkeletonRows } from "@/components/ui/States";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDate } from "@/lib/format";
import type { MyLinkBibliographyEntry } from "@/lib/types";

function hostOf(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return url;
  }
}

/** Vista "tutti i miei blog" (todo/UX_REDESIGN.md), generalizza la
 * bibliografia dei link del singolo blog (/{blog}/link) a tutti i blog
 * dell'utente insieme, con l'attribuzione del blog per citazione. */
export default function MyLinksPage() {
  const { authFetch } = useAuth();
  const t = useTranslations("MyLinksPage");
  const tc = useTranslations("Common");
  const locale = useLocale();
  const [entries, setEntries] = useState<MyLinkBibliographyEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    authFetch((token) => api.users.myLinks(token))
      .then(setEntries)
      .catch((err) => setError(err instanceof ApiClientError ? err.message : tc("unexpectedError")));
  }, [authFetch, tc]);

  useEffect(load, [load]);

  const sortedGroups = useMemo(() => {
    if (!entries) return [];
    const groups = new Map<string, MyLinkBibliographyEntry[]>();
    for (const entry of entries) {
      const host = hostOf(entry.url);
      groups.set(host, [...(groups.get(host) ?? []), entry]);
    }
    return [...groups.entries()].sort((a, b) => b[1].length - a[1].length);
  }, [entries]);

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

      {entries === null && !error && <SkeletonRows rows={4} />}
      {entries !== null && entries.length === 0 && <EmptyState glyph="⛓" title={t("emptyTitle")} body={t("emptyBody")} />}
      {sortedGroups.length > 0 && (
        <div className="flex flex-col gap-6">
          {sortedGroups.map(([host, items]) => (
            <section key={host} className="flex flex-col gap-2.5 border-t border-border pt-3.5">
              <h2 className="font-serif text-[17px]">{host}</h2>
              <ul className="flex flex-col gap-3">
                {items.map((entry) => (
                  <li key={entry.url} className="flex min-w-0 flex-col gap-1">
                    <a
                      href={entry.url}
                      target="_blank"
                      rel="noopener noreferrer nofollow"
                      className="text-[15px] font-medium text-foreground no-underline hover:text-primary"
                    >
                      {entry.link_text || entry.url}
                    </a>
                    <span className="truncate font-mono text-[13px] text-muted">{entry.url}</span>
                    <div className="flex flex-wrap gap-x-2 gap-y-1 text-xs text-muted">
                      {entry.citations.map((c, i) => (
                        <span key={`${c.blog_slug}-${c.permalink}-${i}`}>
                          <span className="font-semibold uppercase tracking-[.04em] text-primary">{c.blog_title}</span>
                          {" · "}
                          {c.post_title}
                          {c.used_at &&
                            ` · ${formatDate(c.used_at, locale, { day: "numeric", month: "short", year: "numeric" })}`}
                        </span>
                      ))}
                    </div>
                  </li>
                ))}
              </ul>
            </section>
          ))}
        </div>
      )}
    </div>
  );
}
