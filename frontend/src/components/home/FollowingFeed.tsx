"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { useEffect, useState } from "react";

import { FeedPostCardClient } from "@/components/FeedPostCardClient";
import { Button } from "@/components/ui/Button";
import { EmptyState, SkeletonRows } from "@/components/ui/States";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { Post } from "@/lib/types";

/** Tab "Seguiti" della home (mockup 1c): post dei blog e degli utenti seguiti,
 * `GET /feed/posts?following=true`. Richiede sessione: da anonimi invita ad accedere. */
export function FollowingFeed() {
  const { user, loading, authFetch } = useAuth();
  const t = useTranslations("HomePage");
  const [posts, setPosts] = useState<Post[] | null>(null);

  useEffect(() => {
    if (loading || !user) return;
    authFetch((token) => api.feed.following(token, 20))
      .then(setPosts)
      .catch(() => setPosts([]));
  }, [loading, user, authFetch]);

  if (loading) return <SkeletonRows rows={4} />;
  if (!user) {
    return (
      <EmptyState
        glyph="◯"
        title={t("followingSignInTitle")}
        body={t("followingSignInBody")}
        action={
          <Link href="/login">
            <Button size="sm">{t("signIn")}</Button>
          </Link>
        }
      />
    );
  }
  if (posts === null) return <SkeletonRows rows={4} />;
  if (posts.length === 0) {
    return (
      <EmptyState
        glyph="❝"
        title={t("followingEmptyTitle")}
        body={t("followingEmptyBody")}
        action={
          <Link href="/blogs">
            <Button size="sm" variant="secondary">
              {t("browseBlogs")}
            </Button>
          </Link>
        }
      />
    );
  }
  return (
    <div className="flex flex-col">
      {posts.map((post) => (
        <FeedPostCardClient key={post.id} post={post} />
      ))}
    </div>
  );
}
