"use client";

import { useLocale, useTranslations } from "next-intl";
import Link from "next/link";
import { useEffect, useMemo, useState } from "react";

import { StatusPill } from "@/components/blog/StatusPill";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { FilterChip } from "@/components/ui/Pill";
import { EmptyState, SkeletonRows } from "@/components/ui/States";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDate } from "@/lib/format";
import { displayPostStatus, type DisplayPostStatus } from "@/lib/post-status";
import { type Post } from "@/lib/types";

import { errorMessage } from "./shared";

const FILTERS: ("all" | DisplayPostStatus)[] = ["all", "draft", "review", "scheduled", "published"];

/** Tab "Post" del blog (mockup 1e/5a): elenco con filtro per stato,
 * pubblicazione rapida delle bozze. */
export function PostsTab({ blogSlug, canWrite }: { blogSlug: string; canWrite: boolean }) {
  const { accessToken, authFetch } = useAuth();
  const t = useTranslations("PostsTab");
  const locale = useLocale();
  const [posts, setPosts] = useState<Post[] | null>(null);
  const [filter, setFilter] = useState<(typeof FILTERS)[number]>("all");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.posts
      .list(accessToken, blogSlug)
      .then(setPosts)
      .catch((err) => setError(errorMessage(err)));
  }, [blogSlug, accessToken]);

  async function handlePublish(postId: string) {
    try {
      const updated = await authFetch((token) => api.posts.publish(token, postId));
      setPosts((prev) => prev?.map((p) => (p.id === postId ? updated : p)) ?? null);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  const visible = useMemo(
    () => (posts ?? []).filter((p) => filter === "all" || displayPostStatus(p) === filter),
    [posts, filter]
  );

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex gap-1.5 overflow-x-auto">
          {FILTERS.map((f) => (
            <FilterChip key={f} active={filter === f} onClick={() => setFilter(f)}>
              {t(`filter.${f}`)}
              {posts && f !== "all" && ` · ${posts.filter((p) => displayPostStatus(p) === f).length}`}
            </FilterChip>
          ))}
        </div>
        {canWrite && (
          <Link href={`/dashboard/blogs/${blogSlug}/posts/new`}>
            <Button size="sm">{t("newPost")}</Button>
          </Link>
        )}
      </div>
      {error && <Alert kind="error">{error}</Alert>}
      {posts === null && !error && <SkeletonRows rows={5} />}
      {posts !== null && posts.length === 0 && (
        <EmptyState
          glyph="✎"
          title={t("emptyTitle")}
          body={t("emptyBody")}
          action={
            canWrite ? (
              <Link href={`/dashboard/blogs/${blogSlug}/posts/new`}>
                <Button size="sm">{t("newPost")}</Button>
              </Link>
            ) : undefined
          }
        />
      )}
      {posts !== null && posts.length > 0 && visible.length === 0 && <p className="text-sm text-muted">{t("noneInFilter")}</p>}
      {visible.length > 0 && (
        <ul className="flex flex-col rounded-xl border border-border bg-surface">
          {visible.map((post) => {
            const status = displayPostStatus(post);
            return (
              <li key={post.id} className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-b border-border px-4 py-3 last:border-0">
                <div className="flex min-w-0 flex-col gap-0.5">
                  <Link
                    href={`/dashboard/blogs/${blogSlug}/posts/${post.id}`}
                    className="truncate font-serif text-[17px] text-foreground no-underline hover:text-primary"
                  >
                    {post.title}
                  </Link>
                  <span className="truncate text-[13px] text-muted">
                    {post.locale.toUpperCase()} ·{" "}
                    {formatDate(post.published_at ?? post.created_at, locale, { day: "numeric", month: "short", year: "numeric" })}
                    {post.category && ` · ${post.category.name}`}
                  </span>
                </div>
                <div className="flex items-center gap-3">
                  {status === "published" && (
                    <Link href={post.permalink} className="text-[13px] text-muted no-underline hover:text-foreground">
                      {t("view")} ↗
                    </Link>
                  )}
                  {canWrite && status === "draft" && (
                    <Button variant="secondary" size="sm" onClick={() => handlePublish(post.id)}>
                      {t("publish")}
                    </Button>
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
