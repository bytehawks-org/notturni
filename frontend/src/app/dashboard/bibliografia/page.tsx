"use client";

import { useTranslations } from "next-intl";
import { useCallback, useEffect, useState, type ReactNode } from "react";

import { Alert } from "@/components/ui/Alert";
import { EmptyState, SkeletonRows } from "@/components/ui/States";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { MyBibliographyEntry } from "@/lib/types";

/** Vista "tutti i miei blog" (todo/UX_REDESIGN.md), generalizza la
 * bibliografia automatica del singolo blog (/{blog}/bibliografia) a tutti i
 * blog dell'utente insieme, con l'attribuzione del blog per citazione. Il
 * contenuto della nota è mostrato come testo semplice: `renderNoteInline`
 * (lib/markdown.ts) è `server-only`, non utilizzabile in questa pagina client. */
export default function MyBibliographyPage() {
  const { authFetch } = useAuth();
  const t = useTranslations("MyBibliographyPage");
  const tb = useTranslations("Bibliography");
  const tc = useTranslations("Common");
  const [entries, setEntries] = useState<MyBibliographyEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    authFetch((token) => api.users.myBibliography(token))
      .then(setEntries)
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

      {entries === null && !error && <SkeletonRows rows={4} />}
      {entries !== null && entries.length === 0 && (
        <EmptyState glyph="❦" title={t("emptyTitle")} body={t("emptyBody")} />
      )}
      {entries !== null && entries.length > 0 && (
        <ol className="flex flex-col">
          {entries.map((entry, i) => (
            <li key={i} className="grid grid-cols-[30px_minmax(0,1fr)] gap-2.5 border-b border-border py-3.5 last:border-0">
              <span className="pt-0.5 font-serif text-[15px] text-muted">{String(i + 1).padStart(2, "0")}</span>
              <div className="flex min-w-0 flex-col gap-2">
                <p className="text-[15px] leading-[1.45] text-foreground">{entry.content}</p>
                {(entry.author || entry.title || entry.source || entry.issued || entry.page || entry.isbn || entry.doi) && (
                  <p className="text-[13px] text-muted">
                    {(
                      [
                        entry.author,
                        entry.title ? <em key="title">{entry.title}</em> : null,
                        entry.source,
                        entry.issued,
                        entry.page ? tb("page", { page: entry.page }) : null,
                        entry.isbn ? tb("isbn", { isbn: entry.isbn }) : null,
                        entry.doi ? (
                          <a
                            key="doi"
                            href={`https://doi.org/${entry.doi}`}
                            target="_blank"
                            rel="noopener noreferrer nofollow"
                            className="text-primary no-underline hover:underline"
                          >
                            doi.org/{entry.doi}
                          </a>
                        ) : null,
                      ] as (ReactNode | null)[]
                    )
                      .filter((part): part is ReactNode => part !== null && part !== "")
                      .reduce<ReactNode[]>((acc, part, idx) => (idx > 0 ? [...acc, " · ", part] : [part]), [])}
                  </p>
                )}
                {entry.kind && (
                  <span className="w-fit rounded border border-border px-1.5 py-0.5 font-mono text-[11px] uppercase tracking-[.06em] text-muted">
                    {tb(`kind.${entry.kind}`)}
                  </span>
                )}
                <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[13px] text-muted">
                  <span>{tb("citedIn")}</span>
                  {entry.citations.map((c, j) => (
                    <span key={`${c.blog_slug}-${c.permalink}-${c.idx}`}>
                      <span className="font-semibold uppercase tracking-[.04em] text-primary">{c.blog_title}</span>
                      {" · "}
                      {c.post_title}
                      {c.locale ? ` (${c.locale})` : ""}
                      {j < entry.citations.length - 1 ? "," : ""}
                    </span>
                  ))}
                </div>
              </div>
            </li>
          ))}
        </ol>
      )}
    </div>
  );
}
