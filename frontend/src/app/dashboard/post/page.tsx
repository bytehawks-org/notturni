"use client";

import { useLocale, useTranslations } from "next-intl";
import Link from "next/link";
import { useCallback, useEffect, useMemo, useState } from "react";

import { StatusPill } from "@/components/blog/StatusPill";
import { Alert } from "@/components/ui/Alert";
import { FilterChip } from "@/components/ui/Pill";
import { EmptyState, SkeletonRows } from "@/components/ui/States";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDate } from "@/lib/format";
import { displayPostStatus, type DisplayPostStatus } from "@/lib/post-status";
import type { Post } from "@/lib/types";

const FILTERS: ("all" | DisplayPostStatus)[] = ["all", "draft", "review", "scheduled", "published"];

/** Vista "tutti i miei blog" (todo/UX_REDESIGN.md), generalizza il tab Post
 * del singolo blog (PostsTab.tsx) a tutti i blog dell'utente insieme, con
 * l'attribuzione del blog per riga. */
export default function MyPostsPage() {
  const { authFetch } = useAuth();
  const t = useTranslations("MyPostsPage");
  const tc = useTranslations("Common");
  const locale = useLocale();
  const [posts, setPosts] = useState<Post[] | null>(null);
  const [filter, setFilter] = useState<(typeof FILTERS)[number]>("all");
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    authFetch((token) => api.users.myPosts(token))
      .then(setPosts)
      .catch((err) => setError(err instanceof ApiClientError ? err.message : tc("unexpectedError")));
  }, [authFetch, tc]);

  useEffect(load, [load]);

  const visible = useMemo(
    () => (posts ?? []).filter((p) => filter === "all" || displayPostStatus(p) === filter),
    [posts, filter]
  );

  return (
    <div className="mx-auto max-w-3xl">
      <div className="mb-5 flex flex-col gap-0.5">
        <h1 className="font-serif text-2xl font-medium text-foreground">{t("title")}</h1>
        <p className="text-[13px] text-muted">{t("subtitle")}</p>
      </div>

      <div className="mb-4 flex flex-wrap gap-1.5">
        {FILTERS.map((f) => (
          <FilterChip key={f} active={filter === f} onClick={() => setFilter(f)}>
            {t(`filter.${f}`)}
            {posts && f !== "all" && ` · ${posts.filter((p) => displayPostStatus(p) === f).length}`}
          </FilterChip>
        ))}
      </div>

      {error && (
        <div className="mb-5">
          <Alert kind="error">{error}</Alert>
        </div>
      )}

      {posts === null && !error && <SkeletonRows rows={5} />}
      {posts !== null && posts.length === 0 && <EmptyState glyph="✎" title={t("emptyTitle")} body={t("emptyBody")} />}
      {posts !== null && posts.length > 0 && visible.length === 0 && (
        <p className="text-sm text-muted">{t("noneInFilter")}</p>
      )}
      {visible.length > 0 && (
        <ul className="flex flex-col rounded-xl border border-border bg-surface">
          {visible.map((post) => {
            const status = displayPostStatus(post);
            return (
              <li
                key={post.id}
                className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-b border-border px-4 py-3 last:border-0"
              >
                <div className="flex min-w-0 flex-col gap-0.5">
                  <Link
                    href={`/dashboard/blogs/${post.blog_slug}?tab=posts`}
                    className="w-fit truncate text-[11px] font-semibold uppercase tracking-[.04em] text-primary no-underline hover:underline"
                  >
                    {post.blog_slug}
                  </Link>
                  <Link
                    href={`/dashboard/blogs/${post.blog_slug}/posts/${post.id}`}
                    className="truncate font-serif text-[17px] text-foreground no-underline hover:text-primary"
                  >
                    {post.title}
                  </Link>
                  <span className="truncate text-[13px] text-muted">
                    {post.locale.toUpperCase()} ·{" "}
                    {formatDate(post.published_at ?? post.created_at, locale, {
                      day: "numeric",
                      month: "short",
                      year: "numeric",
                    })}
                    {post.category && ` · ${post.category.name}`}
                  </span>
                </div>
                <div className="flex items-center gap-3">
                  {status === "published" && (
                    <Link href={post.permalink} className="text-[13px] text-muted no-underline hover:text-foreground">
                      {t("view")} ↗
                    </Link>
                  )}
                  <StatusPill status={status} />
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
