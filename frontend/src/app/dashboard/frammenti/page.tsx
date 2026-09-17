"use client";

import { useLocale, useTranslations } from "next-intl";
import Link from "next/link";
import { useCallback, useEffect, useMemo, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { EmptyState, SkeletonRows } from "@/components/ui/States";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDate } from "@/lib/format";
import type { FragmentCollectionEntry } from "@/lib/types";

type GroupMode = "none" | "post" | "author";

const GROUP_OPTIONS: { value: GroupMode; key: "groupAll" | "groupPost" | "groupAuthor" }[] = [
  { value: "none", key: "groupAll" },
  { value: "post", key: "groupPost" },
  { value: "author", key: "groupAuthor" },
];

function groupFragments(
  fragments: FragmentCollectionEntry[],
  mode: GroupMode
): { key: string; label: string; items: FragmentCollectionEntry[] }[] {
  if (mode === "none") return [{ key: "all", label: "", items: fragments }];
  const keyOf = (f: FragmentCollectionEntry) => (mode === "post" ? f.post_title : f.author_display_name);
  const groups = new Map<string, FragmentCollectionEntry[]>();
  for (const fragment of fragments) {
    const key = keyOf(fragment);
    groups.set(key, [...(groups.get(key) ?? []), fragment]);
  }
  return [...groups.entries()].map(([label, items]) => ({ key: label, label, items }));
}

export default function FragmentsPage() {
  const { authFetch } = useAuth();
  const t = useTranslations("FragmentsPage");
  const tc = useTranslations("Common");
  const locale = useLocale();
  const [fragments, setFragments] = useState<FragmentCollectionEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [groupMode, setGroupMode] = useState<GroupMode>("none");
  const [copiedId, setCopiedId] = useState<string | null>(null);

  const load = useCallback(() => {
    authFetch((token) => api.fragments.listMine(token))
      .then(setFragments)
      .catch((err) => setError(err instanceof ApiClientError ? err.message : tc("unexpectedError")));
  }, [authFetch, tc]);

  useEffect(load, [load]);

  async function handleRemove(id: string) {
    try {
      await authFetch((token) => api.fragments.remove(token, id));
      setFragments((prev) => prev?.filter((f) => f.id !== id) ?? null);
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : t("removeError"));
    }
  }

  async function handleToggleVisibility(id: string, isPublic: boolean) {
    try {
      const updated = await authFetch((token) => api.fragments.setPublic(token, id, isPublic));
      setFragments(
        (prev) => prev?.map((f) => (f.id === id ? { ...f, is_public: updated.is_public } : f)) ?? null
      );
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : t("visibilityError"));
    }
  }

  async function handleCopy(fragment: FragmentCollectionEntry) {
    try {
      await navigator.clipboard.writeText(fragment.text);
      setCopiedId(fragment.id);
      setTimeout(() => setCopiedId((current) => (current === fragment.id ? null : current)), 1500);
    } catch {
      // clipboard non disponibile (permessi/contesto non sicuro): nessun blocco, solo niente feedback
    }
  }

  const authorCount = useMemo(
    () => (fragments ? new Set(fragments.map((f) => f.author_display_name)).size : 0),
    [fragments]
  );
  const groups = useMemo(() => groupFragments(fragments ?? [], groupMode), [fragments, groupMode]);

  return (
    <div className="mx-auto max-w-2xl">
      <div className="mb-5 flex flex-wrap items-end justify-between gap-3">
        <div className="flex flex-col gap-0.5">
          <h1 className="font-serif text-2xl font-medium text-foreground">{t("title")}</h1>
          {fragments && fragments.length > 0 && (
            <span className="text-[13px] text-muted">
              {t("summary", { count: fragments.length, authors: authorCount })}
            </span>
          )}
        </div>
      </div>

      {fragments && fragments.length > 0 && (
        <div className="mb-5 flex gap-2 overflow-x-auto text-[13px]">
          {GROUP_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              type="button"
              onClick={() => setGroupMode(opt.value)}
              className={`shrink-0 rounded-full px-3 py-1.5 whitespace-nowrap ${
                groupMode === opt.value
                  ? "bg-foreground text-background"
                  : "border border-border text-muted hover:text-foreground"
              }`}
            >
              {t(opt.key)}
            </button>
          ))}
        </div>
      )}

      {error && (
        <div className="mb-5">
          <Alert kind="error">{error}</Alert>
        </div>
      )}

      {fragments === null ? (
        <SkeletonRows rows={4} />
      ) : fragments.length === 0 ? (
        <EmptyState
          title={t("emptyTitle")}
          body={t("emptyBody")}
          action={
            <Link href="/">
              <Button variant="secondary" size="sm">
                {t("browseFeed")}
              </Button>
            </Link>
          }
        />
      ) : (
        <div className="flex flex-col gap-8">
          {groups.map((group) => (
            <div key={group.key} className="flex flex-col gap-3.5">
              {group.label && (
                <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{group.label}</span>
              )}
              <ul className="flex flex-col gap-3.5">
                {group.items.map((fragment) => (
                  <li key={fragment.id}>
                    <Card className="flex flex-col gap-2.5">
                      <Link href={fragment.permalink}>
                        <blockquote className="fragment-quote font-serif text-base leading-relaxed text-foreground">
                          “{fragment.text}”
                        </blockquote>
                      </Link>
                      <div className="flex items-center justify-between gap-3 text-xs text-muted">
                        <span className="truncate">
                          <span className="font-semibold text-foreground">{fragment.author_display_name}</span> ·{" "}
                          {fragment.post_title}
                        </span>
                        <span className="shrink-0">{formatDate(fragment.created_at, locale)}</span>
                      </div>
                      <div className="flex items-center gap-4 text-[13px]">
                        <Link href={fragment.permalink} className="font-medium text-primary hover:underline">
                          {t("openInPost")}
                        </Link>
                        <button type="button" onClick={() => handleCopy(fragment)} className="text-muted hover:text-foreground">
                          {copiedId === fragment.id ? tc("copied") : tc("copy")}
                        </button>
                        <button
                          type="button"
                          onClick={() => handleToggleVisibility(fragment.id, !fragment.is_public)}
                          className={fragment.is_public ? "text-primary hover:underline" : "text-muted hover:text-foreground"}
                          title={t("toggleVisibility")}
                        >
                          {fragment.is_public ? tc("public") : tc("private")}
                        </button>
                        <button
                          type="button"
                          onClick={() => handleRemove(fragment.id)}
                          className="ml-auto text-muted hover:text-foreground"
                        >
                          {tc("remove")}
                        </button>
                      </div>
                    </Card>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
